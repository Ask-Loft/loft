//! The files of a tree that hold no record of another file and that no record leaves: the
//! manifest, `spec/labels.yaml`, the registries and `spec/lint.yaml`, each merged as the merge
//! driver merges it.

use std::collections::BTreeMap;

use super::super::{merge_content, one_value};
use super::TreeMergeError;
use super::sides::Trees;
use crate::codec::{CATEGORIES, Content, LABELS, LINT, MANIFEST, Role, SpecPath, TAGS, TERMS};
use crate::identity::Label;

/// The merged single files, and the labels that two publishes gave apart.
pub(super) struct Singles {
    pub(super) files: BTreeMap<SpecPath, (Role, Content)>,
    pub(super) disputed: Vec<Label>,
}

/// Merges each single file that ours or theirs holds.
///
/// # Errors
///
/// The merge driver's refusal of a file, with its path: `spec/lint.yaml` that both sides
/// changed to different contents.
pub(super) fn merge(trees: &Trees) -> Result<Singles, TreeMergeError> {
    let mut singles = Singles {
        files: BTreeMap::new(),
        disputed: Vec::new(),
    };
    for name in [MANIFEST, LABELS, TAGS, CATEGORIES, TERMS, LINT] {
        let path = SpecPath::new(name);
        let role = Role::of(&path).expect("a single file of the Format");
        let versions = trees.file(&path);
        let refused = |error| TreeMergeError::File {
            path: path.clone(),
            error,
        };
        let Some((content, disputed)) = merged(&role, versions).map_err(refused)? else {
            continue;
        };
        singles.disputed.extend(disputed);
        singles.files.insert(path, (role, content));
    }
    Ok(singles)
}

type Merged = Option<(Content, Vec<Label>)>;

/// One single file merged; none where the result lacks it. A registry that a side lacks is
/// an empty one, and the result holds the registry where both sides hold it or where it holds
/// an entry. The lint configuration, and the manifest or the bindings that one side lacks, merge
/// as one value: one side's change wins.
fn merged(
    role: &Role,
    [base, ours, theirs]: [Option<&Content>; 3],
) -> Result<Merged, super::super::MergeError> {
    let whole = |kept: Option<&Content>| kept.map(|content| (content.clone(), Vec::new()));
    if *role == Role::Lint {
        return Ok(whole(one_value(&base, ours, theirs)?));
    }
    match (ours, theirs) {
        (Some(ours), Some(theirs)) => merge_content(base, ours, theirs).map(Some),
        (None, None) => Ok(None),
        (ours, theirs) if role.is_registry() => {
            let empty = Content::Registry(Vec::new());
            let (content, disputed) =
                merge_content(base, ours.unwrap_or(&empty), theirs.unwrap_or(&empty))?;
            let holds = content.records().next().is_some();
            Ok(holds.then_some((content, disputed)))
        }
        (ours, theirs) => Ok(whole(one_value(&base, ours, theirs)?)),
    }
}
