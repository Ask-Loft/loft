//! A session's consultation file as text, `yardconsult/1`, and back: the counts, the order and the
//! estimate in all, by stage and within it by agent, and under no stage.

use loft_common::yaml::{Out, write};
use serde_json::Value;

use super::{Consultation, Estimate, Order, Scope, Six, Stage};
use crate::Time;
use crate::totals::Who;

/// The format a consultation file names.
pub const FORMAT: &str = "yardconsult/1";

impl Six {
    /// The six counts' fields, each written, a zero too.
    #[must_use]
    pub fn fields(self) -> Vec<(&'static str, Out)> {
        [
            ("specReads", self.spec_reads),
            ("codeReads", self.code_reads),
            ("otherReads", self.other_reads),
            ("loftReads", self.loft_reads),
            ("specEdits", self.spec_edits),
            ("codeEdits", self.code_edits),
        ]
        .into_iter()
        .map(|(name, count)| (name, Out::Integer(count)))
        .collect()
    }

    /// Reads what [`Six::fields`] wrote, as JSON.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        let field = |name: &str| value[name].as_u64().unwrap_or_default();
        Self {
            spec_reads: field("specReads"),
            code_reads: field("codeReads"),
            other_reads: field("otherReads"),
            loft_reads: field("loftReads"),
            spec_edits: field("specEdits"),
            code_edits: field("codeEdits"),
        }
    }
}

impl Order {
    /// The order's fields: each time it holds, and the edits of code before the specification.
    #[must_use]
    pub fn fields(self) -> Vec<(&'static str, Out)> {
        let mut fields = Vec::new();
        if let Some(at) = self.first_spec {
            fields.push(("firstSpec", Out::Token(at.to_string())));
        }
        if let Some(at) = self.first_code_edit {
            fields.push(("firstCodeEdit", Out::Token(at.to_string())));
        }
        fields.push((
            "codeEditsBeforeSpec",
            Out::Integer(self.code_edits_before_spec),
        ));
        fields
    }

    /// Reads what [`Order::fields`] wrote, as JSON.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        Self {
            first_spec: time(value, "firstSpec"),
            first_code_edit: time(value, "firstCodeEdit"),
            code_edits_before_spec: value["codeEditsBeforeSpec"].as_u64().unwrap_or_default(),
        }
    }
}

fn time(value: &Value, name: &str) -> Option<Time> {
    value[name].as_str().and_then(|at| at.parse().ok())
}

impl Scope {
    /// The scope's fields: its counts, its order and its estimate where it took any tokens.
    fn fields(self) -> Vec<(&'static str, Out)> {
        let mut fields = vec![
            ("counts", Out::Mapping(self.counts.fields())),
            ("order", Out::Mapping(self.order.fields())),
        ];
        let estimate = self.estimate.fields();
        if !estimate.is_empty() {
            fields.push(("estimate", Out::Mapping(estimate)));
        }
        fields
    }

    /// Reads what [`Scope::fields`] wrote, as JSON.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        Self {
            counts: Six::from_fields(&value["counts"]),
            order: Order::from_fields(&value["order"]),
            estimate: Estimate::from_fields(&value["estimate"]),
        }
    }
}

/// One stage's entry: its key, where it has one, its scope in all, and each agent's.
fn stage_out(stage: Option<&str>, counted: &Stage) -> Out {
    let mut fields = Vec::new();
    if let Some(stage) = stage {
        fields.push(("stage", Out::Token(stage.to_owned())));
    }
    fields.extend(counted.all.fields());
    let agents = counted.agents.iter().map(|(who, scope)| {
        let mut fields = who.fields();
        fields.extend(scope.fields());
        Out::Mapping(fields)
    });
    fields.push(("agents", Out::Sequence(agents.collect())));
    Out::Mapping(fields)
}

impl Stage {
    /// Reads what `stage_out` wrote, as JSON.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        let agents = value["agents"].as_array().into_iter().flatten();
        Self {
            all: Scope::from_fields(value),
            agents: agents
                .map(|agent| (Who::from_fields(agent), Scope::from_fields(agent)))
                .collect(),
        }
    }
}

impl Consultation {
    /// Reads a consultation file as JSON, as [`Consultation::text`] wrote it.
    #[must_use]
    pub fn read(file: &Value) -> Self {
        let items = |name: &str| file[name].as_array().cloned().unwrap_or_default();
        let stages = items("stages").into_iter().filter_map(|stage| {
            Some((
                stage["stage"].as_str()?.to_owned(),
                Stage::from_fields(&stage),
            ))
        });
        Self {
            session: file["session"].as_str().unwrap_or_default().to_owned(),
            host: file["host"].as_str().unwrap_or_default().to_owned(),
            first_started: time(file, "firstStarted"),
            last_started: time(file, "lastStarted"),
            last_ended: time(file, "lastEnded"),
            all: Scope {
                counts: Six::from_fields(&file["counts"]),
                order: Order::default(),
                estimate: Estimate::from_fields(&file["estimate"]),
            },
            stages: stages.collect(),
            no_stage: file.get("noStage").map(Stage::from_fields),
            partial: file["partial"].as_bool().unwrap_or_default(),
            ..Self::default()
        }
    }

    /// The consultation file's text.
    #[must_use]
    pub fn text(&self) -> String {
        let mut fields = vec![
            ("format", Out::Token(FORMAT.to_owned())),
            ("session", Out::Token(self.session.clone())),
            ("host", Out::Token(self.host.clone())),
        ];
        let times = [
            ("firstStarted", self.first_started),
            ("lastStarted", self.last_started),
            ("lastEnded", self.last_ended),
        ];
        for (name, at) in times {
            if let Some(at) = at {
                fields.push((name, Out::Token(at.to_string())));
            }
        }
        fields.push(("counts", Out::Mapping(self.all.counts.fields())));
        let estimate = self.all.estimate.fields();
        if !estimate.is_empty() {
            fields.push(("estimate", Out::Mapping(estimate)));
        }
        if self.partial {
            fields.push(("partial", Out::Plain(true.to_string())));
        }
        if !self.stages.is_empty() {
            let stages = self
                .stages
                .iter()
                .map(|(stage, counted)| stage_out(Some(stage), counted));
            fields.push(("stages", Out::Sequence(stages.collect())));
        }
        if let Some(counted) = &self.no_stage {
            fields.push(("noStage", stage_out(None, counted)));
        }
        write(&Out::Mapping(fields))
    }
}
