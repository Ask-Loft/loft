//! The merge of one list field (D-11 F-32, F-33): elements matched across the sides by key, a
//! binding by its key, an assessment by its option and driver, anything else by its value
//! (AC-471, AC-662); which elements survive; and, for a list in authored order, where each goes.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::{Host, conflict, push};
use super::order::{Anchor, merge_order, moved};
use super::record::merge_host;
use super::same::{same_value, text};
use crate::codec::schema::{Field, Item, Kind, Order, Shape, field};
use crate::codec::{Rec, Val};

/// What an element is matched by, and how many times its side has held it before.
type Id = (Ident, usize);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Ident {
    Key(String),
    Pair(String, String),
    Value(String),
}

/// How an element of a list merges.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Element {
    /// A record with a key, which holds its own conflicts and merges field by field.
    Host,
    /// An assessment, which merges as one value (RQ-119's whole values); bindings, the other
    /// whole values, are merged by the labels merge before any list merge sees them.
    Whole,
    /// A scalar or a record matched by its value, which no side can change, only add or remove.
    Value,
}

const fn element(item: Item) -> Element {
    match item {
        Item::Record(Kind::Assessment) => Element::Whole,
        Item::Record(kind) if kind.hosts_conflicts() => Element::Host,
        _ => Element::Value,
    }
}

fn ident(item: Item, value: &Val) -> Ident {
    let record_text = |name: &str| match value {
        Val::Record(record) => record.text(name).unwrap_or_default().to_owned(),
        _ => String::new(),
    };
    match (element(item), item) {
        (Element::Host | Element::Whole, Item::Record(Kind::Assessment)) => {
            Ident::Pair(record_text(field::OPTION), record_text(field::DRIVER))
        }
        (Element::Host | Element::Whole, _) => Ident::Key(record_text(field::KEY)),
        (Element::Value, _) => Ident::Value(text(value)),
    }
}

/// One side's list: each element's id in order, and the element each id names.
struct Side<'a> {
    ids: Vec<Id>,
    by_id: BTreeMap<Id, &'a Val>,
}

fn side(item: Item, values: &[Val]) -> Side<'_> {
    let mut seen: BTreeMap<Ident, usize> = BTreeMap::new();
    let mut ids = Vec::new();
    let mut by_id = BTreeMap::new();
    for value in values {
        let ident = ident(item, value);
        let count = seen.entry(ident.clone()).or_default();
        let id = (ident, *count);
        *count += 1;
        ids.push(id.clone());
        by_id.insert(id, value);
    }
    Side { ids, by_id }
}

/// Merges a list field whose sides differ; a disagreement over a whole value, or over the place
/// of a value, is one conflict over the list on the record that holds it.
pub(super) fn merge_list(
    each: &Field,
    [base, ours, theirs]: [&[Val]; 3],
    host: &mut Host<'_>,
) -> Vec<Val> {
    let Shape::List(item) = each.shape else {
        unreachable!("only a list field merges as a list");
    };
    let kind = element(item);
    let sides = [side(item, base), side(item, ours), side(item, theirs)];
    let mut merger = Merger {
        kind,
        moved_by_ours: moved(&sides[0].ids, &sides[1].ids),
        moved_by_theirs: moved(&sides[0].ids, &sides[2].ids),
        disagree: false,
    };
    let mut elements = merger.elements(&sides);
    let order = if each.order == Order::Authored {
        let survives = |id: &Id| elements.contains_key(id);
        let result = merge_order(&sides[0].ids, &sides[1].ids, &sides[2].ids, &survives);
        for position in result.conflicts {
            match elements.get_mut(&position.element) {
                Some(Val::Record(record)) if kind == Element::Host => {
                    push(record, position_conflict(&position));
                }
                _ => merger.disagree = true,
            }
        }
        result.order
    } else {
        sides[1].ids.iter().chain(&sides[2].ids).cloned().collect()
    };
    if merger.disagree {
        let lists = [base, ours, theirs].map(|values| Val::List(values.to_vec()));
        host.conflict(each.name, lists.each_ref().map(Some));
    }
    let mut placed = BTreeSet::new();
    order
        .into_iter()
        .filter(|id| placed.insert(id.clone()))
        .filter_map(|id| elements.remove(&id))
        .collect()
}

struct Merger {
    kind: Element,
    moved_by_ours: BTreeSet<Id>,
    moved_by_theirs: BTreeSet<Id>,
    disagree: bool,
}

