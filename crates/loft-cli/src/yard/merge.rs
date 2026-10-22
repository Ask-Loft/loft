//! `loft merge-driver` on a file of Yard Log under `.yard/` (the yardlog dispatch of the
//! `loft merge-driver` Feature): the root and each stage's file joined by key, as yardlog
//! merges two copies of one file, and a file one session writes alone taken whole from the copy
//! whose last event ended later. None of them ever gives a conflict record; what spans files is
//! left to the settling of the next `loft yard` command.

use std::path::{Component, Path};

use yardlog::codec::session::{ACTIVITY, TOKENS};
use yardlog::codec::{FileError, ROOT};
use yardlog::merge::{Taken, merge_file, take_session_file};

/// The folder of every branch's Yard Log, at the top of the worktree.
const YARD: &str = ".yard";

/// Whether `path`, as git names it from the top of the worktree, is a file of Yard Log.
pub fn holds(path: &str) -> bool {
    Path::new(path).components().next() == Some(Component::Normal(YARD.as_ref()))
}

/// Merges two copies of the file of Yard Log at `path` over their base and gives the result;
/// or the messages, each to follow `<path>`. `root` is the top of the worktree, where git runs
/// the driver, and where this copy's Yard Log lies.
pub fn merge(
    root: &Path,
    path: &str,
    base: &[u8],
    ours: &[u8],
    theirs: &[u8],
) -> Result<Vec<u8>, Vec<String>> {
    let file = Path::new(path);
    let name = file
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or(path);
    if is_session_file(root, file) {
        let named = format!("{}/{name}", folder(file));
        return match take_session_file(&named, ours, theirs).map_err(message)? {
            Taken::Ours => Ok(ours.to_vec()),
            Taken::Theirs => Ok(theirs.to_vec()),
        };
    }
    // git hands an empty base where the two copies added the file apart.
    let base = (!base.is_empty()).then_some(base);
    merge_file(name, base, ours, theirs)
        .map(String::into_bytes)
        .map_err(message)
}

/// The name of the folder that holds `file`.
fn folder(file: &Path) -> &str {
    file.parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .unwrap_or_default()
}

/// Whether `file` is one a session writes alone: in a `tokens` or `activity` folder that is not
/// itself a branch's Yard Log, as the folder of a branch named `x/tokens` would be.
fn is_session_file(root: &Path, file: &Path) -> bool {
    [TOKENS, ACTIVITY].contains(&folder(file))
        && file
            .parent()
            .is_some_and(|parent| !root.join(parent).join(ROOT).is_file())
}

/// The message of a copy that does not read: where in the file, which copy, and why.
fn message((copy, error): (&'static str, FileError)) -> Vec<String> {
    let at = error
        .error
        .position
        .map(|position| format!(":{position}"))
        .unwrap_or_default();
    vec![format!("{at}: {copy}: {}", error.error.message)]
}
