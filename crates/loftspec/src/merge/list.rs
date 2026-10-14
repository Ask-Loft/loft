//! The merge of one list field of unkeyed values (F-32, F-33): elements matched across the
//! sides, an assessment by its option and driver, anything else by its value (AC-471, AC-662);
//! which elements survive; and, for a list in authored order, where each goes. Lists of keyed
//! records are merged by key, record by record, and never reach this merge.

use std::collections::{BTreeMap, BTreeSet};

use super::conflict::Host;
use super::order::merge_order;
use super::same::{same_value, text};
use crate::codec::Val;
use crate::codec::schema::{Field, Item, Kind, Order, Shape, field};

/// What an element is matched by, and how many times its side has held it before.
type Id = (Ident, usize);

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
enum Ident {
    Pair(String, String),
    Value(String),
}

/// What a list's element is matched by: an assessment (`whole`) by its option and driver, and
/// anything else by its value.
fn ident(whole: bool, value: &Val) -> Ident {
    let record_text = |name: &str| match value {
        Val::Record(record) => record.text(name).unwrap_or_default().to_owned(),
        _ => String::new(),
    };
    if whole {
        Ident::Pair(record_text(field::OPTION), record_text(field::DRIVER))
    } else {
        Ident::Value(text(value))
    }
}

/// One side's list: each element's id in order, and the element each id names.
struct Listed<'a> {
    ids: Vec<Id>,
    by_id: BTreeMap<Id, &'a Val>,
}

fn side(whole: bool, values: &[Val]) -> Listed<'_> {
    let mut seen: BTreeMap<Ident, usize> = BTreeMap::new();
    let mut ids = Vec::new();
    let mut by_id = BTreeMap::new();
    for value in values {
        let ident = ident(whole, value);
        let count = seen.entry(ident.clone()).or_default();
        let id = (ident, *count);
        *count += 1;
        ids.push(id.clone());
        by_id.insert(id, value);
    }
    Listed { ids, by_id }
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
    let whole = item == Item::Record(Kind::Assessment);
    let sides = [side(whole, base), side(whole, ours), side(whole, theirs)];
    let mut merger = Merger {
        whole,
        disagree: false,
    };
    let mut elements = merger.elements(&sides);
    let order = if each.order == Order::Authored {
        let survives = |id: &Id| elements.contains_key(id);
        let result = merge_order(&sides[0].ids, &sides[1].ids, &sides[2].ids, &survives);
        merger.disagree |= !result.conflicts.is_empty();
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
    /// Whether the list holds assessments, each of which merges as one value (RQ-119's whole
    /// values); bindings, the other whole values, are merged by the labels merge before any
    /// list merge sees them. Any other element is a scalar or a record matched by its value,
    /// which no side can change, only add or remove.
    whole: bool,
    disagree: bool,
}

impl Merger {
    /// Every surviving element, merged, by its id.
    fn elements(&mut self, [base, ours, theirs]: &[Listed<'_>; 3]) -> BTreeMap<Id, Val> {
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
                self.element(base, ours, theirs)
                    .map(|value| (id.clone(), value))
            })
            .collect()
    }

    fn element(
        &mut self,
        base: Option<&Val>,
        ours: Option<&Val>,
        theirs: Option<&Val>,
    ) -> Option<Val> {
        match (base, ours, theirs) {
            (Some(base), Some(ours), Some(theirs)) => Some(self.three(base, ours, theirs)),
            (Some(base), Some(kept), None) | (Some(base), None, Some(kept)) => {
                self.deleted(base, kept)
            }
            (None, Some(ours), Some(theirs)) => Some(self.whole_value(None, ours, theirs)),
            (None, Some(only), None) | (None, None, Some(only)) => Some(only.clone()),
            (Some(_) | None, None, None) => None,
        }
    }

    /// An element all three sides hold.
    fn three(&mut self, base: &Val, ours: &Val, theirs: &Val) -> Val {
        if self.whole {
            self.whole_value(Some(base), ours, theirs)
        } else {
            base.clone()
        }
    }

    /// A whole value: one side's change wins, equal changes coalesce, and a disagreement keeps
    /// base, or ours where base lacks it, with a conflict over the list.
    fn whole_value(&mut self, base: Option<&Val>, ours: &Val, theirs: &Val) -> Val {
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

    /// An element one side deleted: removed when the other side left it as it was; an
    /// assessment the other side changed keeps its base value with a conflict over the list, as
    /// when both sides change it apart.
    fn deleted(&mut self, base: &Val, kept: &Val) -> Option<Val> {
        if self.whole && !same_value(base, kept) {
            self.disagree = true;
            return Some(base.clone());
        }
        None
    }
}