impl Merger {
    /// Every surviving element, merged, by its id.
    fn elements(&mut self, [base, ours, theirs]: &[Side<'_>; 3]) -> BTreeMap<Id, Val> {
        let ids: BTreeSet<&Id> = base
            .ids
            .iter()
            .chain(&ours.ids)
            .chain(&theirs.ids)
            .collect();
        ids.into_iter()
            .filter_map(|id| {
                let [base, ours, theirs] =
                    [&base.by_id, &ours.by_id, &theirs.by_id].map(|by_id| by_id.get(id).copied());
                self.element(id, base, ours, theirs)
                    .map(|value| (id.clone(), value))
            })
            .collect()
    }

    fn element(
        &mut self,
        id: &Id,
        base: Option<&Val>,
        ours: Option<&Val>,
        theirs: Option<&Val>,
    ) -> Option<Val> {
        match (base, ours, theirs) {
            (Some(base), Some(ours), Some(theirs)) => Some(self.three(base, ours, theirs)),
            (Some(base), Some(kept), None) => {
                self.deleted(base, kept, self.moved_by_ours.contains(id), Side3::Theirs)
            }
            (Some(base), None, Some(kept)) => {
                self.deleted(base, kept, self.moved_by_theirs.contains(id), Side3::Ours)
            }
            (None, Some(ours), Some(theirs)) => Some(self.added(ours, theirs)),
            (None, Some(only), None) | (None, None, Some(only)) => Some(only.clone()),
            (Some(_) | None, None, None) => None,
        }
    }

    /// An element all three sides hold.
    fn three(&mut self, base: &Val, ours: &Val, theirs: &Val) -> Val {
        match (self.kind, base, ours, theirs) {
            (Element::Host, Val::Record(base), Val::Record(ours), Val::Record(theirs)) => {
                Val::Record(merge_host(Some(base), ours, theirs))
            }
            (Element::Whole, ..) => self.whole(Some(base), ours, theirs),
            _ => base.clone(),
        }
    }

    /// An element both sides added.
    fn added(&mut self, ours: &Val, theirs: &Val) -> Val {
        match (self.kind, ours, theirs) {
            (Element::Host, Val::Record(ours), Val::Record(theirs)) => {
                Val::Record(merge_host(None, ours, theirs))
            }
            _ => self.whole(None, ours, theirs),
        }
    }

    /// A whole value: one side's change wins, equal changes coalesce, and a disagreement keeps
    /// base, or ours where base lacks it, with a conflict over the list.
    fn whole(&mut self, base: Option<&Val>, ours: &Val, theirs: &Val) -> Val {
        let unchanged = |side: &Val| base.is_some_and(|base| same_value(side, base));
        if same_value(ours, theirs) || unchanged(theirs) {
            return ours.clone();
        }
        if unchanged(ours) {
            return theirs.clone();
        }
        self.disagree = true;
        base.unwrap_or(ours).clone()
    }

    /// An element one side deleted: removed when the other side left it as it was, and
    /// otherwise, for a record with a key, kept as the other side wrote it with a conflict over
    /// the record (AC-473, AC-660).
    fn deleted(&self, base: &Val, kept: &Val, moved: bool, deleter: Side3) -> Option<Val> {
        let unchanged = same_value(base, kept) && !moved;
        match (unchanged, self.kind, kept) {
            (false, Element::Host, Val::Record(record)) => {
                let mut record = record.clone();
                let key = record.get(field::KEY).cloned();
                let null = Val::Null {
                    position: record.position,
                };
                let sides = match deleter {
                    Side3::Ours => [key.as_ref(), Some(&null), key.as_ref()],
                    Side3::Theirs => [key.as_ref(), key.as_ref(), Some(&null)],
                };
                push(&mut record, conflict(field::RECORD, sides));
                Some(Val::Record(record))
            }
            _ => None,
        }
    }
}

/// The side that deleted an element.
#[derive(Debug, Clone, Copy)]
enum Side3 {
    Ours,
    Theirs,
}

/// A conflict over where a record goes: each side names the element it follows, or `first`.
fn position_conflict(position: &super::order::PositionConflict<Id>) -> Val {
    let anchor = |anchor: &Option<Anchor<Id>>| {
        anchor.as_ref().map(|anchor| {
            Val::scalar(match anchor {
                Anchor::First => "first",
                Anchor::After((Ident::Key(key) | Ident::Value(key), _)) => key.as_str(),
                Anchor::After((Ident::Pair(option, _), _)) => option.as_str(),
            })
        })
    };
    let [base, ours, theirs] = [&position.base, &position.ours, &position.theirs].map(anchor);
    conflict(
        field::POSITION,
        [base.as_ref(), ours.as_ref(), theirs.as_ref()],
    )
}

/// The records of a list, to merge as host records of a registry.
pub(super) fn records(values: Vec<Val>) -> Vec<Rec> {
    values
        .into_iter()
        .filter_map(|value| match value {
            Val::Record(record) => Some(record),
            _ => None,
        })
        .collect()
}
