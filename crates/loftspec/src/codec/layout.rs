//! Reading a whole tree in two phases. Parsing reads the manifest first (RQ-18, RQ-67), then
//! every file as its record. Checking applies the rules that span files: Products as the
//! manifest lists them (RQ-175), Features as their Feature Groups list them (RQ-28, RQ-212),
//! slugs that match their names (RQ-211) and keys carried once (RQ-4). Checking is always
//! strict; the formatter checks only after it has supplied what was missing.

use super::schema::field;
use std::collections::{BTreeMap, BTreeSet};

use super::error::{Problem, ReadError, Site, SpecPath};
use super::files::{Files, MANIFEST, Role, Tree, keyed, read_file};
use super::record::{Mode, Rec, Val};
use crate::identity::Key;
use crate::manifest::check_identifiers;
use crate::yaml::{Location, Position};

/// Reads and checks every file of a tree, as every role but the formatter does.
///
/// # Errors
///
/// Every error found: a refused manifest alone, since no other file is read then (AC-70);
/// otherwise each file's first error, and the rules across files when every file reads.
pub fn read_tree(files: &Files) -> Result<Tree, Vec<ReadError>> {
    let tree = parse_files(files, Mode::Strict)?;
    check_tree(&tree)?;
    Ok(tree)
}

/// Reads every file as the record its path names, the manifest first.
///
/// # Errors
///
/// A refused manifest alone, or each file's first error.
pub(crate) fn parse_files(files: &Files, mode: Mode) -> Result<Tree, Vec<ReadError>> {
    let manifest_path = SpecPath::new(MANIFEST);
    let Some(bytes) = files.get(&manifest_path) else {
        return Err(vec![ReadError::at(
            &manifest_path,
            None,
            Problem::MissingManifest,
        )]);
    };
    let manifest = read_file(&manifest_path, bytes, mode).map_err(|error| vec![error])?;
    if let Some(record) = manifest.1.records().next() {
        let format = record.text(field::FORMAT).unwrap_or_default();
        let algorithm = record.text(field::ALGORITHM).unwrap_or_default();
        if let Err(error) = check_identifiers(format, algorithm) {
            let problem = Problem::Unsupported(error);
            let position = Some(record.position);
            return Err(vec![ReadError::at(&manifest_path, position, problem)]);
        }
    }
    let mut tree = Tree::default();
    let mut errors = Vec::new();
    tree.files.insert(manifest_path, manifest);
    for (path, bytes) in files.iter().filter(|(path, _)| path.as_str() != MANIFEST) {
        match read_file(path, bytes, mode) {
            Ok(file) => {
                tree.files.insert(path.clone(), file);
            }
            Err(error) => errors.push(error),
        }
    }
    if errors.is_empty() {
        Ok(tree)
    } else {
        Err(errors)
    }
}

/// Applies the rules that span files.
///
/// # Errors
///
/// Every rule the tree breaks.
pub(crate) fn check_tree(tree: &Tree) -> Result<(), Vec<ReadError>> {
    let mut errors = Vec::new();
    check_slugs(tree, &mut errors);
    check_products(tree, &mut errors);
    check_features(tree, &mut errors);
    check_keys(tree, &mut errors);
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors)
    }
}

fn error(path: &SpecPath, position: Position, problem: Problem) -> ReadError {
    ReadError::at(path, Some(position), problem)
}

/// A checked key value.
fn key_of(value: &Val) -> Option<(Key, Position)> {
    match value {
        Val::Scalar { text, position } => Some((Key::parse(text).ok()?, *position)),
        _ => None,
    }
}

/// A record's slug must equal the name it fixes (RQ-211).
fn check_slugs(tree: &Tree, errors: &mut Vec<ReadError>) {
    for (path, role, record) in tree.records(|_| true) {
        let Some(name) = role.slug() else {
            continue;
        };
        if let Some(Val::Scalar { text, position }) = record.get(field::SLUG)
            && text != name
        {
            let problem = Problem::SlugMismatch {
                slug: text.clone(),
                name: name.to_owned(),
            };
            errors.push(error(path, *position, problem));
        }
    }
}

/// The keys a list field names, with their positions.
fn listed(record: &Rec, field: &str) -> Vec<(Key, Position)> {
    record.list(field).iter().filter_map(key_of).collect()
}

/// Checks one list of keys against the files that carry them: each listed key has a file
/// (AC-99, AC-699) and is listed once (AC-102, AC-701).
fn check_list(
    path: &SpecPath,
    keys: &[(Key, Position)],
    carried: &BTreeMap<Key, &SpecPath>,
    errors: &mut Vec<ReadError>,
) {
    let mut seen = BTreeSet::new();
    for &(key, position) in keys {
        if !seen.insert(key) {
            errors.push(error(path, position, Problem::ListedTwice(key)));
        } else if !carried.contains_key(&key) {
            errors.push(error(path, position, Problem::ListedWithoutFile(key)));
        }
    }
}

/// Products as the manifest lists them (AC-699..AC-701).
fn check_products(tree: &Tree, errors: &mut Vec<ReadError>) {
    let manifest_path = SpecPath::new(MANIFEST);
    let Some(manifest) = tree.record(&manifest_path) else {
        return;
    };
    let carried = carried(tree, |role| matches!(role, Role::Product { .. }));
    let keys = listed(manifest, field::PRODUCTS);
    check_list(&manifest_path, &keys, &carried, errors);
    report_unlisted(tree, &carried, &keys, "manifest", errors);
}

