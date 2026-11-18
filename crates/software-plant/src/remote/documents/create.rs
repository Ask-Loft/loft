//! A create: the document's key and its name, which make the remote hold the document, with no
//! published state, unless it holds it already.

use serde_json::{Value, json};

use crate::http::Response;
use crate::remote::problem::{conflict, invalid, refusal};
use crate::remote::{Call, Held, State, head_mark};

/// The longest name a document may have, in Unicode scalar values.
const NAME_LIMIT: usize = 200;

pub(super) fn create(state: &mut State, body: &Value) -> Response {
    let Some(fields) = body.as_object() else {
        return invalid(&[], Some("the body is not a JSON object"));
    };
    let exact = fields.len() == 2 && fields.contains_key("documentKey");
    let Some(name) = fields.get("name").and_then(Value::as_str).filter(|_| exact) else {
        return invalid(&[], Some("the body holds exactly `documentKey` and `name`"));
    };
    if fields["documentKey"] != state.scenario.key.as_str() {
        return invalid(&[], None);
    }
    if name.trim().is_empty() || name.chars().count() > NAME_LIMIT {
        return invalid(&[], Some("`name` is blank or longer than 200 characters"));
    }
    if state.scenario.forbidden.contains(&Call::Create) {
        return refusal("forbidden");
    }
    if let Some(held) = &state.held {
        return conflict(head_mark(held), None);
    }
    state.held = Some(Held {
        name: name.to_owned(),
        ..Held::default()
    });
    Response::json(
        200,
        &json!({ "documentKey": state.scenario.key, "head": null }),
    )
}
