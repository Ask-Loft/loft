//! What the tag conformance tests share: the Features of this repository's own `spec/` that list
//! a tag, and the commands they are.

use std::path::Path;

use loftspec::node::Node;
use loftspec_store::{DocumentStore, FileStore};

use crate::commands::{COMMANDS, Fixture};

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

/// The command of each Feature that lists `tag`, each with its arguments and fixture.
pub fn tagged_commands(tag: &str) -> Vec<(&'static str, &'static [&'static str], Fixture)> {
    tagged(tag)
        .into_iter()
        .map(|slug| {
            COMMANDS
                .into_iter()
                .find(|(name, _, _)| *name == slug)
                .unwrap_or_else(|| panic!("{slug} lists `{tag}` but COMMANDS has no entry"))
        })
        .collect()
}
