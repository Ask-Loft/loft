//! What the tag conformance tests share: the Features of this repository's own `spec/` that list
//! a tag.

use std::path::Path;

use loftspec::node::Node;
use loftspec_store::{DocumentStore, FileStore};

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
