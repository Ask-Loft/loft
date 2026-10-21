//! A document held in memory, checked, placed and published without writing and reading its
//! files back: validating without linting, linting without validating, the sites of the files the
//! writer writes, and a publishable tree. Each equals what writing the document and reading the
//! files back gives, for a generated document, which reads, and for this repository's own
//! `spec/`; the publishable tree also for documents whose written files the reader refuses or
//! reads otherwise: edited at random, holding conflict sides of any text, and each way listed.

use std::fs;
use std::path::Path;

use loftspec::check::{Checked, check, linted, validated};
use loftspec::codec::{self, Files, LINT, SpecPath, read_sited};
use loftspec::conflict::{Conflict, ConflictOwner, ConflictValue, FieldPath};
use loftspec::document::{Document, Edge};
use loftspec::ids::{ComponentId, ElementId, FeatureId, RequirementId};
use loftspec::publish::{Made, PublishedAt, ReadTree, publish};
use loftspec::yaml::{Location, Position};
use proptest::prelude::*;
use proptest::test_runner::TestCaseError;

use crate::common::doc_edits::{doc_edit, doc_edited};
use crate::common::generated::{generated, rich_document};
use crate::common::{key, small_document};

/// A lint configuration that turns a rule off and raises the severity of another.
const LINT_TEXT: &str =
    "rules:\n  - rule: LQ-001\n    enabled: false\n  - rule: SH-001\n    severity: error\n";

/// Every file of this repository's own `spec/`, by its path under `spec/`.
fn own_spec() -> Files {
    fn walk(root: &Path, dir: &Path, files: &mut Files) {
        for entry in fs::read_dir(dir).unwrap() {
            let path = entry.unwrap().path();
            let hidden = path.file_name().unwrap().to_str().unwrap().starts_with('.');
            if hidden {
                continue;
            }
            if path.is_dir() {
                walk(root, &path, files);
            } else {
                let relative = path.strip_prefix(root).unwrap().to_str().unwrap();
                files.insert(SpecPath::new(relative), fs::read(&path).unwrap());
            }
        }
    }
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../spec");
    let mut files = Files::new();
    walk(&root, &root, &mut files);
    files
}

fn own_document() -> Document {
    codec::read(&own_spec()).expect("the repository's own spec reads")
}

/// The files the writer writes for `document`, with `spec/lint.yaml` holding `lint`.
fn written(document: &Document, lint: Option<&str>) -> Files {
    let mut files = codec::write(document);
    if let Some(text) = lint {
        files.insert(SpecPath::new(LINT), text.as_bytes().to_vec());
    }
    files
}

/// Asserts that every function on the document held equals the write-and-read result.
fn agrees(document: &Document, lint: Option<&str>) -> Result<(), TestCaseError> {
    let files = written(document, lint);
    let (read, sites) = read_sited(&files).expect("the written files read");
    prop_assert_eq!(&read, document);
    let (written_files, held_sites) = codec::write_sited(document);
    prop_assert_eq!(&written_files, &codec::write(document));
    prop_assert_eq!(&held_sites, &sites);
    prop_assert_eq!(&codec::sites(document), &sites);
    let Checked::Read {
        invalid,
        config,
        findings,
    } = check(&files)
    else {
        return Err(TestCaseError::fail("the written files read"));
    };
    prop_assert_eq!(validated(document, &held_sites), invalid);
    let lint_bytes = lint.map(str::as_bytes);
    match linted(document, &held_sites, lint_bytes) {
        Ok(held) => prop_assert_eq!(held, findings),
        Err(errors) => prop_assert_eq!(errors, config),
    }
    tree_agrees(document, lint, &files)
}

/// Asserts that a tree built from the document held equals the written files read back, or is
/// refused with the same read errors.
fn tree_agrees(
    document: &Document,
    lint: Option<&str>,
    files: &Files,
) -> Result<(), TestCaseError> {
    let (held, read) = (ReadTree::of_document(document, lint), ReadTree::read(files));
    let (held, read) = match (held, read) {
        (Ok(held), Ok(read)) => (held, read),
        (held, read) => {
            prop_assert_eq!(held.err(), read.err());
            return Ok(());
        }
    };
    prop_assert_eq!(held.document(), read.document());
    prop_assert_eq!(held.sites(), read.sites());
    prop_assert_eq!(held.roots(), read.roots());
    let at = PublishedAt::from_unix_seconds(1_800_000_000).unwrap();
    let published = |tree| publish(tree, None, None, at, Made::NotAsRemote);
    prop_assert_eq!(published(held), published(read));
    Ok(())
}

