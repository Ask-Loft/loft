//! The term list: its file, its entries' fields and order, its keys, its root, and how the
//! projections show it.

use crate::common::text::formatted;
use crate::common::text::{files, path, text};
use crate::common::{key, small_document, term};
use loftspec::codec::{self, Files, Problem, TERMS};
use loftspec::document::{Document, TermEntry};
use loftspec::fmt::format;
use loftspec::hash::{Views, hash_document, hash_indexed_with_views};
use loftspec::render::render;
use serde_json::{Value, json};

/// The small document with a term list.
fn with_terms(terms: Vec<TermEntry>) -> Document {
    let mut document = small_document();
    document.terms = terms;
    document
}

/// The small document's files with `spec/terms.yaml` holding `yaml`.
fn with_terms_file(yaml: &str) -> Files {
    let mut files = files();
    files.insert(path(TERMS), yaml.as_bytes().to_vec());
    files
}

const ENTRY: &str = "- key: 00000000-0000-7000-8000-000000000050
  name: \"change set\"
  definition: \"A group of edits.\"
  aliases:
    - \"changeset\"
";

fn views(document: &Document) -> Views {
    hash_indexed_with_views(&document.index().unwrap()).1
}

/// The root view named `name`, if the hasher computed one.
fn root_view(views: &Views, name: &str) -> Option<Value> {
    views
        .roots
        .iter()
        .find(|(each, ..)| *each == name)
        .map(|(_, _, view)| view.clone())
}

#[test]
fn the_term_list_is_written_to_spec_terms_yaml() {
    let files = codec::write(&with_terms(vec![term(50, "change set", &[])]));
    assert!(text(&files, TERMS).contains("name: \"change set\""));
}

#[test]
fn a_document_without_a_term_is_written_without_spec_terms_yaml() {
    assert!(!codec::write(&small_document()).contains_key(&path(TERMS)));
}

#[test]
fn a_tree_without_spec_terms_yaml_or_with_an_empty_one_holds_an_empty_term_list() {
    for files in [files(), with_terms_file("")] {
        assert_eq!(codec::read(&files).unwrap().terms, []);
    }
}

#[test]
fn a_term_entrys_fields_are_key_name_definition_and_aliases_in_that_order() {
    let files = codec::write(&with_terms(vec![term(50, "change set", &["changeset"])]));
    assert_eq!(
        text(&files, TERMS),
        format!(
            "- key: {}\n  name: \"change set\"\n  definition: \"What change set means.\"\n  \
             aliases:\n    - \"changeset\"\n",
            key(50)
        )
    );
    let read = codec::read(&files).unwrap();
    assert_eq!(read.terms, [term(50, "change set", &["changeset"])]);
}

#[test]
fn a_term_entry_without_aliases_omits_the_field() {
    let files = codec::write(&with_terms(vec![term(50, "change set", &[])]));
    assert!(!text(&files, TERMS).contains("aliases"));
}

#[test]
fn a_term_entry_without_its_key_name_or_definition_is_refused() {
    for (from, to, field) in [
        (
            "- key: 00000000-0000-7000-8000-000000000050\n  name:",
            "- name:",
            "key",
        ),
        ("  name: \"change set\"\n", "", "name"),
        ("  definition: \"A group of edits.\"\n", "", "definition"),
    ] {
        let errors = codec::read(&with_terms_file(&ENTRY.replacen(from, to, 1))).unwrap_err();
        let missing = errors
            .iter()
            .any(|error| error.problem == Problem::MissingField(field));
        assert!(missing, "{field}: {errors:?}");
    }
}

#[test]
fn an_alias_that_is_not_a_string_is_refused() {
    let yaml = ENTRY.replace("    - \"changeset\"\n", "    - one: \"changeset\"\n");
    assert!(codec::read(&with_terms_file(&yaml)).is_err());
}

#[test]
fn a_blank_name_or_alias_is_an_error_naming_its_file_line_and_column() {
    for (from, to, line, column) in [
        ("name: \"change set\"", "name: \" \"", 2, 9),
        ("- \"changeset\"", "- \"\"", 5, 7),
    ] {
        let errors = codec::read(&with_terms_file(&ENTRY.replace(from, to))).unwrap_err();
        assert_eq!(errors.len(), 1, "{errors:?}");
        let site = errors[0].sites[0].to_string();
        assert_eq!(site, format!("spec/terms.yaml:{line}:{column}"), "{from}");
        assert!(
            errors[0].problem.to_string().contains("blank"),
            "{errors:?}"
        );
    }
}

