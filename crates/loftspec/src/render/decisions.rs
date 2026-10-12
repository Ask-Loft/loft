//! The decisions section of the Markdown projection: accepted decisions, then open proposals, each
//! in full, then a one-line history of the rejected, superseded and retired ones.

use std::fmt::Write;

use super::Labels;
use super::markdown::{Nodes, escape_inline, node_name};
use crate::decision::{Decision, DecisionState, DriverPayload, NodeRef, StateKind};
use crate::document::{Document, Edge};
use crate::ids::{DecisionId, DriverId, OptionId};

pub(super) fn render(out: &mut String, document: &Document, labels: &Labels, nodes: &Nodes<'_>) {
    if document.decisions.is_empty() {
        return;
    }
    let mut decisions: Vec<&Decision> = document.decisions.iter().collect();
    decisions.sort_by_key(|decision| order(labels, decision.id));
    out.push_str("## Architecture Decisions\n\n");
    let writer = Writer {
        document,
        labels,
        nodes,
    };
    for (heading, kind) in [
        ("Active Architecture Decisions", StateKind::Accepted),
        ("Open Proposals", StateKind::Proposed),
    ] {
        let group: Vec<_> = decisions
            .iter()
            .filter(|decision| decision.state.kind() == kind)
            .collect();
        if group.is_empty() {
            continue;
        }
        let _ = writeln!(out, "### {heading}\n");
        for decision in group {
            writer.decision(out, decision);
        }
    }
    let history: Vec<_> = decisions
        .iter()
        .filter(|decision| {
            !matches!(
                decision.state.kind(),
                StateKind::Accepted | StateKind::Proposed
            )
        })
        .collect();
    if history.is_empty() {
        return;
    }
    out.push_str("### Decision History\n\n");
    for decision in history {
        let suffix = match &decision.state {
            DecisionState::Rejected { rationale } | DecisionState::Retired { rationale, .. } => {
                format!(" · {}", escape_inline(rationale))
            }
            DecisionState::Superseded { successor, .. } => {
                format!(" · successor `{}`", labels.name(*successor))
            }
            DecisionState::Accepted { .. } | DecisionState::Proposed => String::new(),
        };
        let _ = writeln!(
            out,
            "- **{} · {}** — {}{suffix}",
            labels.name(decision.id),
            escape_inline(&decision.title),
            decision.state.kind().as_str()
        );
    }
    out.push('\n');
}

/// Labelled decisions by their number, then unlabelled ones by key.
fn order(labels: &Labels, id: DecisionId) -> (u8, u64, DecisionId) {
    let number = labels
        .label(id)
        .and_then(|label| label.rsplit_once('-')?.1.parse().ok());
    number.map_or((1, 0, id), |number| (0, number, id))
}

struct Writer<'a> {
    document: &'a Document,
    labels: &'a Labels,
    nodes: &'a Nodes<'a>,
}