/// Conflict side values of a scalar field, some of which the reader refuses for a field: a key,
/// `first`, a tag name, `null`, and short text that may be empty or span lines. The writer writes
/// only a side in its field's shape, so no other shape is given.
fn conflict_value() -> impl Strategy<Value = ConflictValue> {
    let scalar = prop_oneof![
        Just(key(5).to_string()),
        Just("first".to_owned()),
        Just("portable".to_owned()),
        "[a-z \n]{0,6}",
    ]
    .prop_map(ConflictValue::Scalar);
    prop_oneof![4 => scalar, 1 => Just(ConflictValue::Deleted)]
}

proptest! {
    #[test]
    fn a_tree_built_from_any_edited_document_held_is_its_written_files_read_back(
        edits in proptest::collection::vec(doc_edit(), 0..=8),
        configured in any::<bool>(),
    ) {
        let document = doc_edited(&rich_document(), &edits);
        let lint = configured.then_some(LINT_TEXT);
        tree_agrees(&document, lint, &written(&document, lint))?;
    }

    #[test]
    fn a_tree_built_from_a_document_held_with_any_conflict_sides_is_its_written_files_read_back(
        field in prop_oneof![
            Just("parent"), Just("position"), Just("record"), Just("body"),
            Just("statement.shall"),
        ],
        sides in proptest::collection::vec(proptest::option::of(conflict_value()), 3),
    ) {
        let mut document = small_document();
        document.conflicts.push(Conflict {
            owner: ConflictOwner::Element(ElementId::Node(RequirementId::new(key(4)).into())),
            field: FieldPath::new(field),
            base: sides[0].clone(),
            ours: sides[1].clone(),
            theirs: sides[2].clone(),
        });
        tree_agrees(&document, None, &written(&document, None))?;
    }

    #[test]
    fn a_generated_document_held_checks_places_and_publishes_as_its_files_read_back(
        document in generated(),
        configured in any::<bool>(),
    ) {
        prop_assume!(document.is_some());
        let lint = configured.then_some(LINT_TEXT);
        agrees(&document.unwrap(), lint)?;
    }
}

#[test]
fn this_repositorys_own_spec_held_checks_places_and_publishes_as_its_files_read_back() {
    let document = own_document();
    agrees(&document, None).unwrap();
    agrees(&document, Some(LINT_TEXT)).unwrap();
}

#[test]
fn each_validation_error_of_a_held_document_names_the_sites_its_caller_gives() {
    let mut document = small_document();
    let requirements = &mut document.products[0].feature_groups[0].features[0].requirements;
    requirements.push(requirements[0].clone());
    let sites = codec::sites(&small_document());
    let invalid = validated(&document, &sites);
    assert_ne!(invalid, []);
    for placed in &invalid {
        let expected: Vec<_> = placed
            .error
            .keys()
            .into_iter()
            .filter_map(|key| sites.get(key).cloned())
            .collect();
        assert_eq!(placed.sites, expected, "{placed}");
    }
}

/// The small document with a Product body the linter reports.
fn reported() -> Document {
    let mut document = small_document();
    document.products[0].details.body = Some("The writer MUST write a key.".to_owned());
    document
}

#[test]
fn validating_a_held_document_gives_no_lint_finding() {
    let document = reported();
    let sites = codec::sites(&document);
    assert_ne!(linted(&document, &sites, None).unwrap(), []);
    assert_eq!(validated(&document, &sites), []);
}

#[test]
fn a_held_document_the_validator_refuses_is_linted_all_the_same() {
    let mut document = reported();
    document.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: ComponentId::new(key(999)),
    });
    let sites = codec::sites(&reported());
    assert_ne!(validated(&document, &sites), []);
    let refused = linted(&document, &sites, None).unwrap();
    let clean = linted(&reported(), &sites, None).unwrap();
    assert_ne!(clean, []);
    for finding in &clean {
        assert!(refused.contains(finding), "{finding:?}");
    }
}

#[test]
fn a_lint_configuration_in_error_gives_its_errors_and_no_finding() {
    let document = small_document();
    let sites = codec::sites(&document);
    let errors = linted(
        &document,
        &sites,
        Some(b"rules:\n  - rule: NOPE-1\n    enabled: false\n"),
    )
    .expect_err("the configuration is in error");
    assert_ne!(errors, []);
    let mut files = written(&document, None);
    files.insert(
        SpecPath::new(LINT),
        b"rules:\n  - rule: NOPE-1\n    enabled: false\n".to_vec(),
    );
    let Checked::Read {
        config, findings, ..
    } = check(&files)
    else {
        panic!("the tree reads");
    };
    assert_eq!(errors, config);
    assert_eq!(findings, []);
}

