//! The three-way merge of one ordered list (D-11 F-33). Each side's insertions and moves hang
//! after the element they follow on that side, ours before theirs; the elements neither side
//! moved keep their base order; an element both sides moved apart stays at its base position,
//! with a conflict over its place (RQ-123, RQ-124). Which elements survive is the caller's to
//! say; this module decides only the order.

use std::collections::{BTreeMap, BTreeSet};

/// Where an element sits on one side: first, or after another element.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum Anchor<Id> {
    First,
    After(Id),
}

/// An element both sides moved to different places, with where each side has it; none on a
/// side that does not hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PositionConflict<Id> {
    pub element: Id,
    pub base: Option<Anchor<Id>>,
    pub ours: Option<Anchor<Id>>,
    pub theirs: Option<Anchor<Id>>,
}

/// The merged order, and the conflicts over the place of elements kept at their base position.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Merged<Id> {
    pub order: Vec<Id>,
    pub conflicts: Vec<PositionConflict<Id>>,
}

/// The lists, and which base elements each side left in place.
struct Sides<'a, Id> {
    base: &'a [Id],
    ours: &'a [Id],
    theirs: &'a [Id],
    still_ours: BTreeSet<Id>,
    still_theirs: BTreeSet<Id>,
}

impl<Id: Ord + Clone> Sides<'_, Id> {
    /// Whether a side holds a base element in a place it moved it to.
    fn moved(&self, element: &Id, side: &[Id], still: &BTreeSet<Id>) -> bool {
        self.base.contains(element) && side.contains(element) && !still.contains(element)
    }

    fn moved_by_ours(&self, element: &Id) -> bool {
        self.moved(element, self.ours, &self.still_ours)
    }

    fn moved_by_theirs(&self, element: &Id) -> bool {
        self.moved(element, self.theirs, &self.still_theirs)
    }

    fn conflict(&self, element: &Id) -> PositionConflict<Id> {
        PositionConflict {
            element: element.clone(),
            base: anchor(self.base, element),
            ours: anchor(self.ours, element),
            theirs: anchor(self.theirs, element),
        }
    }
}

/// Merges one ordered list. `survives` says, for every element of any of the three lists,
/// whether it is in the result.
pub fn merge_order<Id: Ord + Clone>(
    base: &[Id],
    ours: &[Id],
    theirs: &[Id],
    survives: &dyn Fn(&Id) -> bool,
) -> Merged<Id> {
    let sides = Sides {
        base,
        ours,
        theirs,
        still_ours: stationary(base, ours),
        still_theirs: stationary(base, theirs),
    };
    let mut pinned = BTreeSet::new();
    let mut conflicts = Vec::new();
    for element in base.iter().filter(|element| survives(element)) {
        if sides.moved_by_ours(element)
            && sides.moved_by_theirs(element)
            && anchor(ours, element) != anchor(theirs, element)
        {
            pinned.insert(element.clone());
            conflicts.push(sides.conflict(element));
        }
    }
    let survivors: BTreeSet<&Id> = base
        .iter()
        .chain(ours)
        .chain(theirs)
        .filter(|element| survives(element))
        .collect();
    loop {
        let order = assemble(&sides, &pinned, survives);
        let placed: BTreeSet<&Id> = order.iter().collect();
        // A move on each side that places one element after the other leaves both unplaced;
        // they stay at their base position.
        let stranded: Vec<&Id> = base
            .iter()
            .filter(|element| survivors.contains(element) && !placed.contains(element))
            .filter(|element| !pinned.contains(*element))
            .collect();
        if stranded.is_empty() {
            return Merged { order, conflicts };
        }
        for element in stranded {
            pinned.insert(element.clone());
            conflicts.push(sides.conflict(element));
        }
    }
}

/// Places every surviving element: each side's insertions and moves after the element they
/// follow on that side, ours first; then the base elements neither side moved, in base order.
fn assemble<Id: Ord + Clone>(
    sides: &Sides<'_, Id>,
    pinned: &BTreeSet<Id>,
    survives: &dyn Fn(&Id) -> bool,
) -> Vec<Id> {
    let mut children: BTreeMap<Anchor<Id>, Vec<Id>> = BTreeMap::new();
    let mut placed = BTreeSet::new();
    for (side, still) in [
        (sides.ours, &sides.still_ours),
        (sides.theirs, &sides.still_theirs),
    ] {
        let mut after = Anchor::First;
        for element in side.iter().filter(|element| survives(element)) {
            let stays = still.contains(element) || pinned.contains(element);
            if !(sides.base.contains(element) && stays) && placed.insert(element.clone()) {
                children
                    .entry(after.clone())
                    .or_default()
                    .push(element.clone());
            }
            after = Anchor::After(element.clone());
        }
    }
    let spine = sides.base.iter().filter(|element| {
        survives(element)
            && (pinned.contains(*element)
                || !(sides.moved_by_ours(element) || sides.moved_by_theirs(element)))
    });
    let mut order = Vec::new();
    emit(&Anchor::First, &children, &mut order);
    for element in spine {
        order.push(element.clone());
        emit(&Anchor::After(element.clone()), &children, &mut order);
    }
    order
}

/// Writes the elements that hang after `anchor`, each followed by those that hang after it.
fn emit<Id: Ord + Clone>(
    anchor: &Anchor<Id>,
    children: &BTreeMap<Anchor<Id>, Vec<Id>>,
    order: &mut Vec<Id>,
) {
    for child in children.get(anchor).into_iter().flatten() {
        order.push(child.clone());
        emit(&Anchor::After(child.clone()), children, order);
    }
}

/// Where a list has an element: first, or after the element before it; none if it lacks it.
fn anchor<Id: Ord + Clone>(list: &[Id], element: &Id) -> Option<Anchor<Id>> {
    let at = list.iter().position(|each| each == element)?;
    Some(match at {
        0 => Anchor::First,
        _ => Anchor::After(list[at - 1].clone()),
    })
}

/// The base elements a side left in place: a longest sequence of the elements both hold that
/// both hold in the same order. Of two equally long choices, the one that keeps the earlier
/// base element in place.
fn stationary<Id: Ord + Clone>(base: &[Id], side: &[Id]) -> BTreeSet<Id> {
    let in_side: BTreeSet<&Id> = side.iter().collect();
    let in_base: BTreeSet<&Id> = base.iter().collect();
    let left: Vec<&Id> = base.iter().filter(|each| in_side.contains(each)).collect();
    let right: Vec<&Id> = side.iter().filter(|each| in_base.contains(each)).collect();
    // `longest[i][j]`: the longest common sequence of `left[i..]` and `right[j..]`.
    let mut longest = vec![vec![0usize; right.len() + 1]; left.len() + 1];
    for i in (0..left.len()).rev() {
        for j in (0..right.len()).rev() {
            longest[i][j] = if left[i] == right[j] {
                longest[i + 1][j + 1] + 1
            } else {
                longest[i + 1][j].max(longest[i][j + 1])
            };
        }
    }
    let (mut i, mut j) = (0, 0);
    let mut still = BTreeSet::new();
    while i < left.len() && j < right.len() {
        if left[i] == right[j] {
            still.insert(left[i].clone());
            i += 1;
            j += 1;
        } else if longest[i][j + 1] >= longest[i + 1][j] {
            j += 1;
        } else {
            i += 1;
        }
    }
    still
}
