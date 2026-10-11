//! Reading a whole tree: the manifest first (RQ-18, RQ-67), then every file, then the rules
//! that span files: Products as the manifest lists them (RQ-175), Features as their Feature
//! Groups list them (RQ-28, RQ-212), slugs that match their names (RQ-211) and keys carried
//! once (RQ-4).

use std::collections::BTreeMap;

use super::error::{Problem, ReadError, Site, SpecPath};
use super::files::{Content, Files, MANIFEST, Role, Tree, keyed, read_file};
use super::record::{Mode, Rec, Val};
use crate::manifest::check_identifiers;
use crate::yaml::{Location, Position};

/// Reads and checks every file of a tree.
///
/// # Errors
///
/// Every error found: a refused manifest alone, since no other file is read then (AC-70);
/// otherwise each file's first error, and the rules across files when every file reads.
pub fn read_tree(files: &Files, mode: Mode) -> Result<Tree, Vec<ReadError>> {
    let manifest_path = SpecPath::new(MANIFEST);
    let Some(bytes) = files.get(&manifest_path) else {
        return Err(vec![ReadError::at(
            &manifest_path,
            None,
            Problem::MissingManifest,
        )]);
    };
    let manifest = read_file(&manifest_path, bytes, mode).map_err(|error| vec![error])?;
    if let (_, Content::Record(record)) = &manifest {
        let format = record.text("format").unwrap_or_default();
        let algorithm = record.text("algorithm").unwrap_or_default();
        if check_identifiers(format, algorithm).is_err() {
            let unknown = if format == crate::FORMAT {
                algorithm
            } else {
                format
            };
            let problem = Problem::Unsupported(format!("`{unknown}`"));
            return Err(vec![ReadError::at(
                &manifest_path,
                Some(record.position),
                problem,
            )]);
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
        check_slugs(&tree, &mut errors);
        check_products(&tree, &mut errors);
        check_features(&tree, &mut errors);
        check_keys(&tree, &mut errors);
    }
    if errors.is_empty() {
        Ok(tree)
    } else {
        Err(errors)
    }
}

fn error(path: &SpecPath, position: Position, problem: Problem) -> ReadError {
    ReadError::at(path, Some(position), problem)
}

/// A record's slug, when it has one, must equal the name it fixes (RQ-211).
fn check_slugs(tree: &Tree, errors: &mut Vec<ReadError>) {
    for (path, role, record) in tree.records(|_| true) {
        let name = match role {
            Role::Product { product } => product,
            Role::Feature { feature, .. } => feature,
            Role::Component { slug } | Role::Decision { slug } => slug,
            _ => continue,
        };
        if let Some(Val::Scalar { text, position }) = record.get("slug")
            && text != name
        {
            let problem = Problem::SlugMismatch {
                slug: text.clone(),
                name: name.clone(),
            };
            errors.push(error(path, *position, problem));
        }
    }
}

/// The keys a list field names, with their positions.
fn listed(record: &Rec, field: &str) -> Vec<(String, Position)> {
    record
        .list(field)
        .iter()
        .filter_map(|value| match value {
            Val::Scalar { text, position } => Some((text.clone(), *position)),
            _ => None,
        })
        .collect()
}

/// Checks one list of keys against the files that carry them: each listed key has a file
/// (AC-99, AC-699) and is listed once (AC-102, AC-701).
fn check_list(
    path: &SpecPath,
    keys: &[(String, Position)],
    carried: &BTreeMap<String, SpecPath>,
    errors: &mut Vec<ReadError>,
) {
    let mut seen = BTreeMap::new();
    for (key, position) in keys {
        if seen.insert(key.clone(), *position).is_some() {
            errors.push(error(path, *position, Problem::ListedTwice(key.clone())));
        } else if !carried.contains_key(key) {
            errors.push(error(
                path,
                *position,
                Problem::ListedWithoutFile(key.clone()),
            ));
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
    let keys = listed(manifest, "products");
    check_list(&manifest_path, &keys, &carried, errors);
    report_unlisted(tree, &carried, &keys, "manifest", errors);
}

/// The key each file of a role carries, by key.
fn carried(tree: &Tree, wanted: impl Fn(&Role) -> bool) -> BTreeMap<String, SpecPath> {
    tree.records(wanted)
        .filter_map(|(path, _, record)| Some((record.text("key")?.to_owned(), path.clone())))
        .collect()
}

/// A file whose key no list names (AC-100, AC-700). A file without a key, which only the
/// formatter reads, is not reported: the formatter adds it to its list.
fn report_unlisted(
    tree: &Tree,
    carried: &BTreeMap<String, SpecPath>,
    keys: &[(String, Position)],
    lister: &'static str,
    errors: &mut Vec<ReadError>,
) {
    for (key, path) in carried {
        if !keys.iter().any(|(listed, _)| listed == key) {
            let position = tree.record(path).map(|record| record.position);
            errors.push(ReadError::at(path, position, Problem::NotListed(lister)));
        }
    }
}

/// Features as their Feature Groups list them, each in its group's directory (AC-99..AC-102,
/// RQ-212).
fn check_features(tree: &Tree, errors: &mut Vec<ReadError>) {
    let features = carried(tree, |role| matches!(role, Role::Feature { .. }));
    let mut lister: BTreeMap<String, String> = BTreeMap::new();
    let mut all_listed = Vec::new();
    for (path, role, product) in tree.records(|role| matches!(role, Role::Product { .. })) {
        let Role::Product {
            product: product_dir,
        } = role
        else {
            continue;
        };
        for group in product.list("featureGroups").iter().flat_map(Val::records) {
            let group = Group {
                path,
                product_dir,
                slug: group.text("slug").unwrap_or_default(),
                keys: listed(group, "features"),
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
    keys: Vec<(String, Position)>,
}

fn check_group(
    tree: &Tree,
    group: &Group<'_>,
    features: &BTreeMap<String, SpecPath>,
    lister: &mut BTreeMap<String, String>,
    errors: &mut Vec<ReadError>,
) {
    check_list(group.path, &group.keys, features, errors);
    for (key, position) in &group.keys {
        let first = lister.insert(key.clone(), group.slug.to_owned());
        if first.is_some_and(|first| first != group.slug) {
            let problem = Problem::ListedByTwoGroups(key.clone());
            errors.push(error(group.path, *position, problem));
        }
        check_directory(
            tree,
            features.get(key),
            group.product_dir,
            group.slug,
            errors,
        );
    }
}

/// A listed Feature's file must sit in its group's directory under its Product (RQ-212).
fn check_directory(
    tree: &Tree,
    file: Option<&SpecPath>,
    product_dir: &str,
    group_slug: &str,
    errors: &mut Vec<ReadError>,
) {
    let Some(path) = file else {
        return;
    };
    if let Some((Role::Feature { product, group, .. }, _)) = tree.files.get(path)
        && (product != product_dir || group != group_slug)
    {
        let position = tree.record(path).map(|record| record.position);
        let problem = Problem::WrongDirectory(group_slug.to_owned());
        errors.push(ReadError::at(path, position, problem));
    }
}

/// No two keyed records carry one key (RQ-4, AC-13, AC-14, AC-251, AC-848, AC-849).
fn check_keys(tree: &Tree, errors: &mut Vec<ReadError>) {
    let mut seen: BTreeMap<&str, Site> = BTreeMap::new();
    for (path, (_, content)) in &tree.files {
        let mut found = Vec::new();
        match content {
            Content::Record(record) => keyed(record, &mut found),
            Content::Registry(entries) => entries.iter().for_each(|entry| keyed(entry, &mut found)),
            Content::Lint(_) => {}
        }
        for record in found {
            let Some(Val::Scalar { text, position }) = record.get("key") else {
                continue;
            };
            let site = Site {
                path: path.clone(),
                location: Some(Location::At(*position)),
            };
            if let Some(first) = seen.get(text.as_str()) {
                errors.push(ReadError {
                    sites: vec![first.clone(), site],
                    problem: Problem::DuplicateKey(text.clone()),
                });
            } else {
                seen.insert(text, site);
            }
        }
    }
}
