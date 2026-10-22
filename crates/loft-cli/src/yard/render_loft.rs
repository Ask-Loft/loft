//! A `loft` block's version as the results give it: the view, what the agent asked for, where
//! and when the report was computed, and the report as `loft` gave it.

use serde_json::{Map, Value, json};
use yardlog::body::{Loft, Reason, Recorded};

fn put(fields: &mut Map<String, Value>, name: &str, value: Option<&String>) {
    if let Some(value) = value {
        fields.insert(name.to_owned(), json!(value));
    }
}

pub fn loft(loft: &Loft) -> Value {
    let mut fields = Map::new();
    fields.insert("view".to_owned(), json!(loft.view.as_str()));
    put(&mut fields, "note", loft.note.as_ref());
    put(&mut fields, "base", loft.base.as_ref());
    put(&mut fields, "decision", loft.decision.as_ref());
    let reasons: Vec<Value> = loft.reasons.iter().map(reason).collect();
    fields.insert("reasons".to_owned(), Value::Array(reasons));
    fields.insert("recorded".to_owned(), recorded(&loft.recorded));
    fields.insert("report".to_owned(), Value::from(&loft.report));
    Value::Object(fields)
}

fn reason(reason: &Reason) -> Value {
    json!({ "rule": reason.rule, "key": reason.key.to_string(), "reason": reason.reason })
}

fn recorded(recorded: &Recorded) -> Value {
    let mut fields = Map::new();
    put(&mut fields, "commit", recorded.commit.as_ref());
    put(&mut fields, "branch", recorded.branch.as_ref());
    fields.insert("dirty".to_owned(), json!(recorded.dirty));
    fields.insert("loft".to_owned(), json!(recorded.loft));
    fields.insert("at".to_owned(), json!(recorded.at.to_string()));
    Value::Object(fields)
}
