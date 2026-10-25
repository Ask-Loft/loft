//! The JSON files of `~/.yard/`: each written pretty, an object's names in order, ending in a
//! line break; a file that is not there, or does not read as an object, read as an empty one.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};
use yardlog_store::home::{Home, HomeFile};

use super::failure::Failure;

/// The object a file's bytes hold: empty where there are none or they hold no object.
pub fn object(bytes: Option<&[u8]>) -> Map<String, Value> {
    match bytes.map(serde_json::from_slice::<Value>) {
        Some(Ok(Value::Object(map))) => map,
        _ => Map::new(),
    }
}

/// A value's bytes as a file of `~/.yard/` holds them.
pub fn bytes(value: &impl Serialize) -> Vec<u8> {
    let mut text = serde_json::to_string_pretty(value).expect("a JSON value serializes");
    text.push('\n');
    text.into_bytes()
}

/// An object's bytes as a file of `~/.yard/` holds them, its names in order.
pub fn ordered(map: &Map<String, Value>) -> Vec<u8> {
    bytes(&map.iter().collect::<BTreeMap<&String, &Value>>())
}

/// Sets `name` to `value` in the object `file` holds, under `~/.yard/`'s lock, leaving the file
/// as it was where it holds that already; whether it changed.
pub fn set(home: &Home, file: HomeFile, name: &str, value: Value) -> Result<bool, Failure> {
    let mut changed = false;
    home.update(file, |held| {
        let mut map = object(held.as_deref());
        if map.get(name) == Some(&value) {
            return None;
        }
        map.insert(name.to_owned(), value);
        changed = true;
        Some(ordered(&map))
    })?;
    Ok(changed)
}
