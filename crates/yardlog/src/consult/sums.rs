//! The sums of a Yard Log's consultation files, by the rules that count one: in all, by stage, by
//! session and by agent type. A stage's sum takes the earliest first consultation of the
//! specification and the earliest first edit of code among the files, and the sum of each
//! session's edits of code before its consultation, since each session's agents consult with
//! their own context. A session whose adapter does not declare `consults` is listed as
//! unmeasured, and adds nothing.

use std::collections::BTreeMap;

use serde_json::{Value, json};

use super::{Consultation, Estimate, Order, Scope, Six, Stage};
use crate::totals::Who;

impl Six {
    /// The six counts as JSON.
    #[must_use]
    pub fn json(self) -> Value {
        json!({
            "specReads": self.spec_reads, "codeReads": self.code_reads,
            "otherReads": self.other_reads, "loftReads": self.loft_reads,
            "specEdits": self.spec_edits, "codeEdits": self.code_edits,
        })
    }
}

impl Estimate {
    /// The estimate as JSON, each class written.
    #[must_use]
    pub fn json(self) -> Value {
        json!({
            "spec": self.spec, "code": self.code,
            "other": self.other, "unclassified": self.unclassified,
        })
    }
}

impl Order {
    /// Adds the order of another session's scope: the earlier of each time, and both counts.
    fn join(&mut self, other: Self) {
        let earlier = |left: Option<_>, right: Option<_>| match (left, right) {
            (Some(left), Some(right)) => Some(std::cmp::min(left, right)),
            (left, right) => left.or(right),
        };
        self.first_spec = earlier(self.first_spec, other.first_spec);
        self.first_code_edit = earlier(self.first_code_edit, other.first_code_edit);
        self.code_edits_before_spec += other.code_edits_before_spec;
    }

    /// The order as JSON, with whether the specification was consulted before the code was
    /// first edited, where the code was edited at all.
    #[must_use]
    pub fn json(self) -> Value {
        json!({
            "firstSpec": self.first_spec.map(|at| at.to_string()),
            "firstCodeEdit": self.first_code_edit.map(|at| at.to_string()),
            "codeEditsBeforeSpec": self.code_edits_before_spec,
            "specFirst": self.first_code_edit.map(|_| self.code_edits_before_spec == 0),
        })
    }
}

impl Scope {
    fn join(&mut self, other: Self) {
        self.counts.add(other.counts);
        self.order.join(other.order);
        self.estimate.add(other.estimate);
    }

    fn json(self) -> Value {
        json!({
            "counts": self.counts.json(),
            "order": self.order.json(),
            "estimate": self.estimate.json(),
        })
    }
}

/// The name a sum by agent type gives an agent: `main`, or a subagent's type.
fn agent_type(who: &Who) -> String {
    match who {
        Who::Main => "main".to_owned(),
        Who::Sub { kind, .. } => kind.clone(),
    }
}

/// Whether the descriptor an activity file keeps declares that its adapter classifies tool calls.
fn consults(activity: &Value) -> bool {
    activity["adapter"]["capabilities"]["consults"] == true
}

/// The sums of the consultation files `files` and the activity files `activities` of one Yard
/// Log, each as JSON.
#[must_use]
pub fn sums(files: &[Value], activities: &[Value]) -> Value {
    let read: Vec<Consultation> = files.iter().map(Consultation::read).collect();
    let mut all = Scope::default();
    let mut stages: BTreeMap<String, Scope> = BTreeMap::new();
    let mut no_stage: Option<Scope> = None;
    let mut agents: BTreeMap<String, Scope> = BTreeMap::new();
    let mut add_agents = |stage: &Stage| {
        for (who, scope) in &stage.agents {
            agents.entry(agent_type(who)).or_default().join(*scope);
        }
    };
    for file in &read {
        all.counts.add(file.all.counts);
        all.estimate.add(file.all.estimate);
        for (key, stage) in &file.stages {
            stages.entry(key.clone()).or_default().join(stage.all);
            add_agents(stage);
        }
        if let Some(stage) = &file.no_stage {
            no_stage.get_or_insert_with(Scope::default).join(stage.all);
            add_agents(stage);
        }
    }
    let sessions = read.iter().map(|file| {
        json!({
            "session": file.session, "host": file.host,
            "counts": file.all.counts.json(), "estimate": file.all.estimate.json(),
            "partial": file.partial,
        })
    });
    let unmeasured = activities
        .iter()
        .filter(|activity| !consults(activity))
        .map(|activity| json!({ "session": activity["session"], "host": activity["host"] }));
    json!({
        "measured": !read.is_empty(),
        "partial": read.iter().any(|file| file.partial),
        "all": { "counts": all.counts.json(), "estimate": all.estimate.json() },
        "stages": stages.into_iter().map(|(key, scope)| {
            let mut stage = scope.json();
            stage["stage"] = json!(key);
            stage
        }).collect::<Vec<_>>(),
        "noStage": no_stage.map(Scope::json),
        "sessions": sessions.collect::<Vec<_>>(),
        "agents": agents.into_iter().map(|(agent, scope)| {
            json!({ "agent": agent, "counts": scope.counts.json(), "estimate": scope.estimate.json() })
        }).collect::<Vec<_>>(),
        "unmeasured": unmeasured.collect::<Vec<_>>(),
    })
}
