//! What the tag conformance tests share: the Features of this repository's own `spec/` that list
//! a tag, the check that a pending command is still unknown, and a snapshot of a scratch tree.

use std::collections::BTreeMap;
use std::fs;
use std::path::Path;

use loftspec::node::Node;
use loftspec_store::{DocumentStore, FileStore};

use crate::common::Scratch;

/// The slugs of the Features in this repository's `spec/` that list `tag`.
pub fn tagged(tag: &str) -> Vec<String> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let document = FileStore::open(&root).unwrap().load().unwrap();
    let mut slugs = Vec::new();
    document.walk(&mut |node| {
        if let Node::Feature(feature) = node
            && feature.details.tags.iter().any(|name| name.as_str() == tag)
        {
            slugs.push(feature.slug.as_str().to_owned());
        }
    });
    slugs.sort();
    slugs
}

/// Whether the Feature `slug` is pending, after checking that its subcommand is still unknown
/// to `loft`; once it exists it must move from `pending` to the test's commands.
pub fn is_pending(slug: &str, pending: &[(&str, &str)]) -> bool {
    let Some((_, subcommand)) = pending.iter().find(|(name, _)| *name == slug) else {
        return false;
    };
    let output = Scratch::with_reading_list().loft(&[subcommand]);
    assert_eq!(
        output.status.code(),
        Some(2),
        "`loft {subcommand}` exists now: move {slug} from PENDING to COMMANDS"
    );
    true
}

/// Every file under `root` with its bytes, by its path from `root`.
pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut files = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                pending.push(path);
            } else {
                let name = path.strip_prefix(root).unwrap().display().to_string();
                files.insert(name, fs::read(&path).unwrap());
            }
        }
    }
    files
}
