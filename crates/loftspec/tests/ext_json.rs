//! Extension data read back from its JSON (Extension data): the exact inverse of writing it as
//! JSON, refusing JSON the text form cannot hold, at the JSON Pointer of the value refused.

use std::collections::BTreeMap;

use loftspec::codec;
use loftspec::ext::{self, Ext, ExtValue, FromJsonError, JsonProblem, MAX_INTEGER};
use loftspec::names::Namespace;
use proptest::prelude::*;
use serde_json::{Value, json};

use crate::common::small_document;

fn refusal(json: &Value) -> FromJsonError {
    ext::from_json(json).expect_err("the JSON is refused")
}

fn refused(json: &Value, path: &str, problem: JsonProblem) {
    assert_eq!(
        refusal(json),
        FromJsonError {
            path: path.to_owned(),
            problem
        },
        "{json}"
    );
}

/// Extension values the text form holds: text, integers within the bound, flags, and lists and
/// mappings that hold an entry, under keys that are field names.
fn value() -> impl Strategy<Value = ExtValue> {
    let leaf = prop_oneof![
        any::<String>().prop_map(ExtValue::Text),
        (-MAX_INTEGER..=MAX_INTEGER).prop_map(ExtValue::Integer),
        any::<bool>().prop_map(ExtValue::Boolean),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 1..4).prop_map(ExtValue::List),
            proptest::collection::btree_map("[A-Za-z][A-Za-z0-9]{0,6}", inner, 1..4)
                .prop_map(ExtValue::Map),
        ]
    })
}

fn extension_data() -> impl Strategy<Value = Ext> {
    proptest::collection::btree_map("[a-z][a-z0-9]{0,6}", value(), 0..3).prop_map(|entries| {
        entries
            .into_iter()
            .map(|(name, value)| (Namespace::parse(&name).unwrap(), value))
            .collect()
    })
}

/// JSON of every shape, extension data or not.
fn any_json() -> impl Strategy<Value = Value> {
    let leaf = prop_oneof![
        Just(Value::Null),
        any::<bool>().prop_map(Value::Bool),
        any::<i64>().prop_map(Value::from),
        any::<u64>().prop_map(Value::from),
        (-1.0e3..1.0e3f64).prop_map(Value::from),
        "[a-z ]{0,4}".prop_map(Value::from),
        any::<String>().prop_map(Value::from),
    ];
    leaf.prop_recursive(3, 24, 4, |inner| {
        prop_oneof![
            proptest::collection::vec(inner.clone(), 0..3).prop_map(Value::from),
            proptest::collection::btree_map("[a-zA-Z0-9_/~]{0,3}", inner, 0..3)
                .prop_map(|entries| Value::Object(entries.into_iter().collect())),
        ]
    })
}

proptest! {
    #[test]
    fn extension_data_written_as_json_reads_back_as_it_was(ext in extension_data()) {
        prop_assert_eq!(ext::from_json(&ext::to_json(&ext)), Ok(ext));
    }

    #[test]
    fn json_read_as_extension_data_writes_back_as_it_was(json in any_json()) {
        if let Ok(ext) = ext::from_json(&json) {
            prop_assert_eq!(ext::to_json(&ext), json);
        }
    }

    #[test]
    fn json_read_as_extension_data_is_held_by_the_text_form(json in any_json()) {
        if let Ok(ext) = ext::from_json(&json) {
            let mut document = small_document();
            document.products[0].details.ext = ext.clone();
            let read = codec::read(&codec::write(&document)).expect("the ext reads");
            prop_assert_eq!(&read.products[0].details.ext, &ext);
        }
    }
}

#[test]
fn json_that_is_not_an_object_is_refused() {
    for json in [json!(null), json!([]), json!("text"), json!(3)] {
        refused(&json, "", JsonProblem::NotAnObject);
    }
}

#[test]
fn an_object_key_that_is_not_an_extension_namespace_is_refused() {
    refused(&json!({"Tool": 1}), "/Tool", JsonProblem::Namespace);
    refused(&json!({"1tool": 1}), "/1tool", JsonProblem::Namespace);
}

#[test]
fn a_null_is_refused() {
    refused(&json!({"tool": null}), "/tool", JsonProblem::Null);
    refused(&json!({"tool": [1, null]}), "/tool/1", JsonProblem::Null);
}

#[test]
fn a_number_with_a_fraction_or_an_exponent_is_refused() {
    refused(&json!({"tool": 1.5}), "/tool", JsonProblem::NotAnInteger);
    let exponent: Value = serde_json::from_str(r#"{"tool": 1e2}"#).unwrap();
    refused(&exponent, "/tool", JsonProblem::NotAnInteger);
}

#[test]
fn an_integer_outside_the_canonical_json_bound_is_refused_and_the_bounds_read() {
    refused(
        &json!({"tool": MAX_INTEGER + 1}),
        "/tool",
        JsonProblem::OutOfRange,
    );
    refused(
        &json!({"tool": -MAX_INTEGER - 1}),
        "/tool",
        JsonProblem::OutOfRange,
    );
    refused(&json!({"tool": u64::MAX}), "/tool", JsonProblem::OutOfRange);
    let bounds = json!({"tool": [MAX_INTEGER, -MAX_INTEGER]});
    let read = ext::from_json(&bounds).unwrap();
    let tool = ExtValue::List(vec![
        ExtValue::Integer(MAX_INTEGER),
        ExtValue::Integer(-MAX_INTEGER),
    ]);
    assert_eq!(read, Ext::from([(Namespace::parse("tool").unwrap(), tool)]));
}

#[test]
fn an_empty_array_or_object_is_refused() {
    refused(&json!({"tool": []}), "/tool", JsonProblem::Empty);
    refused(&json!({"tool": {"a": {}}}), "/tool/a", JsonProblem::Empty);
}

#[test]
fn a_mapping_key_that_is_not_a_field_name_is_refused() {
    refused(
        &json!({"tool": {"two words": 1}}),
        "/tool/two words",
        JsonProblem::Key,
    );
    refused(
        &json!({"tool": {"9lives": 1}}),
        "/tool/9lives",
        JsonProblem::Key,
    );
}

#[test]
fn a_refusal_names_the_json_pointer_of_the_value_it_refuses() {
    let json = json!({"tool": {"items": [{"ok": 1}, {"a/b": 2}]}});
    refused(&json, "/tool/items/1/a~1b", JsonProblem::Key);
    let error = refusal(&json!({"tool": {"x": [true, null]}}));
    assert_eq!(error.path, "/tool/x/1");
    assert!(error.to_string().contains("`/tool/x/1`"), "{error}");
}

#[test]
fn an_empty_object_is_extension_data_with_no_namespace() {
    assert_eq!(ext::from_json(&json!({})), Ok(BTreeMap::new()));
}
