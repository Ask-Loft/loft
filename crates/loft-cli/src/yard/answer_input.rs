//! The answers and decisions `loft yard answer` reads: a JSON list, each naming its block by id
//! or key, with the labels chosen, the person's own words and notes for a question, or the
//! verdict and the reason for an approval or a plan. Input of any other shape is a usage error.

use serde_json::{Map, Value};
use yardlog::body::Verdict;
use yardlog::log::Given;

use super::failure::Failure;

/// One answer or decision, its block as the host named it.
pub struct Named {
    pub block: String,
    pub given: Given,
}

fn usage(index: usize, message: &str) -> Failure {
    Failure::Usage(format!("--input: answer #{}: {message}", index + 1))
}

fn text(map: &Map<String, Value>, name: &str, index: usize) -> Result<Option<String>, Failure> {
    match map.get(name) {
        None => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        Some(_) => Err(usage(index, &format!("`{name}` is not text"))),
    }
}

/// Reads the list of answers and decisions.
pub fn read(value: &Value) -> Result<Vec<Named>, Failure> {
    let Value::Array(items) = value else {
        return Err(Failure::Usage("--input: answers are a list".to_owned()));
    };
    if items.is_empty() {
        return Err(Failure::Usage(
            "--input: the list holds no answer".to_owned(),
        ));
    }
    items
        .iter()
        .enumerate()
        .map(|(index, item)| one(item, index))
        .collect()
}

const FIELDS: [&str; 6] = ["block", "labels", "words", "notes", "verdict", "reason"];

fn one(item: &Value, index: usize) -> Result<Named, Failure> {
    let Value::Object(map) = item else {
        return Err(usage(index, "an answer is an object"));
    };
    if let Some(name) = map.keys().find(|name| !FIELDS.contains(&name.as_str())) {
        return Err(usage(
            index,
            &format!("`{name}` is not a field of an answer"),
        ));
    }
    let block = text(map, "block", index)?.ok_or_else(|| usage(index, "`block` is required"))?;
    let given = if let Some(verdict) = text(map, "verdict", index)? {
        let verdict = Verdict::parse(&verdict)
            .ok_or_else(|| usage(index, "`verdict` is `approved` or `rejected`"))?;
        if ["labels", "words", "notes"]
            .iter()
            .any(|n| map.contains_key(*n))
        {
            return Err(usage(
                index,
                "a decision gives a verdict, not labels or words",
            ));
        }
        Given::Outcome {
            verdict,
            reason: text(map, "reason", index)?,
        }
    } else {
        if map.contains_key("reason") {
            return Err(usage(index, "`reason` goes with a `verdict`"));
        }
        Given::Answer {
            chosen: labels(map, index)?,
            words: text(map, "words", index)?,
            notes: text(map, "notes", index)?,
        }
    };
    Ok(Named { block, given })
}

fn labels(map: &Map<String, Value>, index: usize) -> Result<Vec<String>, Failure> {
    match map.get("labels") {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| {
                item.as_str()
                    .map(ToOwned::to_owned)
                    .ok_or_else(|| usage(index, "each of `labels` is text"))
            })
            .collect(),
        Some(_) => Err(usage(index, "`labels` is a list")),
    }
}