#[test]
fn the_formatter_sorts_term_entries_by_name_and_each_entrys_aliases() {
    let yaml = "- key: 00000000-0000-7000-8000-000000000051
  name: \"work item\"
  definition: \"A unit of work.\"
  aliases:
    - \"task\"
    - \"job\"
- key: 00000000-0000-7000-8000-000000000050
  name: \"change set\"
  definition: \"A group of edits.\"
";
    let formatted = formatted(&with_terms_file(yaml));
    let written = text(&formatted, TERMS);
    let change = written.find("change set").unwrap();
    assert!(change < written.find("work item").unwrap(), "{written}");
    assert!(
        written.contains("    - \"job\"\n    - \"task\"\n"),
        "{written}"
    );
}

#[test]
fn a_term_entry_without_a_key_has_one_after_formatting() {
    let files = with_terms_file(&ENTRY.replace(
        "- key: 00000000-0000-7000-8000-000000000050\n  name:",
        "- name:",
    ));
    let refused = codec::read(&files).unwrap_err();
    assert!(
        refused
            .iter()
            .any(|e| e.problem == Problem::MissingField("key"))
    );
    assert_eq!(format(&files).unwrap().minted, 1);
    let document = codec::read(&formatted(&files)).unwrap();
    assert_eq!(document.terms[0].id.key().version(), 7);
}

#[test]
fn a_document_without_a_term_has_no_term_list_root_and_its_content_view_is_unchanged() {
    let views = views(&small_document());
    assert_eq!(root_view(&views, "terms"), None);
    let content = root_view(&views, "content").unwrap();
    assert_eq!(content.as_array().unwrap().len(), 6);
    assert_eq!(hash_document(&small_document()).unwrap().roots.terms, None);
}

#[test]
fn the_term_list_root_follows_the_decisions_root_in_the_content_view() {
    let document = with_terms(vec![term(50, "change set", &[])]);
    let roots = hash_document(&document).unwrap().roots;
    let content = root_view(&views(&document), "content").unwrap();
    assert_eq!(content[5], json!(roots.decisions));
    assert_eq!(content[6], json!(roots.terms.unwrap()));
    assert_ne!(
        roots.content,
        hash_document(&small_document()).unwrap().roots.content
    );
}

#[test]
fn the_term_list_root_view_begins_with_terms_and_holds_the_sorted_entries() {
    let document = with_terms(vec![
        term(51, "work item", &["task", "job"]),
        term(50, "change set", &[]),
    ]);
    let view = root_view(&views(&document), "terms").unwrap();
    assert_eq!(
        view,
        json!([
            "terms",
            [
                ["change set", "What change set means.", []],
                ["work item", "What work item means.", ["job", "task"]],
            ]
        ])
    );
}

#[test]
fn changing_a_terms_alias_changes_the_term_list_root() {
    let root = |aliases: &[&str]| {
        let document = with_terms(vec![term(50, "change set", aliases)]);
        hash_document(&document).unwrap().roots.terms
    };
    assert_ne!(root(&["changeset"]), root(&["change-set"]));
    assert_eq!(root(&["a", "b"]), root(&["b", "a"]));
}

#[test]
fn the_export_holds_the_term_list_only_when_it_has_an_entry() {
    let export = |document: &Document| -> Value {
        serde_json::from_slice(&render(document).unwrap().json).unwrap()
    };
    assert_eq!(export(&small_document()).get("terms"), None);
    let document = with_terms(vec![term(50, "change set", &["changeset"])]);
    assert_eq!(
        export(&document)["terms"],
        json!([{
            "key": key(50),
            "name": "change set",
            "definition": "What change set means.",
            "aliases": ["changeset"],
        }])
    );
}

#[test]
fn the_term_list_renders_as_a_glossary_after_the_tag_registry() {
    let document = with_terms(vec![term(50, "change set", &["changeset", "batch"])]);
    let markdown = render(&document).unwrap().markdown;
    let glossary = markdown.find("## Glossary\n").unwrap();
    assert!(markdown.find("## Domain Vocabulary\n").unwrap() < glossary);
    assert!(markdown[glossary..].starts_with(
        "## Glossary\n\n### change set\n\nWhat change set means.\n\nAliases: batch, changeset\n"
    ));
    assert!(
        !render(&small_document())
            .unwrap()
            .markdown
            .contains("## Glossary")
    );
}