#[test]
fn each_held_site_names_the_file_and_the_line_and_column_of_the_keys_value() {
    let document = small_document();
    let files = codec::write(&document);
    let path = SpecPath::new("products/example-product/identity/keys.yaml");
    let text = String::from_utf8(files[&path].clone()).unwrap();
    let needle = format!("key: {}", key(5));
    let (line, row) = text
        .lines()
        .enumerate()
        .find(|(_, row)| row.contains(&needle))
        .expect("the criterion's key");
    let column = row.find(&needle).unwrap() + "key: ".len() + 1;
    let site = codec::sites(&document)
        .get(key(5))
        .cloned()
        .expect("a site");
    assert_eq!(site.path, path);
    assert_eq!(
        site.location,
        Some(Location::At(Position::new(line + 1, column)))
    );
}

#[test]
fn a_tree_built_from_a_held_document_holds_its_written_files_and_its_lint_text() {
    let document = small_document();
    let tree = ReadTree::of_document(&document, Some(LINT_TEXT)).unwrap();
    let files = written(&document, Some(LINT_TEXT));
    let at = PublishedAt::from_unix_seconds(1_800_000_000).unwrap();
    let published = publish(tree, None, None, at, Made::NotAsRemote).unwrap();
    let expected = publish(
        ReadTree::read(&files).unwrap(),
        None,
        None,
        at,
        Made::NotAsRemote,
    );
    assert_eq!(Ok(published), expected);
}

#[test]
fn a_tree_built_from_a_held_document_holds_it_in_the_order_the_reader_reads_it() {
    let mut document = small_document();
    document.tags.reverse();
    document.edges.reverse();
    let read = codec::read(&codec::write(&document)).unwrap();
    assert_ne!(read, document);
    let tree = ReadTree::of_document(&document, None).unwrap();
    assert_eq!(tree.document(), &read);
}

/// The small document with a conflict over `field` of the Requirement keyed 4 whose ours side
/// is `ours`.
fn with_conflict(field: &str, ours: &str) -> Document {
    let mut document = small_document();
    document.conflicts.push(Conflict {
        owner: ConflictOwner::Element(ElementId::Node(RequirementId::new(key(4)).into())),
        field: FieldPath::new(field),
        base: None,
        ours: Some(ConflictValue::Scalar(ours.to_owned())),
        theirs: None,
    });
    document
}

/// The ways a document held in memory may hold what its written files cannot be read as.
fn unreadable() -> Vec<(&'static str, Document)> {
    let mut keyless = small_document();
    keyless.products[0].feature_groups[0].features[0].requirements[0].criteria[0].id = None;
    let mut dangling = small_document();
    dangling.edges.push(Edge::Realizes {
        feature: FeatureId::new(key(3)),
        component: ComponentId::new(key(999)),
    });
    let mut twice = small_document();
    let requirements = &mut twice.products[0].feature_groups[0].features[0].requirements;
    requirements.push(requirements[0].clone());
    let mut empty_ext = small_document();
    let empty = loftspec::ext::ExtValue::List(Vec::new());
    empty_ext.products[0].details.ext = std::collections::BTreeMap::from([(
        loftspec::names::Namespace::parse("tool").unwrap(),
        empty,
    )]);
    let mut branch = small_document();
    branch.publish_branch = Some(String::new());
    let mut sequence = small_document();
    sequence.derived_from = Some(loftspec::document::DerivedFrom {
        authored_state_id: loftspec::ids::AuthoredStateId::new(key(80)),
        content_address: loftspec::canonical::Digest::from_bytes([1; 32]),
        publish_sequence: 0,
    });
    let mut blank = small_document();
    blank.terms = vec![crate::common::term(70, " ", &[])];
    let parent = with_conflict("parent", "x");
    let position = with_conflict("position", "");
    vec![
        ("a node without a key", keyless),
        ("a reference to no element", dangling),
        ("a key carried twice", twice),
        ("an empty extension list", empty_ext),
        ("an empty publish branch", branch),
        ("a publish sequence of zero", sequence),
        ("a blank term name", blank),
        ("a parent conflict whose side names no key", parent),
        ("a position conflict whose side is empty", position),
    ]
}

#[test]
fn a_held_document_whose_written_files_do_not_read_is_refused_with_their_read_errors() {
    for (case, document) in unreadable() {
        let files = written(&document, None);
        let expected = ReadTree::read(&files).err();
        assert!(expected.is_some(), "{case}");
        assert_eq!(
            ReadTree::of_document(&document, None).err(),
            expected,
            "{case}"
        );
    }
}
