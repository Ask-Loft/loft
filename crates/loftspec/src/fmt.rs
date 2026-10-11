//! `loft fmt` as a function (D-11 F-39): the tree read as the formatter reads it, every missing
//! key minted and every missing slug supplied, and the files whose bytes change in canonical
//! layout. It writes nothing itself; writing belongs to a store (CMP-4).

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::{
    Content, Files, MANIFEST, Mode, ReadError, Rec, Role, SpecPath, Tree, Val, canonicalize,
    read_tree, write_files,
};
use crate::identity::Key;
use crate::slug::Slug;
use crate::yaml::Position;

/// What formatting a tree changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formatted {
    /// Each file whose bytes change, with its new bytes (AC-534, AC-535).
    pub changed: BTreeMap<SpecPath, Vec<u8>>,
    /// How many keys were minted (AC-536).
    pub minted: usize,
}

/// Formats a tree.
///
/// # Errors
///
/// Every read error of the tree, read as the formatter reads it; nothing is formatted then
/// (RQ-141).
pub fn format(files: &Files) -> Result<Formatted, Vec<ReadError>> {
    let mut tree = read_tree(files, Mode::Formatting)?;
    let mut minted = 0;
    let mut new_files = Vec::new();
    for (path, (role, content)) in &mut tree.files {
        let records: Vec<&mut Rec> = match content {
            Content::Record(record) => vec![record],
            Content::Registry(entries) => entries.iter_mut().collect(),
            Content::Lint(_) => Vec::new(),
        };
        for record in records {
            let top_level_new = record.text("key").is_none();
            minted += mint_keys(record);
            supply_slug(record, role);
            if top_level_new && matches!(role, Role::Product { .. } | Role::Feature { .. }) {
                new_files.push((
                    path.clone(),
                    role.clone(),
                    record.text("key").map(str::to_owned),
                ));
            }
        }
    }
    supply_group_slugs(&mut tree);
    for (_, role, key) in new_files {
        if let Some(key) = key {
            list_new_file(&mut tree, &role, key);
        }
    }
    canonicalize(&mut tree);
    let changed = write_files(&tree)
        .into_iter()
        .filter(|(path, bytes)| files.get(path) != Some(bytes))
        .collect();
    Ok(Formatted { changed, minted })
}

fn scalar(text: String) -> Val {
    Val::Scalar {
        text,
        position: Position::new(1, 1),
    }
}

/// Mints a version 7 key for every keyed record without one, the record and everything in it
/// (RQ-3, RQ-218), returning how many were minted.
fn mint_keys(record: &mut Rec) -> usize {
    let mut minted = 0;
    if record.kind.is_keyed() && record.text("key").is_none() {
        record.set("key", scalar(Key::mint().to_string()));
        minted += 1;
    }
    for (_, value) in &mut record.fields {
        for child in value.records_mut() {
            minted += mint_keys(child);
        }
    }
    minted
}

/// A file's record without a slug takes the name of its file or directory (RQ-213).
fn supply_slug(record: &mut Rec, role: &Role) {
    let name = match role {
        Role::Product { product } => product,
        Role::Feature { feature, .. } => feature,
        Role::Component { slug } | Role::Decision { slug } => slug,
        _ => return,
    };
    if record.text("slug").is_none() {
        record.set("slug", scalar(name.clone()));
    }
}

/// A Feature Group without a slug takes the directory its listed Features sit in, or, with none
/// there, the slug its name derives, unique among its Product's groups (AC-819, AC-820).
fn supply_group_slugs(tree: &mut Tree) {
    let feature_dirs: BTreeMap<String, String> = tree
        .records(|role| matches!(role, Role::Feature { .. }))
        .filter_map(|(_, role, record)| match role {
            Role::Feature { group, .. } => Some((record.text("key")?.to_owned(), group.clone())),
            _ => None,
        })
        .collect();
    for (_, content) in tree.files.values_mut() {
        let Content::Record(product) = content else {
            continue;
        };
        let Some((_, Val::List(groups))) = product
            .fields
            .iter_mut()
            .find(|(name, _)| *name == "featureGroups")
        else {
            continue;
        };
        let mut taken: BTreeSet<Slug> = groups
            .iter()
            .flat_map(Val::records)
            .filter_map(|group| Slug::parse(group.text("slug")?).ok())
            .collect();
        for group in groups.iter_mut().flat_map(Val::records_mut) {
            if group.text("slug").is_some() {
                continue;
            }
            let from_files = group
                .list("features")
                .iter()
                .find_map(|feature| feature_dirs.get(feature.text()?).cloned());
            let slug = from_files.unwrap_or_else(|| {
                let name = group.text("name").unwrap_or_default();
                Slug::derive(name, "feature-group")
                    .unique_in(&mut taken)
                    .to_string()
            });
            group.set("slug", scalar(slug));
        }
    }
}

/// Lists a file whose key was just minted: a Product in the manifest, a Feature in the Feature
/// Group whose directory holds it.
fn list_new_file(tree: &mut Tree, role: &Role, key: String) {
    let (lister, field, group_slug) = match role {
        Role::Product { .. } => (SpecPath::new(MANIFEST), "products", None),
        Role::Feature { product, group, .. } => (
            SpecPath::new(&format!("products/{product}/product.yaml")),
            "features",
            Some(group.as_str()),
        ),
        _ => return,
    };
    let Some((_, Content::Record(record))) = tree.files.get_mut(&lister) else {
        return;
    };
    let target = match group_slug {
        None => Some(record),
        Some(slug) => record
            .fields
            .iter_mut()
            .filter(|(name, _)| *name == "featureGroups")
            .flat_map(|(_, value)| value.records_mut())
            .find(|group| group.text("slug") == Some(slug)),
    };
    if let Some(target) = target {
        let mut items = target.list(field).to_vec();
        items.push(scalar(key));
        target.set(field, Val::List(items));
    }
}
