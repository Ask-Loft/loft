//! The sums of a Yard Log's consultation files, by the rules that count one: in all, by stage, by
//! session and by agent type. A stage's sum takes the earliest first consultation of the
//! specification and the earliest first edit of code among the files, and the sum of each
//! session's edits of code before its consultation, since each session's agents consult with
//! their own context. A file of another format is left out and marks the sums partial; a session
//! whose adapter does not declare `consults` is listed as unmeasured, and adds nothing.

use std::collections::BTreeMap;

use serde_json::Value;

use super::{Consultation, Estimate, Order, Scope, Six, Stage};
use crate::adapter::Descriptor;
use crate::totals::Who;

/// One session's sums: its file in all.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionSum {
    pub session: String,
    pub host: String,
    pub counts: Six,
    pub estimate: Estimate,
    pub partial: bool,
}

/// The sums of one Yard Log's consultation files.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sums {
    pub counts: Six,
    pub estimate: Estimate,
    pub stages: BTreeMap<String, Scope>,
    pub no_stage: Option<Scope>,
    pub sessions: Vec<SessionSum>,
    /// By agent type: `main`, or a subagent's type.
    pub agents: BTreeMap<String, Scope>,
    /// Each session, with its host, whose adapter does not declare `consults`.
    pub unmeasured: Vec<(String, String)>,
    /// Whether a file's estimate is partial, or a file of another format was left out.
    pub partial: bool,
}

impl Order {
    /// Adds the order of another session's scope: the earlier of each time, and both counts.
    fn join(&mut self, other: Self) {
        let earlier = |left: Option<_>, right: Option<_>| left.into_iter().chain(right).min();
        self.first_spec = earlier(self.first_spec, other.first_spec);
        self.first_code_edit = earlier(self.first_code_edit, other.first_code_edit);
        self.code_edits_before_spec += other.code_edits_before_spec;
    }
}

impl Scope {
    fn join(&mut self, other: Self) {
        self.counts.add(other.counts);
        self.order.join(other.order);
        self.estimate.add(other.estimate);
    }
}

/// The name a sum by agent type gives an agent: `main`, or a subagent's type.
fn agent_type(who: &Who) -> String {
    match who {
        Who::Main => "main".to_owned(),
        Who::Sub { kind, .. } => kind.clone(),
    }
}

impl Sums {
    /// The sums of the consultation files `files` and the activity files `activities` of one Yard
    /// Log, each as JSON.
    #[must_use]
    pub fn of(files: &[Value], activities: &[Value]) -> Self {
        let mut sums = Self::default();
        for file in files {
            match Consultation::read(file) {
                Some(consultation) => sums.add(&consultation),
                None => sums.partial = true,
            }
        }
        for activity in activities {
            sums.describe(activity);
        }
        sums
    }

    /// Whether any session's consultation was counted.
    #[must_use]
    pub fn measured(&self) -> bool {
        !self.sessions.is_empty()
    }

    /// Adds one session's consultation.
    pub fn add(&mut self, file: &Consultation) {
        self.counts.add(file.counts);
        self.estimate.add(file.estimate);
        self.partial |= file.partial;
        for (key, stage) in &file.stages {
            self.stages.entry(key.clone()).or_default().join(stage.all);
            self.add_agents(stage);
        }
        if let Some(stage) = &file.no_stage {
            self.no_stage
                .get_or_insert_with(Scope::default)
                .join(stage.all);
            self.add_agents(stage);
        }
        self.sessions.push(SessionSum {
            session: file.session.clone(),
            host: file.host.clone(),
            counts: file.counts,
            estimate: file.estimate,
            partial: file.partial,
        });
    }

    fn add_agents(&mut self, stage: &Stage) {
        for (who, scope) in &stage.agents {
            self.agents.entry(agent_type(who)).or_default().join(*scope);
        }
    }

    /// Lists the session of the activity file `activity` as unmeasured where its adapter's
    /// descriptor does not declare `consults`.
    pub fn describe(&mut self, activity: &Value) {
        let consults =
            Descriptor::read(&activity["adapter"]).is_ok_and(|adapter| adapter.consults());
        if !consults {
            let text = |name: &str| activity[name].as_str().unwrap_or_default().to_owned();
            self.unmeasured.push((text("session"), text("host")));
        }
    }
}
