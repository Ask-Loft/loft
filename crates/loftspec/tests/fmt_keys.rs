//! The key a node keeps through each edit the formatter sees, and the records it mints a key for
//! when one is missing.

use crate::common::key;
use crate::common::text::{FEATURE, PRODUCT, edited, files, path, text};
use crate::fmt::formatted;
use loftspec::codec::{self, Files, Problem};
use loftspec::document::Document;
use loftspec::fmt::format;
use loftspec::identity::{Key, Label};
use loftspec::names::TagName;

const COMPONENT: &str = "components/document-model.yaml";
const DECISION: &str = "decisions/keys-are-uuids.yaml";

/// One edit: the first `from` replaced by `to` in the file at `at`.
type Edit = (&'static str, &'static str, &'static str);

/// The small document's files with every edit made.
fn edited_all(edits: &[Edit]) -> Files {
    let mut files = files();
    for (at, from, to) in edits {
        let current = text(&files, at);
        assert!(current.contains(from), "{from:?} in {at}");
        files.insert(path(at), current.replacen(from, to, 1).into_bytes());
    }
    files
}

/// The document the formatted tree reads as, after asserting that formatting minted no key.
fn read_formatted(files: &Files) -> Document {
    assert_eq!(format(files).unwrap().minted, 0);
    codec::read(&formatted(files)).unwrap_or_else(|errors| panic!("{errors:?}"))
}

/// Asserts that `minted` is a version 7 key other than the key `before` it replaced.
fn assert_minted(minted: Key, before: u32) {
    assert_eq!(minted.version(), 7);
    assert_ne!(minted, key(before));
}

#[test]
fn a_nodes_key_is_unchanged_after_its_body_changes() {
    let files = edited(
        FEATURE,
        "body: \"Keys address nodes.\"",
        "body: \"Keys address every node.\"",
    );
    let document = read_formatted(&files);
    let feature = &document.products[0].feature_groups[0].features[0];
    assert_eq!(
        feature.details.body.as_deref(),
        Some("Keys address every node.")
    );
    assert_eq!(feature.id.unwrap().key(), key(3));
}

#[test]
fn a_nodes_key_is_unchanged_after_it_moves_to_another_parent() {
    let mut files = files();
    let feature = text(&files, FEATURE);
    let (kept, moved) = feature.split_at(feature.find("qualityAttributes:\n").unwrap());
    let item = moved.strip_prefix("qualityAttributes:\n").unwrap();
    let component = text(&files, COMPONENT) + item;
    files.insert(path(FEATURE), kept.as_bytes().to_vec());
    files.insert(path(COMPONENT), component.into_bytes());
    let document = read_formatted(&files);
    let feature = &document.products[0].feature_groups[0].features[0];
    assert_eq!(feature.quality_attributes, Vec::new());
    let qualities = &document.components[0].component.quality_attributes;
    let names: Vec<(&str, Key)> = qualities
        .iter()
        .map(|quality| (quality.name.as_str(), quality.id.unwrap().key()))
        .collect();
    assert_eq!(
        names,
        [
            ("Formatting completes quickly.", key(8)),
            ("Minting a key completes quickly.", key(10)),
        ]
    );
}

#[test]
fn a_nodes_key_is_unchanged_after_its_tags_change() {
    let files = edited_all(&[
        (PRODUCT, "tags:\n  - portable\n", "tags:\n  - identity\n"),
        (
            FEATURE,
            "body: \"Keys address nodes.\"\n",
            "body: \"Keys address nodes.\"\ntags:\n  - identity\n",
        ),
    ]);
    let document = read_formatted(&files);
    let product = &document.products[0];
    let feature = &product.feature_groups[0].features[0];
    for (tags, id, n) in [
        (&product.details.tags, product.id.unwrap().key(), 1),
        (&feature.details.tags, feature.id.unwrap().key(), 3),
    ] {
        let names: Vec<&str> = tags.iter().map(TagName::as_str).collect();
        assert_eq!(names, ["identity"]);
        assert_eq!(id, key(n));
    }
}

#[test]
fn a_node_read_without_a_key_has_none_until_the_formatter_mints_one() {
    let files = edited(
        FEATURE,
        "      - key: 00000000-0000-7000-8000-000000000005\n        name:",
        "      - name:",
    );
    let errors = codec::read(&files).unwrap_err();
    let problems: Vec<&Problem> = errors.iter().map(|error| &error.problem).collect();
    assert_eq!(problems, [&Problem::MissingField("key")]);
    assert_eq!(format(&files).unwrap().minted, 1);
    let document = codec::read(&formatted(&files)).unwrap();
    let requirement = &document.products[0].feature_groups[0].features[0].requirements[0];
    assert_minted(requirement.criteria[0].id.unwrap().key(), 5);
}

#[test]
fn a_decision_without_a_key_has_one_after_formatting() {
    let files = edited(DECISION, "key: 00000000-0000-7000-8000-000000000014\n", "");
    let result = format(&files).unwrap();
    assert_eq!(result.minted, 1);
    let adr = Label::parse("ADR-1").unwrap();
    assert_eq!(result.dropped, [(adr, key(20))], "the old key's binding");
    let document = codec::read(&formatted(&files)).unwrap();
    assert_minted(document.decisions[0].id.key(), 20);
}

#[test]
fn a_tag_or_category_entry_without_a_key_has_one_after_formatting() {
    let files = edited_all(&[
        (
            "tags.yaml",
            "- key: 00000000-0000-7000-8000-00000000001e\n  name:",
            "- name:",
        ),
        (
            "categories.yaml",
            "- key: 00000000-0000-7000-8000-000000000020\n  name:",
            "- name:",
        ),
    ]);
    assert_eq!(format(&files).unwrap().minted, 2);
    let document = codec::read(&formatted(&files)).unwrap();
    let tag = document
        .tags
        .iter()
        .find(|tag| tag.name.as_str() == "identity");
    assert_minted(tag.unwrap().id.key(), 30);
    assert_minted(document.categories[0].id.key(), 32);
}

/// One record of each keyed kind the Format names, with the edits that remove its key. A file's
/// own node is also taken off the list that names it, which the formatter then lists it in again.
const KEYED: [(&str, &[Edit]); 14] = [
    (
        "Product",
        &[
            (PRODUCT, "key: 00000000-0000-7000-8000-000000000001\n", ""),
            (
                "loftspec.yaml",
                "  - 00000000-0000-7000-8000-000000000001\n",
                "",
            ),
        ],
    ),
    (
        "Feature Group",
        &[(
            PRODUCT,
            "  - key: 00000000-0000-7000-8000-000000000002\n    slug:",
            "  - slug:",
        )],
    ),
    (
        "Feature",
        &[
            (FEATURE, "key: 00000000-0000-7000-8000-000000000003\n", ""),
            (
                PRODUCT,
                "      - 00000000-0000-7000-8000-000000000003\n",
                "",
            ),
        ],
    ),
    (
        "Requirement",
        &[(
            FEATURE,
            "  - key: 00000000-0000-7000-8000-000000000004\n    statement:",
            "  - statement:",
        )],
    ),
    (
        "criterion",
        &[(
            FEATURE,
            "      - key: 00000000-0000-7000-8000-000000000005\n        name:",
            "      - name:",
        )],
    ),
    (
        "Component",
        &[(COMPONENT, "key: 00000000-0000-7000-8000-000000000006\n", "")],
    ),
    (
        "Constraint",
        &[(
            COMPONENT,
            "  - key: 00000000-0000-7000-8000-000000000007\n    name:",
            "  - name:",
        )],
    ),
    (
        "Quality Attribute",
        &[(
            COMPONENT,
            "  - key: 00000000-0000-7000-8000-000000000008\n    name:",
            "  - name:",
        )],
    ),
    (
        "decision",
        &[(DECISION, "key: 00000000-0000-7000-8000-000000000014\n", "")],
    ),
    (
        "driver",
        &[(
            DECISION,
            "  - key: 00000000-0000-7000-8000-000000000015\n    kind:",
            "  - kind:",
        )],
    ),
    (
        "option",
        &[(
            DECISION,
            "  - key: 00000000-0000-7000-8000-000000000017\n    title:",
            "  - title:",
        )],
    ),
    (
        "consequence",
        &[(
            DECISION,
            "  - key: 00000000-0000-7000-8000-000000000019\n    kind:",
            "  - kind:",
        )],
    ),
    (
        "tag entry",
        &[(
            "tags.yaml",
            "- key: 00000000-0000-7000-8000-00000000001e\n  name:",
            "- name:",
        )],
    ),
    (
        "category entry",
        &[(
            "categories.yaml",
            "- key: 00000000-0000-7000-8000-000000000020\n  name:",
            "- name:",
        )],
    ),
];

/// A record of each kind outside the keyed kinds, edited to lack or to carry a key, and what the
/// formatter then refuses instead of minting one.
fn unkeyed() -> [(&'static str, Edit, Problem); 3] {
    let carried = || Problem::UnknownField("key".to_owned());
    [
        (
            "binding",
            (
                "labels.yaml",
                "  - key: 00000000-0000-7000-8000-000000000001\n    label:",
                "  - label:",
            ),
            Problem::MissingField("key"),
        ),
        (
            "assessment",
            (
                DECISION,
                "  - option: 00000000-0000-7000-8000-000000000018\n    driver: \
                 00000000-0000-7000-8000-000000000016",
                "  - key: 00000000-0000-7000-8000-000000000070\n    option: \
                 00000000-0000-7000-8000-000000000018\n    driver: \
                 00000000-0000-7000-8000-000000000016",
            ),
            carried(),
        ),
        (
            "subject",
            (
                DECISION,
                "  - role: affected\n",
                "  - key: 00000000-0000-7000-8000-000000000071\n    role: affected\n",
            ),
            carried(),
        ),
    ]
}

#[test]
fn the_keyed_records_are_exactly_nodes_decisions_drivers_options_consequences_and_entries() {
    for (kind, edits) in KEYED {
        let files = edited_all(edits);
        let refused = codec::read(&files).unwrap_err();
        let missing = refused
            .iter()
            .any(|error| error.problem == Problem::MissingField("key"));
        assert!(missing, "a {kind} without a key is refused: {refused:?}");
        let minted = format(&files).map(|result| result.minted);
        assert_eq!(minted, Ok(1), "a {kind} without a key is given one");
    }
    for (kind, edit, problem) in unkeyed() {
        let errors = format(&edited_all(&[edit])).unwrap_err();
        let problems: Vec<&Problem> = errors.iter().map(|error| &error.problem).collect();
        assert_eq!(problems, [&problem], "a {kind} takes no minted key");
    }
}
