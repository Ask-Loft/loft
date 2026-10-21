//! The tree merger: a three-way merge of three whole `spec/` trees, which pairs each record by
//! its key whatever file holds it on each side and merges each pair by the merge engine's rules,
//! so that a record moved to another file on one side and changed on the other comes out once.
//! It takes only the base, ours and theirs versions of one tree. The manifest, `spec/labels.yaml`,
//! the registries and `spec/lint.yaml` merge as the merge driver merges each of them.

mod sides;
mod single;
mod write;

use std::fmt;

use super::tree::{Referenced, merge_flat};
use super::{Input, MergeError, count};
use crate::codec::{Files, ReadError, SpecPath, read_tree};
use crate::identity::{Key, Label};

/// The merged tree, every file in canonical layout, how many conflict records it holds, and the
/// labels that two publishes gave apart.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergedTree {
    pub files: Files,
    pub conflicts: usize,
    pub disputed: Vec<Label>,
}

/// Why three trees could not be merged; no tree is returned.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeMergeError {
    /// Each input tree that does not read, with where.
    Unreadable(Vec<(Input, ReadError)>),
    /// A file that merges as the merge driver merges it, which refused it: `spec/lint.yaml`
    /// that both sides changed to different contents.
    File { path: SpecPath, error: MergeError },
    /// A record the merge surely keeps that no place can take, which only a side that changed
    /// the kind of the record or of its parent can leave.
    Unplaced(Key),
    /// Two records of the result that their parents and slugs place in one file.
    SamePath(SpecPath),
    /// The merged tree does not read back.
    Unwritable(Vec<ReadError>),
}

impl fmt::Display for TreeMergeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable(errors) => {
                formatter.write_str("an input tree does not read")?;
                for (input, error) in errors {
                    write!(formatter, "; {input}: {error}")?;
                }
                Ok(())
            }
            Self::File {
                path,
                error: MergeError::BothChanged,
            } => write!(formatter, "both sides changed {path} to different contents"),
            Self::File { path, .. } => write!(formatter, "{path} does not merge"),
            Self::Unplaced(key) => write!(formatter, "no place of the merge can take {key}"),
            Self::SamePath(path) => write!(formatter, "two records of the merge are {path}"),
            Self::Unwritable(errors) => {
                formatter.write_str("the merged tree does not read")?;
                for error in errors {
                    write!(formatter, "; {error}")?;
                }
                Ok(())
            }
        }
    }
}

impl std::error::Error for TreeMergeError {}

/// Merges three whole `spec/` trees. An empty base is a base that holds no record.
///
/// # Errors
///
/// An input tree does not read, both sides changed `spec/lint.yaml` to different contents, a
/// record the merge keeps has no place, two records of the result fall in one file, or the
/// result does not read back.
pub fn merge_tree(
    base: &Files,
    ours: &Files,
    theirs: &Files,
) -> Result<MergedTree, TreeMergeError> {
    let trees = sides::read([base, ours, theirs])?;
    let singles = single::merge(&trees)?;
    let flats = trees.flats();
    let by_key = merge_flat(&flats, Referenced::Kept).map_err(TreeMergeError::Unplaced)?;
    let tree = write::tree(singles.files, by_key, &flats)?;
    let files = crate::codec::write_files(&tree);
    let read = read_tree(&files).map_err(TreeMergeError::Unwritable)?;
    let conflicts = read
        .files
        .values()
        .flat_map(|(_, content)| content.records())
        .map(count)
        .sum();
    Ok(MergedTree {
        files,
        conflicts,
        disputed: singles.disputed,
    })
}
