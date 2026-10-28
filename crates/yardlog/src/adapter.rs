//! A native adapter's versioned declaration of the Yard integration it can provide.

use loft_common::yaml::Out;
use serde_json::{Value, json};

use crate::usage::COUNTERS;

/// The format of the descriptor an adapter sends.
pub const FORMAT: &str = "yard-adapter/1";

/// The normalized activity kinds understood by the core.
pub const EVENTS: &[&str] = &[
    "turn.start",
    "turn.end",
    "request",
    "tool.start",
    "tool.end",
    "subagent.start",
    "subagent.end",
    "compaction",
    "plan.start",
    "plan.end",
];

/// Validated metadata, independent of any host runtime or I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Descriptor {
    version: String,
    events: Vec<String>,
    usage: Vec<String>,
    native_questions: bool,
    native_plans: bool,
    task_mirroring: bool,
}

impl Descriptor {
    /// Reads the versioned wire declaration. Missing capabilities are unavailable.
    pub fn read(value: &Value) -> Result<Self, &'static str> {
        let object = value.as_object().ok_or("adapter must be an object")?;
        if object
            .keys()
            .any(|key| !["format", "version", "capabilities"].contains(&key.as_str()))
        {
            return Err("adapter holds an unknown field");
        }
        if value["format"] != FORMAT {
            return Err("unsupported adapter format; expected yard-adapter/1");
        }
        let version = value["version"]
            .as_str()
            .filter(|text| !text.is_empty() && text.len() <= 64)
            .ok_or("adapter version must be nonempty and at most 64 bytes")?;
        let capabilities = value["capabilities"]
            .as_object()
            .ok_or("adapter capabilities must be an object")?;
        if capabilities.keys().any(|key| {
            ![
                "events",
                "usage",
                "nativeQuestions",
                "nativePlans",
                "taskMirroring",
            ]
            .contains(&key.as_str())
        }) {
            return Err("adapter capabilities hold an unknown field");
        }
        let cap = &value["capabilities"];
        Ok(Self {
            version: version.to_owned(),
            events: names(cap.get("events"), EVENTS)?,
            usage: names(cap.get("usage"), &COUNTERS)?,
            native_questions: flag(cap.get("nativeQuestions"))?,
            native_plans: flag(cap.get("nativePlans"))?,
            task_mirroring: flag(cap.get("taskMirroring"))?,
        })
    }

    /// The normalized descriptor that a caller reads, including empty capability lists.
    #[must_use]
    pub fn json(&self) -> Value {
        json!({"format":FORMAT, "version":self.version, "capabilities":{
            "events":self.events, "usage":self.usage, "nativeQuestions":self.native_questions,
            "nativePlans":self.native_plans, "taskMirroring":self.task_mirroring
        }})
    }

    /// Canonical YAML data. Empty lists are omitted and read as unavailable.
    #[must_use]
    pub fn yaml(&self) -> Out {
        let mut capabilities = vec![
            (
                "nativeQuestions",
                Out::Plain(self.native_questions.to_string()),
            ),
            ("nativePlans", Out::Plain(self.native_plans.to_string())),
            ("taskMirroring", Out::Plain(self.task_mirroring.to_string())),
        ];
        for (name, values) in [("events", &self.events), ("usage", &self.usage)] {
            if !values.is_empty() {
                capabilities.push((
                    name,
                    Out::Sequence(values.iter().cloned().map(Out::Token).collect()),
                ));
            }
        }
        Out::Mapping(vec![
            ("format", Out::Token(FORMAT.to_owned())),
            ("version", Out::Text(self.version.clone())),
            ("capabilities", Out::Mapping(capabilities)),
        ])
    }
}

fn flag(value: Option<&Value>) -> Result<bool, &'static str> {
    value.map_or(Ok(false), |value| {
        value.as_bool().ok_or("adapter capability must be boolean")
    })
}

fn names(value: Option<&Value>, allowed: &[&str]) -> Result<Vec<String>, &'static str> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let values = value
        .as_array()
        .ok_or("adapter capability list must be an array")?;
    let mut names = Vec::new();
    for value in values {
        let name = value
            .as_str()
            .filter(|name| allowed.contains(name))
            .ok_or("adapter capability list holds an unsupported name")?;
        if !names.iter().any(|held| held == name) {
            names.push(name.to_owned());
        }
    }
    Ok(names)
}