impl Writer<'_> {
    fn decision(&self, out: &mut String, decision: &Decision) {
        let _ = writeln!(
            out,
            "#### {} · {}\n",
            self.labels.name(decision.id),
            escape_inline(&decision.title)
        );
        let _ = writeln!(out, "Status: `{}`\n", decision.state.kind().as_str());
        heading(out, "Context");
        let _ = writeln!(out, "{}\n", decision.context);
        self.drivers(out, decision);
        options(out, decision);
        self.assessments(out, decision);
        outcome(out, decision, self.labels);
        self.consequences(out, decision);
        self.subjects(out, decision);
    }

    /// `` `<label>` · <name> ``, or `deleted subject` for a former label.
    fn subject(&self, subject: NodeRef) -> String {
        match subject {
            NodeRef::Held(node) => {
                let name = self
                    .nodes
                    .get(&node)
                    .copied()
                    .map(node_name)
                    .unwrap_or_default();
                format!("`{}` · {}", self.labels.name(node), escape_inline(&name))
            }
            NodeRef::Former(label) => format!("`{label}` · deleted subject"),
        }
    }

    fn driver_title(&self, payload: &DriverPayload) -> String {
        match payload {
            DriverPayload::Local { label, .. } => escape_inline(label),
            DriverPayload::Subject { subject } => self.subject(*subject),
        }
    }

    fn drivers(&self, out: &mut String, decision: &Decision) {
        heading(out, "Drivers");
        if decision.drivers.is_empty() {
            out.push_str("_None._\n\n");
            return;
        }
        for (index, driver) in decision.drivers.iter().enumerate() {
            let line = match &driver.payload {
                DriverPayload::Local { label, statement } => {
                    format!(
                        "**{}** — {}",
                        escape_inline(label),
                        escape_inline(statement)
                    )
                }
                DriverPayload::Subject { subject } => self.subject(*subject),
            };
            let _ = writeln!(out, "{}. {line}", index + 1);
        }
        out.push('\n');
    }

    fn assessments(&self, out: &mut String, decision: &Decision) {
        heading(out, "Assessment");
        if decision.options.is_empty() || decision.drivers.is_empty() {
            out.push_str("_None._\n\n");
            return;
        }
        out.push_str("| Option ");
        for driver in &decision.drivers {
            let _ = write!(out, "| {} ", self.driver_title(&driver.payload));
        }
        out.push_str("|\n| --- ");
        out.push_str(&"| --- ".repeat(decision.drivers.len()));
        out.push_str("|\n");
        for option in &decision.options {
            let _ = write!(out, "| {} ", escape_inline(&option.title));
            for driver in &decision.drivers {
                let _ = write!(out, "| {} ", cell(decision, option.id, driver.id));
            }
            out.push_str("|\n");
        }
        out.push('\n');
    }

    fn consequences(&self, out: &mut String, decision: &Decision) {
        heading(out, "Consequences");
        if decision.consequences.is_empty() {
            out.push_str("_None._\n\n");
            return;
        }
        for consequence in &decision.consequences {
            let _ = write!(
                out,
                "- **{}** — {}",
                consequence.kind.as_str(),
                escape_inline(&consequence.statement)
            );
            if let Some(subject) = consequence.subject {
                let _ = write!(out, " ({})", self.subject(subject));
            }
            out.push('\n');
        }
        out.push('\n');
    }

    fn subjects(&self, out: &mut String, decision: &Decision) {
        heading(out, "Subjects");
        let subjects: Vec<_> = self
            .document
            .edges
            .iter()
            .filter_map(|edge| match *edge {
                Edge::Subject {
                    decision: from,
                    subject,
                    role,
                } if from == decision.id => Some((role, subject)),
                _ => None,
            })
            .collect();
        if subjects.is_empty() {
            out.push_str("_None._\n\n");
            return;
        }
        for (role, subject) in subjects {
            let _ = writeln!(out, "- **{}** — {}", role.as_str(), self.subject(subject));
        }
        out.push('\n');
    }
}

/// The rating and rationale of one option against one driver, or `unknown`.
fn cell(decision: &Decision, option: OptionId, driver: DriverId) -> String {
    decision
        .assessments
        .iter()
        .find(|cell| cell.option == option && cell.driver == driver)
        .map_or_else(
            || "unknown".to_owned(),
            |cell| {
                format!(
                    "{} — {}",
                    cell.rating.as_str(),
                    escape_inline(&cell.rationale)
                )
            },
        )
}

fn heading(out: &mut String, label: &str) {
    let _ = writeln!(out, "##### {label}\n");
}

fn options(out: &mut String, decision: &Decision) {
    heading(out, "Options");
    if decision.options.is_empty() {
        out.push_str("_None._\n\n");
        return;
    }
    for (index, option) in decision.options.iter().enumerate() {
        let _ = writeln!(
            out,
            "{}. **{}** — {}",
            index + 1,
            escape_inline(&option.title),
            escape_inline(&option.description)
        );
    }
    out.push('\n');
}

fn outcome(out: &mut String, decision: &Decision, labels: &Labels) {
    heading(out, "Decision");
    let selected = |option| {
        decision
            .options
            .iter()
            .find(|candidate| candidate.id == option)
            .map_or_else(String::new, |option| escape_inline(&option.title))
    };
    match &decision.state {
        DecisionState::Proposed => out.push_str("_Not resolved._\n\n"),
        DecisionState::Rejected { rationale } => {
            let _ = writeln!(out, "Rejected: {rationale}\n");
        }
        DecisionState::Accepted {
            selected_option,
            outcome,
        }
        | DecisionState::Superseded {
            selected_option,
            outcome,
            ..
        }
        | DecisionState::Retired {
            selected_option,
            outcome,
            ..
        } => {
            let _ = writeln!(out, "Selected: **{}**\n", selected(*selected_option));
            let _ = writeln!(out, "{outcome}\n");
            match &decision.state {
                DecisionState::Superseded { successor, .. } => {
                    let _ = writeln!(out, "Superseded by `{}`.\n", labels.name(*successor));
                }
                DecisionState::Retired { rationale, .. } => {
                    let _ = writeln!(out, "Retired: {rationale}\n");
                }
                _ => {}
            }
        }
    }
}
