//! `loft fmt` as a function (F-39): the tree parsed as the formatter parses it, every
//! missing key minted and every missing slug supplied, the tree then checked as strictly as any
//! role checks it, and the files whose bytes change in canonical layout. It writes nothing
//! itself; writing belongs to a store (CMP-4).

use std::collections::{BTreeMap, BTreeSet};

use crate::codec::schema::{Supply, field};

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
/// nothing is formatted then (Reading the tree).
pub fn format(files: &Files) -> Result<Formatted, Vec<ReadError>> {
    let mut tree = parse_files(files, Mode::Formatting)?;
    let mut supplier = Supplier {
        feature_dirs: feature_dirs(&tree),
        minted: 0,
    };
    let mut new_files = Vec::new();
    for (role, content) in tree.files.values_mut() {
        for record in content.records_mut() {
            let unkeyed = record.kind.is_keyed() && record.get(field::KEY).is_none();
            supplier.supply(record, role.slug());
            if unkeyed && matches!(role, Role::Product { .. } | Role::Feature { .. }) {
                new_files.push((role.clone(), key_of(record)));
            }
        }
    }
    for (role, key) in new_files {
        list_new_file(&mut tree, &role, key.expect("a minted key"));
    }
    check_tree(&tree)?;
    canonicalize(&mut tree);
    let changed = write_files(&tree)
        .into_iter()
        .filter(|(path, bytes)| files.get(path) != Some(bytes))
        .collect();
    Ok(Formatted {
        changed,
        minted: supplier.minted,
    })
}

/// The key a record carries.
fn key_of(record: &Rec) -> Option<Key> {
    Key::parse(record.text(field::KEY)?).ok()
}

/// The directory each Feature file sits in, by the Feature's key.
fn feature_dirs(tree: &Tree) -> BTreeMap<Key, String> {
    tree.records(|role| matches!(role, Role::Feature { .. }))
        .filter_map(|(_, role, record)| match role {
            Role::Feature { group, .. } => Some((key_of(record)?, group.clone())),
            _ => None,
        })
        .collect()
}

/// Fills every required field a record lacks by the rule its schema gives (RQ-3, RQ-59,
/// RQ-213, RQ-218).
struct Supplier {
    feature_dirs: BTreeMap<Key, String>,
    /// How many keys were minted (AC-536).
    minted: usize,
}

impl Supplier {
    /// Supplies a record and everything in it; `path_slug` is the name of the file or
    /// directory a file's record takes its slug from.
    fn supply(&mut self, record: &mut Rec, path_slug: Option<&str>) {
        let kind = record.kind;
        for spec in kind.fields() {
            if record.get(spec.name).is_some() {
                continue;
            }
            match (spec.supply, path_slug) {
                (Some(Supply::MintKey), _) => {
                    record.set_text(spec.name, Key::mint().to_string());
                    self.minted += 1;
                }
                (Some(Supply::SlugFromPath), Some(slug)) => record.set_text(spec.name, slug),
                // A slug from the name is supplied with its siblings, which it must differ from.
                _ => {}
            }
        }
        for (_, value) in &mut record.fields {
            if let Val::List(items) = value {
                self.supply_siblings(items);
            }
            for child in value.records_mut() {
                self.supply(child, None);
            }
        }
    }

    /// Fills the slug of each sibling that takes its slug from its name, unique among them
    /// (AC-819, AC-820).
    fn supply_siblings(&self, items: &mut [Val]) {
        let slug_field = |record: &Rec| {
            let fields = record.kind.fields().iter();
            let mut named = fields.filter(|spec| spec.supply == Some(Supply::SlugFromName));
            named.next().map(|spec| spec.name)
        };
        let mut taken: BTreeSet<Slug> = items
            .iter()
            .flat_map(Val::records)
            .filter_map(|record| Slug::parse(record.text(slug_field(record)?)?).ok())
            .collect();
        for record in items.iter_mut().flat_map(Val::records_mut) {
            if let Some(name) = slug_field(record)
                && record.get(name).is_none()
            {
                let slug = group_slug(record, &self.feature_dirs, &mut taken);
                record.set_text(name, slug);
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
    let from_files = group.list(field::FEATURES).iter().find_map(|feature| {
        let key = Key::parse(feature.text()?).ok()?;
        feature_dirs.get(&key).cloned()
    });
    from_files.unwrap_or_else(|| {
        let name = group.text(field::NAME).unwrap_or_default();
        Slug::derive(name, "feature-group")
            .unique_in(taken)
            .to_string()
    })
}

/// Lists a file whose key was just minted (RQ-219): a Product in the manifest (AC-850), a Feature
/// in the Feature Group whose directory holds it (AC-851), after the keys already listed
/// (AC-852). A file no list can take stays unlisted, and the check after supplying reports it
/// (AC-853, AC-854).
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
        None => Some((record, field::PRODUCTS)),
        Some(slug) => record
            .get_mut(field::FEATURE_GROUPS)
            .and_then(|groups| {
                groups
                    .records_mut()
                    .find(|group| group.text(field::SLUG) == Some(slug))
            })
            .map(|group| (group, field::FEATURES)),
    };
    if let Some((target, field)) = target {
        target.list_mut(field).push(Val::scalar(&key));
    }
}
