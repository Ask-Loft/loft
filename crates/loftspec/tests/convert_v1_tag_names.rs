//! The v2 tag names a v1 conversion derives from v1 names (F-14, F-15): each Facet Tag's and
//! each Vocabulary Entry's name becomes a tag name, and the sidecar keeps every name it changed.

use crate::common::v1::{convert_value, full_export};
use loftspec::convert::v1::{Conversion, ConvertError};
use loftspec::names::TagName;
use serde_json::{Value, json};

/// The Facet Tag on P-1 (`portable`) in the full export.
const FACET_TAG: usize = 8;
/// The Facet Tag on CMP-1 (`identity`) in the full export.
const OTHER_FACET_TAG: usize = 13;
/// The registry entry of `portable` in the full export.
const ENTRY: usize = 0;

fn tag(name: &str) -> TagName {
    TagName::parse(name).unwrap()
}

/// The full export with the Facet Tag on P-1 named `v1_name`.
fn facet_tag_named(v1_name: &str) -> Value {
    let mut value = full_export();
    value["nodes"][FACET_TAG]["name"] = json!(v1_name);
    value
}

/// The full export with the Facet Tag on P-1 and its registry entry both named `v1_name`.
fn both_named(v1_name: &str) -> Value {
    let mut value = facet_tag_named(v1_name);
    value["vocabularyEntries"][ENTRY]["name"] = json!(v1_name);
    value
}

fn converted(value: &Value) -> Conversion {
    convert_value(value).unwrap()
}

/// The tag name derived from `v1_name`, as the Product lists it.
fn derived(v1_name: &str) -> String {
    let document = converted(&both_named(v1_name)).document;
    document.products[0].details.tags[0].as_str().to_owned()
}

#[test]
fn the_parent_lists_the_tag_name_derived_from_the_facet_tag_s_name() {
    let document = converted(&facet_tag_named("MCP tool")).document;
    assert_eq!(document.products[0].details.tags, [tag("mcp-tool")]);
}

#[test]
fn a_registry_entry_s_name_is_the_tag_name_derived_from_the_entry_s_name() {
    let mut value = full_export();
    value["vocabularyEntries"][ENTRY]["name"] = json!("MCP tool");
    let document = converted(&value).document;
    assert_eq!(document.tags[ENTRY].name, tag("mcp-tool"));
}

#[test]
fn a_registry_entry_keeps_the_entry_s_definition() {
    let document = converted(&both_named("MCP tool")).document;
    assert_eq!(
        document.tags[ENTRY].definition,
        "Behaviour every implementation reproduces."
    );
}

#[test]
fn the_derived_name_is_lowercase_with_each_run_of_other_characters_one_hyphen() {
    for (v1_name, expected) in [
        ("MCP tool", "mcp-tool"),
        ("MCP  Tool", "mcp-tool"),
        ("Sign-in / Sign-up", "sign-in-sign-up"),
        ("Web_API v2", "web-api-v2"),
        ("Grüße", "gr-e"),
    ] {
        assert_eq!(derived(v1_name), expected, "{v1_name:?}");
    }
}

#[test]
fn the_derived_name_has_no_hyphen_at_its_start_or_its_end() {
    for (v1_name, expected) in [
        (" (Draft) ", "draft"),
        ("-Edge-", "edge"),
        ("_internal_", "internal"),
    ] {
        assert_eq!(derived(v1_name), expected, "{v1_name:?}");
    }
}

#[test]
fn a_v1_name_that_is_already_a_tag_name_is_kept_as_it_is() {
    for v1_name in ["identity", "key-shape-2", "double--hyphen", "trailing-"] {
        assert_eq!(derived(v1_name), v1_name);
        let sidecar = converted(&both_named(v1_name)).sidecar;
        assert!(sidecar.tag_names.is_empty(), "{v1_name:?}");
    }
}

#[test]
fn a_v1_name_whose_derived_name_is_empty_or_begins_with_no_letter_stops_the_conversion() {
    let mut entry_only = full_export();
    entry_only["vocabularyEntries"][ENTRY]["name"] = json!("2FA");
    for (value, v1_name) in [
        (facet_tag_named("!!!"), "!!!"),
        (facet_tag_named("3D model"), "3D model"),
        (entry_only, "2FA"),
    ] {
        let error = convert_value(&value).unwrap_err();
        assert_eq!(
            error,
            ConvertError::UnderivableTag {
                name: v1_name.to_owned()
            }
        );
        assert!(error.to_string().contains(v1_name), "{error}");
    }
}

#[test]
fn two_v1_names_that_derive_one_tag_name_stop_the_conversion_naming_both() {
    let mut two_facet_tags = facet_tag_named("MCP Tool");
    two_facet_tags["nodes"][OTHER_FACET_TAG]["name"] = json!("mcp tool");
    let mut facet_tag_and_entry = facet_tag_named("MCP tool");
    facet_tag_and_entry["vocabularyEntries"][ENTRY]["name"] = json!("mcp-tool");
    for (value, first, second) in [
        (two_facet_tags, "MCP Tool", "mcp tool"),
        (facet_tag_and_entry, "MCP tool", "mcp-tool"),
    ] {
        let error = convert_value(&value).unwrap_err();
        assert_eq!(
            error,
            ConvertError::TagCollision {
                first: first.to_owned(),
                second: second.to_owned(),
                tag: tag("mcp-tool"),
            }
        );
        let message = error.to_string();
        assert!(
            message.contains(first) && message.contains(second),
            "{message}"
        );
    }
}

#[test]
fn a_facet_tag_and_a_vocabulary_entry_with_one_v1_name_derive_one_tag_name() {
    let document = converted(&both_named("MCP tool")).document;
    assert_eq!(document.products[0].details.tags, [tag("mcp-tool")]);
    assert_eq!(document.tags[ENTRY].name, tag("mcp-tool"));
}

#[test]
fn the_sidecar_records_the_v1_name_behind_each_derived_tag_name_that_differs() {
    let sidecar = converted(&both_named("MCP tool")).sidecar;
    assert_eq!(
        sidecar.tag_names.iter().collect::<Vec<_>>(),
        [(&tag("mcp-tool"), &"MCP tool".to_owned())],
        "`identity` is kept, so it is not recorded"
    );
    let written: Value = serde_json::from_str(&sidecar.to_json()).unwrap();
    assert_eq!(written["tagNames"], json!({"mcp-tool": "MCP tool"}));
}

#[test]
fn a_sidecar_without_a_renamed_tag_writes_no_tag_names() {
    let sidecar = converted(&full_export()).sidecar;
    assert!(sidecar.tag_names.is_empty());
    assert!(!sidecar.to_json().contains("tagNames"));
}
