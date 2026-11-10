//! A session's consultation file as text, `yardconsult/1`, and back: the counts and the estimate
//! in all, and by stage, within it by agent, and under no stage, with each scope's order.

use loft_common::yaml::{Out, write};
use serde_json::Value;

use super::{Consultation, Estimate, Order, Scope, Six, Stage};
use crate::Time;
use crate::totals::Who;

/// The format a consultation file names.
const FORMAT: &str = "yardconsult/1";

/// The fields of `values` under `names`, each written, a zero too.
fn counts_out<const N: usize>(
    names: [&'static str; N],
    values: [u64; N],
) -> Vec<(&'static str, Out)> {
    names
        .into_iter()
        .zip(values)
        .map(|(name, value)| (name, Out::Integer(value)))
        .collect()
}

/// The values `value` holds under `names`, an absent one zero.
fn counts_in<const N: usize>(names: [&str; N], value: &Value) -> [u64; N] {
    names.map(|name| value[name].as_u64().unwrap_or_default())
}

impl Six {
    fn fields(self) -> Vec<(&'static str, Out)> {
        counts_out(Self::NAMES, self.values())
    }

    fn read(value: &Value) -> Self {
        Self::from_values(counts_in(Self::NAMES, value))
    }
}

impl Estimate {
    /// The estimate's fields, each class that took any tokens.
    fn fields(self) -> Vec<(&'static str, Out)> {
        let mut fields = counts_out(Self::NAMES, self.values());
        fields.retain(|(_, tokens)| !matches!(tokens, Out::Integer(0)));
        fields
    }

    fn read(value: &Value) -> Self {
        Self::from_values(counts_in(Self::NAMES, value))
    }
}

impl Order {
    /// The order's fields: each time it holds, and the edits of code before the specification.
    fn fields(self) -> Vec<(&'static str, Out)> {
        let times = [
            ("firstSpec", self.first_spec),
            ("firstCodeEdit", self.first_code_edit),
        ];
        let mut fields: Vec<_> = times
            .into_iter()
            .filter_map(|(name, at)| Some((name, Out::Token(at?.to_string()))))
            .collect();
        fields.push((
            "codeEditsBeforeSpec",
            Out::Integer(self.code_edits_before_spec),
        ));
        fields
    }

    fn read(value: &Value) -> Self {
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

/// The fields of the counts and the estimate, the estimate only where it took any tokens.
fn tallies(counts: Six, estimate: Estimate) -> Vec<(&'static str, Out)> {
    let mut fields = vec![("counts", Out::Mapping(counts.fields()))];
    let estimate = estimate.fields();
    if !estimate.is_empty() {
        fields.push(("estimate", Out::Mapping(estimate)));
    }
    fields
}

impl Scope {
    /// The scope's fields: its counts, its order and its estimate where it took any tokens.
    fn fields(self) -> Vec<(&'static str, Out)> {
        let mut fields = tallies(self.counts, self.estimate);
        fields.insert(1, ("order", Out::Mapping(self.order.fields())));
        fields
    }

    fn read(value: &Value) -> Self {
        Self {
            counts: Six::read(&value["counts"]),
            order: Order::read(&value["order"]),
            estimate: Estimate::read(&value["estimate"]),
        }
    }
}

impl Stage {
    /// The stage's entry: its key, where it has one, its scope in all, and each agent's.
    fn out(&self, key: Option<&str>) -> Out {
        let mut fields = Vec::new();
        if let Some(key) = key {
            fields.push(("stage", Out::Token(key.to_owned())));
        }
        fields.extend(self.all.fields());
        let agents = self.agents.iter().map(|(who, scope)| {
            let mut fields = who.fields();
            fields.extend(scope.fields());
            Out::Mapping(fields)
        });
        fields.push(("agents", Out::Sequence(agents.collect())));
        Out::Mapping(fields)
    }

    fn read(value: &Value) -> Self {
        let agents = value["agents"].as_array().into_iter().flatten();
        Self {
            all: Scope::read(value),
            agents: agents
                .map(|agent| (Who::from_fields(agent), Scope::read(agent)))
                .collect(),
        }
    }
}

impl Consultation {
    /// Whether the consultation file `file`, as JSON, is of the format `yardconsult/1`.
    #[must_use]
    pub fn of_this_format(file: &Value) -> bool {
        file["format"] == FORMAT
    }

    /// Reads a consultation file as JSON, as [`Consultation::text`] wrote it; none where it names
    /// a format other than `yardconsult/1`.
    #[must_use]
    pub fn read(file: &Value) -> Option<Self> {
        if !Self::of_this_format(file) {
            return None;
        }
        let stages = file["stages"].as_array().into_iter().flatten();
        let stages = stages
            .filter_map(|stage| Some((stage["stage"].as_str()?.to_owned(), Stage::read(stage))));
        Some(Self {
            session: file["session"].as_str().unwrap_or_default().to_owned(),
            host: file["host"].as_str().unwrap_or_default().to_owned(),
            first_started: time(file, "firstStarted"),
            last_started: time(file, "lastStarted"),
            last_ended: time(file, "lastEnded"),
            counts: Six::read(&file["counts"]),
            estimate: Estimate::read(&file["estimate"]),
            stages: stages.collect(),
            no_stage: file.get("noStage").map(Stage::read),
            partial: file["partial"].as_bool().unwrap_or_default(),
        })
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
        fields.extend(tallies(self.counts, self.estimate));
        if self.partial {
            fields.push(("partial", Out::Plain(true.to_string())));
        }
        if !self.stages.is_empty() {
            let stages = self.stages.iter().map(|(key, stage)| stage.out(Some(key)));
            fields.push(("stages", Out::Sequence(stages.collect())));
        }
        if let Some(stage) = &self.no_stage {
            fields.push(("noStage", stage.out(None)));
        }
        write(&Out::Mapping(fields))
    }
}
