//! `loft fmt` as a function (D-11 F-39): the tree parsed as the formatter parses it, every
//! missing key minted and every missing slug supplied, the tree then checked as strictly as any
//! role checks it, and the files whose bytes change in canonical layout. It writes nothing
//! itself; writing belongs to a store (CMP-4).

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::{
    Files, MANIFEST, Mode, ReadError, Rec, Role, SpecPath, Tree, Val, canonicalize, check_tree,
    parse_files, write_files,
};
use crate::identity::Key;
use crate::slug::Slug;

/// What formatting a tree changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Formatted {
    /// Each file whose bytes change, with its new bytes (AC-534, AC-535).
    pub changed: Files,
    /// How many keys were minted (AC-536).
    pub minted: usize,
}

/// Formats a tree.
///
/// # Errors
///
/// Every read error of the tree, and every rule across files the supplied tree still breaks;
/// nothing is formatted then (RQ-141).
pub fn format(files: &Files) -> Result<Formatted, Vec<ReadError>> {
    let mut tree = parse_files(files, Mode::Formatting)?;
    let mut minted = 0;
    let mut new_files = Vec::new();
    for (role, content) in tree.files.values_mut() {
        for record in content.records_mut() {
            let unkeyed = record.kind.is_keyed() && record.get("key").is_none();
            minted += mint_keys(record);
            if let Some(slug) = role.slug()
                && record.get("slug").is_none()
            {
                record.set_text("slug", slug);
            }
            if unkeyed && matches!(role, Role::Product { .. } | Role::Feature { .. }) {
                new_files.push((role.clone(), key_of(record)));
            }
        }
    }
    supply_group_slugs(&mut tree);
    for (role, key) in new_files {
        list_new_file(&mut tree, &role, key.expect("a minted key"));
    }
    check_tree(&tree)?;
    canonicalize(&mut tree);
    let changed = write_files(&tree)
        .into_iter()
        .filter(|(path, bytes)| files.get(path) != Some(bytes))
        .collect();
    Ok(Formatted { changed, minted })
}

/// The key a record carries.
fn key_of(record: &Rec) -> Option<Key> {
    Key::parse(record.text("key")?).ok()
}

/// Mints a version 7 key for every keyed record without one, the record and everything in it
/// (RQ-3, RQ-218), returning how many were minted.
fn mint_keys(record: &mut Rec) -> usize {
    let mut minted = 0;
    if record.kind.is_keyed() && record.get("key").is_none() {
        record.set_text("key", Key::mint().to_string());
        minted += 1;
    }
    for (_, value) in &mut record.fields {
        for child in value.records_mut() {
            minted += mint_keys(child);
        }
    }
    minted
}

/// A Feature Group without a slug takes the directory its listed Features sit in, or, with none
/// there, the slug its name derives, unique among its Product's groups (AC-819, AC-820).
fn supply_group_slugs(tree: &mut Tree) {
    let feature_dirs: BTreeMap<Key, String> = tree
        .records(|role| matches!(role, Role::Feature { .. }))
        .filter_map(|(_, role, record)| match role {
            Role::Feature { group, .. } => Some((key_of(record)?, group.clone())),
            _ => None,
        })
        .collect();
    for (role, content) in tree.files.values_mut() {
        if !matches!(role, Role::Product { .. }) {
            continue;
        }
        for product in content.records_mut() {
            let Some(groups) = product.get_mut("featureGroups") else {
                continue;
            };
            let mut taken: BTreeSet<Slug> = groups
                .records()
                .filter_map(|group| Slug::parse(group.text("slug")?).ok())
                .collect();
            for group in groups
                .records_mut()
                .filter(|group| group.get("slug").is_none())
            {
                let slug = group_slug(group, &feature_dirs, &mut taken);
                group.set_text("slug", slug);
            }
        }
    }
}

/// The directory a group's listed Features sit in, or the slug its name derives.
fn group_slug(
    group: &Rec,
    feature_dirs: &BTreeMap<Key, String>,
    taken: &mut BTreeSet<Slug>,
) -> String {
    let from_files = group.list("features").iter().find_map(|feature| {
        let key = Key::parse(feature.text()?).ok()?;
        feature_dirs.get(&key).cloned()
    });
    from_files.unwrap_or_else(|| {
        let name = group.text("name").unwrap_or_default();
        Slug::derive(name, "feature-group")
            .unique_in(taken)
            .to_string()
    })
}

/// Lists a file whose key was just minted: a Product in the manifest, a Feature in the Feature
/// Group whose directory holds it. A file no list can take stays unlisted, and the check after
/// supplying reports it (AC-100, AC-700).
fn list_new_file(tree: &mut Tree, role: &Role, key: Key) {
    let (lister, group_slug) = match role {
        Role::Product { .. } => (SpecPath::new(MANIFEST), None),
        Role::Feature { product, group, .. } => {
            let product = product.clone();
            (Role::Product { product }.path(), Some(group.as_str()))
        }
        _ => return,
    };
    let Some(record) = tree
        .files
        .get_mut(&lister)
        .and_then(|(_, content)| content.records_mut().next())
    else {
        return;
    };
    let target = match group_slug {
        None => Some((record, "products")),
        Some(slug) => record
            .get_mut("featureGroups")
            .and_then(|groups| {
                groups
                    .records_mut()
                    .find(|group| group.text("slug") == Some(slug))
            })
            .map(|group| (group, "features")),
    };
    if let Some((target, field)) = target {
        target.list_mut(field).push(Val::scalar(&key));
    }
}
