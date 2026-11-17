//! The files a Codex `apply_patch` call edits, from its patch's headers.

/// The headers that name a file the patch adds, updates, deletes or moves to.
const HEADERS: &[&str] = &[
    "*** Add File: ",
    "*** Update File: ",
    "*** Delete File: ",
    "*** Move to: ",
];

/// Each file the patch `patch` edits, in the order it names them, the destination of a move
/// included.
pub fn patched(patch: &str) -> Vec<String> {
    patch
        .lines()
        .filter_map(|line| {
            HEADERS
                .iter()
                .find_map(|header| line.strip_prefix(header))
                .map(|path| path.trim().to_owned())
        })
        .filter(|path| !path.is_empty())
        .collect()
}
