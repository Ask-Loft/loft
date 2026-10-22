//! Reading one block's fields from the JSON an agent wrote, gathering every fault rather than
//! stopping at the first.

use pulldown_cmark::{Event, Parser, Tag};
use serde_json::{Map, Value};

use crate::keys::{Handle, Key};

/// A fault in a batch: the block, by its id or its place in the batch, the field, and what is
/// wrong with it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    pub block: String,
    pub field: String,
    pub message: String,
}

/// The fields of one block, as they are read.
pub(super) struct Fields<'a> {
    map: &'a Map<String, Value>,
    pub(super) block: String,
    pub(super) faults: Vec<Fault>,
    read: Vec<&'static str>,
}

impl<'a> Fields<'a> {
    pub(super) fn new(map: &'a Map<String, Value>, block: String) -> Self {
        Self {
            map,
            block,
            faults: Vec::new(),
            read: Vec::new(),
        }
    }

    pub(super) fn fault(&mut self, field: &str, message: impl Into<String>) {
        self.faults.push(Fault {
            block: self.block.clone(),
            field: field.to_owned(),
            message: message.into(),
        });
    }

    fn take(&mut self, name: &'static str) -> Option<&'a Value> {
        self.read.push(name);
        self.map.get(name).filter(|value| !value.is_null())
    }

    /// A text that must be given and not be blank.
    pub(super) fn text(&mut self, name: &'static str) -> String {
        let found = self.optional_text(name);
        if found.is_none() && !self.faulted(name) {
            self.fault(name, "is required");
        }
        found.unwrap_or_default()
    }

    /// A text that may be left out, but is not blank where given.
    pub(super) fn optional_text(&mut self, name: &'static str) -> Option<String> {
        match self.take(name)? {
            Value::String(text) if !text.trim().is_empty() => Some(text.clone()),
            Value::String(_) => {
                self.fault(name, "is blank");
                None
            }
            _ => {
                self.fault(name, "is not text");
                None
            }
        }
    }

    /// Markdown, which may be empty.
    pub(super) fn markdown(&mut self, name: &'static str) -> String {
        match self.take(name) {
            Some(Value::String(text)) => text.clone(),
            Some(_) => {
                self.fault(name, "is not text");
                String::new()
            }
            None => {
                self.fault(name, "is required");
                String::new()
            }
        }
    }

    /// Markdown that holds no raw HTML.
    pub(super) fn plain_markdown(&mut self, name: &'static str) -> Option<String> {
        let Value::String(text) = self.take(name)? else {
            self.fault(name, "is not text");
            return None;
        };
        if holds_html(text) {
            self.fault(name, "holds raw HTML, which a note may not");
        }
        Some(text.clone())
    }

    pub(super) fn flag(&mut self, name: &'static str) -> bool {
        match self.take(name) {
            Some(Value::Bool(flag)) => *flag,
            Some(_) => {
                self.fault(name, "is not true or false");
                false
            }
            None => false,
        }
    }

    /// A word of a closed set, read by `parse`, which `words` lists.
    pub(super) fn word<T>(
        &mut self,
        name: &'static str,
        parse: fn(&str) -> Option<T>,
        words: &str,
    ) -> Option<T> {
        let found = self.take(name)?;
        let parsed = found.as_str().and_then(parse);
        if parsed.is_none() {
            self.fault(name, format!("is not one of {words}"));
        }
        parsed
    }

    pub(super) fn handle(&mut self, name: &'static str) -> Option<Handle> {
        let Value::String(text) = self.take(name)? else {
            self.fault(name, "is not text");
            return None;
        };
        Handle::parse(text)
            .map_err(|error| self.fault(name, error.to_string()))
            .ok()
    }

    pub(super) fn key(&mut self, name: &'static str, value: &Value) -> Option<Key> {
        let parsed = value.as_str().map(Key::parse);
        match parsed {
            Some(Ok(key)) => Some(key),
            Some(Err(error)) => {
                self.fault(name, error.to_string());
                None
            }
            None => {
                self.fault(name, "is not a key");
                None
            }
        }
    }

    /// A field's value as written, for a reader of its own.
    pub(super) fn raw(&mut self, name: &'static str) -> Option<&'a Value> {
        self.take(name)
    }

    pub(super) fn list(&mut self, name: &'static str) -> Option<&'a Vec<Value>> {
        let Value::Array(items) = self.take(name)? else {
            self.fault(name, "is not a list");
            return None;
        };
        Some(items)
    }

    fn faulted(&self, name: &str) -> bool {
        self.faults.iter().any(|fault| fault.field == name)
    }

    /// Refuses each field the block's kind does not read.
    pub(super) fn finish(mut self) -> Vec<Fault> {
        let mut extra: Vec<&String> = self
            .map
            .keys()
            .filter(|name| !self.read.contains(&name.as_str()))
            .collect();
        extra.sort();
        for name in extra {
            let message = refusal(name);
            self.fault(name, message);
        }
        self.faults
    }
}

/// Why a field that is not one of the kind's own is refused.
fn refusal(name: &str) -> &'static str {
    match name {
        "key" => "is minted by `loft`, never given",
        "answer" | "answers" | "outcome" | "outcomes" => "only the person may answer or decide",
        "listed" | "started" | "ended" | "listedAt" | "startedAt" | "endedAt" => {
            "is recorded by `loft` from the writes that change the state"
        }
        "report" | "recorded" => "is computed by `loft` from its own library, never given",
        "number" => "is given by `loft`",
        _ => "is not a field of this kind",
    }
}

/// Whether Markdown holds raw HTML, as a block or inline.
fn holds_html(markdown: &str) -> bool {
    Parser::new(markdown).any(|event| {
        matches!(
            event,
            Event::Html(_) | Event::InlineHtml(_) | Event::Start(Tag::HtmlBlock)
        )
    })
}