/// The file of a role that carries each key.
fn carried<'t>(tree: &'t Tree, wanted: impl Fn(&Role) -> bool + 't) -> BTreeMap<Key, &'t SpecPath> {
    tree.records(wanted)
        .filter_map(|(path, _, record)| Some((key_of(record.get(field::KEY)?)?.0, path)))
        .collect()
}

/// A file whose key no list names (AC-100, AC-700).
fn report_unlisted(
    tree: &Tree,
    carried: &BTreeMap<Key, &SpecPath>,
    keys: &[(Key, Position)],
    lister: &'static str,
    errors: &mut Vec<ReadError>,
) {
    let named: BTreeSet<Key> = keys.iter().map(|(key, _)| *key).collect();
    for (key, path) in carried {
        if !named.contains(key) {
            let position = tree.record(path).map(|record| record.position);
            errors.push(ReadError::at(path, position, Problem::NotListed(lister)));
        }
    }
}

/// Features as their Feature Groups list them, each in its group's directory (AC-99..AC-102,
/// RQ-212).
fn check_features(tree: &Tree, errors: &mut Vec<ReadError>) {
    let features = carried(tree, |role| matches!(role, Role::Feature { .. }));
    let mut lister = BTreeMap::new();
    let mut all_listed = Vec::new();
    for (path, role, product) in tree.records(|role| matches!(role, Role::Product { .. })) {
        let Some(product_dir) = role.slug() else {
            continue;
        };
        for group in product.records(field::FEATURE_GROUPS) {
            let group = Group {
                path,
                product_dir,
                slug: group.text(field::SLUG).unwrap_or_default(),
                at: group.position,
                keys: listed(group, field::FEATURES),
            };
            check_group(tree, &group, &features, &mut lister, errors);
            all_listed.extend(group.keys);
        }
    }
    report_unlisted(tree, &features, &all_listed, "Feature Group", errors);
}

/// One Feature Group's list of Features, in the Product file at `path`.
struct Group<'a> {
    path: &'a SpecPath,
    product_dir: &'a str,
    slug: &'a str,
    /// Where the group's record starts, which tells two groups apart.
    at: Position,
    keys: Vec<(Key, Position)>,
}

/// Checks one group's list. A key another group listed first is an error naming both groups
/// (AC-101), and only that: the Feature's directory answers to the first group alone.
fn check_group<'a>(
    tree: &Tree,
    group: &Group<'a>,
    features: &BTreeMap<Key, &SpecPath>,
    lister: &mut BTreeMap<Key, (&'a SpecPath, Position, &'a str)>,
    errors: &mut Vec<ReadError>,
) {
    check_list(group.path, &group.keys, features, errors);
    for &(key, position) in &group.keys {
        let this = (group.path, group.at, group.slug);
        let first = *lister.entry(key).or_insert(this);
        if (first.0, first.1) != (this.0, this.1) {
            let problem = Problem::ListedByTwoGroups {
                feature: key,
                first: first.2.to_owned(),
                second: group.slug.to_owned(),
            };
            errors.push(error(group.path, position, problem));
        } else if let Some(path) = features.get(&key) {
            check_directory(tree, path, group.product_dir, group.slug, errors);
        }
    }
}

/// A listed Feature's file must sit in its group's directory under its Product (RQ-212).
fn check_directory(
    tree: &Tree,
    path: &SpecPath,
    product_dir: &str,
    group_slug: &str,
    errors: &mut Vec<ReadError>,
) {
    if let Some((Role::Feature { product, group, .. }, _)) = tree.files.get(path)
        && (product != product_dir || group != group_slug)
    {
        let position = tree.record(path).map(|record| record.position);
        let problem = Problem::WrongDirectory(group_slug.to_owned());
        errors.push(ReadError::at(path, position, problem));
    }
}

/// Every key a keyed record of the tree carries, with the file and the position of the key.
fn carried_keys(tree: &Tree) -> impl Iterator<Item = (Key, &SpecPath, Position)> {
    tree.files.iter().flat_map(|(path, (_, content))| {
        let mut found = Vec::new();
        content
            .records()
            .for_each(|record| keyed(record, &mut found));
        found
            .into_iter()
            .filter_map(|record| key_of(record.get(field::KEY)?))
            .map(move |(key, position)| (key, path, position))
    })
}

fn site(path: &SpecPath, position: Position) -> Site {
    Site {
        path: path.clone(),
        location: Some(Location::At(position)),
    }
}

/// No two keyed records carry one key (RQ-4, AC-13, AC-14, AC-251, AC-848, AC-849).
fn check_keys(tree: &Tree, errors: &mut Vec<ReadError>) {
    let mut seen: BTreeMap<Key, (&SpecPath, Position)> = BTreeMap::new();
    for (key, path, position) in carried_keys(tree) {
        if let Some(&(first, at)) = seen.get(&key) {
            errors.push(ReadError {
                sites: vec![site(first, at), site(path, position)],
                problem: Problem::DuplicateKey(key),
            });
        } else {
            seen.insert(key, (path, position));
        }
    }
}

/// Where each key of a read tree is written: its file, and the position of its `key` field. A
/// validation error or a lint finding names an element at its site (RQ-158).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sites(BTreeMap<Key, Site>);

impl Sites {
    /// The sites of every key a tree carries.
    #[must_use]
    pub fn of(tree: &Tree) -> Self {
        Self(
            carried_keys(tree)
                .map(|(key, path, position)| (key, site(path, position)))
                .collect(),
        )
    }

    /// The site of `key`, if the tree carries it.
    #[must_use]
    pub fn get(&self, key: Key) -> Option<&Site> {
        self.0.get(&key)
    }
}
