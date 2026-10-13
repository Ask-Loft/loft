//! A check of a JSON value against the subset of JSON Schema 2020-12 that `schemas/` uses:
//! `type`, `const`, `enum`, `minimum`, `required`, `properties`, `additionalProperties: false`,
//! `items` and local `$ref`s into `$defs`.

use serde_json::Value;

/// The one JSON document `stdout` holds, after asserting that it matches `schema`, the text of
/// a file under `schemas/`.
pub fn conforming(stdout: &[u8], schema: &str) -> Value {
    let document: Value = serde_json::from_slice(stdout).expect("one JSON document");
    let schema: Value = serde_json::from_str(schema).expect("a schema");
    assert_eq!(violations(&document, &schema), Vec::<String>::new());
    document
}

/// Every way `value` departs from `schema`, each with the JSON pointer where it does.
pub fn violations(value: &Value, schema: &Value) -> Vec<String> {
    let mut found = Vec::new();
    check(value, schema, schema, "", &mut found);
    found
}

fn check(value: &Value, schema: &Value, root: &Value, at: &str, found: &mut Vec<String>) {
    if let Some(reference) = schema["$ref"].as_str() {
        let name = reference.trim_start_matches("#/$defs/");
        return check(value, &root["$defs"][name], root, at, found);
    }
    if let Some(expected) = schema.get("const")
        && value != expected
    {
        found.push(format!("{at}: {value} is not {expected}"));
    }
    if let Some(options) = schema["enum"].as_array()
        && !options.contains(value)
    {
        found.push(format!("{at}: {value} is not one of {options:?}"));
    }
    if let Some(kind) = schema["type"].as_str()
        && !has_type(value, kind)
    {
        found.push(format!("{at}: {value} is not {kind}"));
    }
    if let (Some(minimum), Some(number)) = (schema["minimum"].as_i64(), value.as_i64())
        && number < minimum
    {
        found.push(format!("{at}: {number} is below {minimum}"));
    }
    if let Some(object) = value.as_object() {
        object_rules(object, schema, root, at, found);
    }
    if let (Some(items), Some(schema)) = (value.as_array(), schema.get("items")) {
        for (index, item) in items.iter().enumerate() {
            check(item, schema, root, &format!("{at}/{index}"), found);
        }
    }
}

fn object_rules(
    object: &serde_json::Map<String, Value>,
    schema: &Value,
    root: &Value,
    at: &str,
    found: &mut Vec<String>,
) {
    for required in schema["required"].as_array().into_iter().flatten() {
        let name = required.as_str().unwrap_or_default();
        if !object.contains_key(name) {
            found.push(format!("{at}: `{name}` is missing"));
        }
    }
    let properties = schema["properties"].as_object();
    for (name, field) in object {
        match properties.and_then(|properties| properties.get(name)) {
            Some(schema) => check(field, schema, root, &format!("{at}/{name}"), found),
            None if schema["additionalProperties"] == Value::Bool(false) => {
                found.push(format!("{at}: `{name}` is not allowed"));
            }
            None => {}
        }
    }
}

fn has_type(value: &Value, kind: &str) -> bool {
    match kind {
        "object" => value.is_object(),
        "array" => value.is_array(),
        "string" => value.is_string(),
        "integer" => value.is_i64() || value.is_u64(),
        "boolean" => value.is_boolean(),
        "null" => value.is_null(),
        _ => false,
    }
}
