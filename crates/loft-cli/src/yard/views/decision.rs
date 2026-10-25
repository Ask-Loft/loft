//! The `decision` view: a decision of the specification as the tree holds it, named by its slug,
//! its key or its label: its title, label and state, its context and outcome, its drivers,
//! options and assessments, its consequences, and its subjects, each node by its key, label,
//! kind and name.

use std::collections::BTreeMap;

use loftspec::codec::{self, Files};
use loftspec::decision::{Decision, DecisionState, Driver, DriverPayload, NodeRef};
use loftspec::document::{Document, Edge};
use loftspec::identity::Key;
use loftspec::ids::OptionId;
use loftspec::index::selected_option;
use loftspec::query::{element_fields, identifies};
use serde_json::{Map, Value, json};

use crate::stop::Stop;

/// The report of a `decision` view of the decision `named` names in the tree of `files`.
pub fn report(files: &Files, named: &str) -> Result<Value, Stop> {
    let (document, _) = codec::read_sited(files)?;
    let found: Vec<&Decision> = document
        .decisions
        .iter()
        .filter(|decision| {
            let key = decision.id.key();
            let label = document.bindings.label_of(decision.id);
            identifies(named, key, label, Some(decision.slug.as_str()))
        })
        .collect();
    let decision = match found.as_slice() {
        [decision] => *decision,
        [] => return Err(Stop::one("view", format!("`{named}` names no decision"))),
        _ => {
            let message = format!("`{named}` names more than one decision");
            return Err(Stop::one("view", message));
        }
    };
    Ok(json!({ "decision": Nodes::of(&document).decision(decision) }))
}

/// Each keyed node of a document by its key, with its kind and name, for naming a subject.
struct Nodes<'a> {
    document: &'a Document,
    by_key: BTreeMap<Key, (&'static str, String)>,
}

impl<'a> Nodes<'a> {
    fn of(document: &'a Document) -> Self {
        let mut by_key = BTreeMap::new();
        document.walk(&mut |node| {
            if let Some(id) = node.id() {
                by_key.insert(id.key(), (node.kind().as_str(), node.name().into_owned()));
            }
        });
        Self { document, by_key }
    }

    /// A node a decision names: by its key, label, kind and name; or by the label it had, where
    /// the document no longer holds it.
    fn node(&self, node: NodeRef) -> Value {
        match node {
            NodeRef::Held(id) => {
                let key = id.key();
                let (kind, name) = self.by_key.get(&key).cloned().unwrap_or_default();
                let label = self.document.bindings.label_of(id);
                Value::Object(element_fields(key, label, kind, None, &name))
            }
            NodeRef::Former(label) => json!({ "formerLabel": label.to_string() }),
        }
    }

    fn decision(&self, decision: &Decision) -> Value {
        let selected = selected_option(&decision.state).map(OptionId::key);
        let label = self.document.bindings.label_of(decision.id);
        let mut fields = Map::new();
        fields.insert("key".to_owned(), json!(decision.id.key().to_string()));
        fields.insert(
            "label".to_owned(),
            json!(label.map(|label| label.to_string())),
        );
        fields.insert("slug".to_owned(), json!(decision.slug.as_str()));
        fields.insert("title".to_owned(), json!(decision.title));
        fields.insert("context".to_owned(), json!(decision.context));
        fields.insert("state".to_owned(), state(&decision.state));
        let drivers = decision.drivers.iter().map(|driver| self.driver(driver));
        fields.insert("drivers".to_owned(), drivers.collect());
        let options = decision.options.iter().map(|option| {
            json!({
                "key": option.id.key().to_string(),
                "title": option.title,
                "description": option.description,
                "selected": selected == Some(option.id.key()),
            })
        });
        fields.insert("options".to_owned(), options.collect());
        let assessments = decision.assessments.iter().map(|assessment| {
            json!({
                "option": assessment.option.key().to_string(),
                "driver": assessment.driver.key().to_string(),
                "rating": assessment.rating.as_str(),
                "rationale": assessment.rationale,
            })
        });
        fields.insert("assessments".to_owned(), assessments.collect());
        let consequences = decision.consequences.iter().map(|consequence| {
            let mut fields = json!({
                "key": consequence.id.key().to_string(),
                "kind": consequence.kind.as_str(),
                "statement": consequence.statement,
            });
            if let Some(subject) = consequence.subject {
                fields["subject"] = self.node(subject);
            }
            fields
        });
        fields.insert("consequences".to_owned(), consequences.collect());
        fields.insert("subjects".to_owned(), self.subjects(decision));
        Value::Object(fields)
    }

    /// A driver: stated in the decision by its label and statement, or naming a node.
    fn driver(&self, driver: &Driver) -> Value {
        let mut fields = json!({
            "key": driver.id.key().to_string(),
            "kind": driver.payload.kind().as_str(),
        });
        match &driver.payload {
            DriverPayload::Local { label, statement } => {
                fields["label"] = json!(label);
                fields["statement"] = json!(statement);
            }
            DriverPayload::Subject { subject } => fields["node"] = self.node(*subject),
        }
        fields
    }

    /// The nodes the decision names as its subjects, each with its role.
    fn subjects(&self, decision: &Decision) -> Value {
        self.document
            .edges
            .iter()
            .filter_map(|edge| match edge {
                Edge::Subject {
                    decision: of,
                    subject,
                    role,
                } if *of == decision.id => {
                    Some(json!({ "role": role.as_str(), "node": self.node(*subject) }))
                }
                _ => None,
            })
            .collect()
    }
}

/// The decision's state: its kind, and the outcome, rationale or successor it records.
fn state(state: &DecisionState) -> Value {
    let mut fields = json!({ "kind": state.kind().as_str() });
    if let Some(outcome) = state.outcome() {
        fields["outcome"] = json!(outcome);
    }
    match state {
        DecisionState::Rejected { rationale } | DecisionState::Retired { rationale, .. } => {
            fields["rationale"] = json!(rationale);
        }
        DecisionState::Superseded { successor, .. } => {
            fields["successor"] = json!(successor.key().to_string());
        }
        DecisionState::Proposed | DecisionState::Accepted { .. } => {}
    }
    fields
}
