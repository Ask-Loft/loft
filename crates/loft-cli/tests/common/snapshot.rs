//! Every file under a directory, by its path from that directory, outside `.git`: what a test
//! compares to show that a command wrote nothing.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.ends_with(".git") {
                continue;
            }
            if path.is_dir() {
                pending.push(path);
            } else {
                let relative = path.strip_prefix(root).unwrap().display().to_string();
                files.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    files
}

/// The paths of the files that differ between two snapshots: each one `after` holds and `before`
/// does not hold with the same bytes.
pub fn changed(
    before: &BTreeMap<String, Vec<u8>>,
    after: &BTreeMap<String, Vec<u8>>,
) -> Vec<String> {
    after
        .iter()
        .filter(|(path, bytes)| before.get(*path) != Some(bytes))
        .map(|(path, _)| path.clone())
        .collect()
}
