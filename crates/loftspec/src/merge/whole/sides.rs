//! The three input trees, read, and each as one version by key: every record of every record
//! file, with each file's top record in a list of the whole tree, so that a record is matched
//! across the versions by its key whatever file holds it.

use std::collections::BTreeMap;

use super::super::Input;
use super::super::flat::{
    COMPONENT_FILES, DECISION_FILES, FEATURE_FILES, Flat, PRODUCT_FILES, Parent,
};
use super::TreeMergeError;
use crate::codec::{Content, Files, Rec, Role, SpecPath, Tree, filed_in_order, read_tree};
use crate::identity::Key;

/// The three trees, read; none for an empty base.
pub(super) struct Trees {
    pub(super) base: Option<Tree>,
    pub(super) ours: Tree,
    pub(super) theirs: Tree,
}

/// Reads the three trees.
///
/// # Errors
///
/// Every read error of every input.
pub(super) fn read([base, ours, theirs]: [&Files; 3]) -> Result<Trees, TreeMergeError> {
    let mut errors = Vec::new();
    let mut read = |input: Input, files: &Files| match read_tree(files) {
        Ok(tree) => Some(tree),
        Err(found) => {
            errors.extend(found.into_iter().map(|error| (input, error)));
            None
        }
    };
    let base = (!base.is_empty()).then(|| read(Input::Base, base));
    let ours = read(Input::Ours, ours);
    let theirs = read(Input::Theirs, theirs);
    match (base, ours, theirs) {
        (base, Some(ours), Some(theirs)) if errors.is_empty() => Ok(Trees {
            base: base.flatten(),
            ours,
            theirs,
        }),
        _ => Err(TreeMergeError::Unreadable(errors)),
    }
}

impl Trees {
    /// Each tree's record files as one version by key; an absent base holds no record.
    pub(super) fn flats(&self) -> [Flat; 3] {
        [
            self.base.as_ref().map_or_else(Flat::default, flat),
            flat(&self.ours),
            flat(&self.theirs),
        ]
    }

    /// Each version of the file at `path`; none where a tree lacks it.
    pub(super) fn file(&self, path: &SpecPath) -> [Option<&Content>; 3] {
        [self.base.as_ref(), Some(&self.ours), Some(&self.theirs)].map(|tree| {
            tree.and_then(|tree| tree.files.get(path))
                .map(|(_, content)| content)
        })
    }
}

/// One tree's record files by key: each Product in the order the manifest lists it, with its
/// Features in the order each of its Feature Groups lists them, then the root Components and
/// the decisions. The reader checked that every Product and Feature is listed once.
fn flat(tree: &Tree) -> Flat {
    let mut flat = Flat::default();
    for filed in filed_in_order(tree) {
        match filed.group {
            None => flat.node(filed.record, (Parent::Top, PRODUCT_FILES)),
            Some(group) => {
                if let Some(group) = group.key() {
                    flat.node(filed.record, (Parent::Key(group), FEATURE_FILES));
                }
            }
        }
    }
    for (_, record) in tops(tree, |role| matches!(role, Role::Component { .. })) {
        flat.node(record, (Parent::Top, COMPONENT_FILES));
    }
    for (_, record) in tops(tree, |role| matches!(role, Role::Decision { .. })) {
        flat.node(record, (Parent::Top, DECISION_FILES));
    }
    flat
}

/// The top record of each file of a role, by its key.
fn tops(tree: &Tree, wanted: impl Fn(&Role) -> bool + 'static) -> BTreeMap<Key, &Rec> {
    tree.records(wanted)
        .filter_map(|(_, _, record)| Some((record.key()?, record)))
        .collect()
}
