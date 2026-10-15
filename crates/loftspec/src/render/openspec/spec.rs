//! One Feature's `spec.md`: its heading, its Purpose, each Requirement with its statement,
//! metadata lines, body, scenarios and Sections, then the Feature's own Sections.

use std::ops::Range;

use serde::Serialize;

use crate::convert::openspec::ext;
use crate::identity::Key;
use crate::ids::{ElementId, RequirementId};
use crate::render::Labels;
use crate::statement::Statement;
use crate::tree::{Feature, Requirement, Section};

/// A heading the render map records, by the name the map gives it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum Mark {
    Requirement,
    Scenario,
}

/// A rendered requirement or scenario heading: what it renders, its 1-based line and its node.
pub(super) struct Marked {
    pub mark: Mark,
    pub line: usize,
    pub element: Option<ElementId>,
}

/// A rendered requirement: its node, the name its heading gives, the line of its heading and
/// the lines after the heading that belong to it.
pub(super) struct Written {
    pub key: Option<Key>,
    pub name: String,
    pub line: usize,
    pub lines: Range<usize>,
}

/// A Feature's `spec.md` and what the renderer reports from it.
pub(super) struct Spec {
    pub lines: Vec<String>,
    pub purpose_line: usize,
    pub marked: Vec<Marked>,
    pub requirements: Vec<Written>,
}

pub(super) fn write(feature: &Feature, labels: &Labels) -> Spec {
    let mut writer = Writer {
        labels,
        lines: Vec::new(),
        marked: Vec::new(),
        requirements: Vec::new(),
    };
    writer.heading(1, &format!("{} Specification", feature.name));
    let purpose_line = writer.heading(2, "Purpose");
    let purpose = feature
        .details
        .body
        .as_deref()
        .or_else(|| ext::purpose(&feature.details.ext));
    writer.text(purpose.unwrap_or_default());
    writer.heading(2, "Requirements");
    for requirement in &feature.requirements {
        writer.requirement(requirement);
    }
    for section in &feature.details.sections {
        writer.section(section, 2);
    }
    Spec {
        lines: writer.lines,
        purpose_line,
        marked: writer.marked,
        requirements: writer.requirements,
    }
}

struct Writer<'a> {
    labels: &'a Labels,
    lines: Vec<String>,
    marked: Vec<Marked>,
    requirements: Vec<Written>,
}

impl Writer<'_> {
    /// A heading after a blank line, with a blank line after it when its level is 1 or 2; its
    /// 1-based line.
    fn heading(&mut self, level: usize, title: &str) -> usize {
        self.blank();
        self.lines
            .push(format!("{} {title}", "#".repeat(level.min(6))));
        let line = self.lines.len();
        if level <= 2 {
            self.lines.push(String::new());
        }
        line
    }

    fn blank(&mut self) {
        if self.lines.last().is_some_and(|line| !line.is_empty()) {
            self.lines.push(String::new());
        }
    }

    /// Each line of `text` as written.
    fn text(&mut self, text: &str) {
        self.lines.extend(text.lines().map(str::to_owned));
    }

    fn requirement(&mut self, requirement: &Requirement) {
        let key = requirement.id.map(RequirementId::key);
        let name = self.name(requirement);
        let line = self.heading(3, &format!("Requirement: {name}"));
        self.mark(Mark::Requirement, line, requirement.id.map(ElementId::from));
        let start = self.lines.len();
        match &requirement.statement {
            Statement::Form(form) => {
                self.text(&form.render_with_modal(ext::modal(&requirement.details.ext)));
            }
            Statement::Text(text) => self.text(text),
        }
        for (label, value) in ext::metadata(&requirement.details.ext) {
            self.lines.push(format!("**{label}**: {value}"));
        }
        if let Some(body) = &requirement.details.body {
            self.blank();
            self.text(body);
        }
        for criterion in &requirement.criteria {
            let line = self.heading(4, &format!("Scenario: {}", criterion.name));
            self.mark(Mark::Scenario, line, criterion.id.map(ElementId::from));
            match &criterion.details.body {
                Some(body) => self.text(body),
                None => self.lines.push(format!("- **THEN** {}", criterion.name)),
            }
        }
        for section in &requirement.details.sections {
            self.section(section, 4);
        }
        self.requirements.push(Written {
            key,
            name,
            line,
            lines: start..self.lines.len(),
        });
    }

    /// The name a Requirement's heading gives: its `ext.openspec.name`, else its label, else
    /// its key; a Requirement with none of them is named by its statement.
    fn name(&self, requirement: &Requirement) -> String {
        if let Some(name) = ext::name(&requirement.details.ext) {
            return name.to_owned();
        }
        requirement
            .id
            .map_or_else(|| requirement.statement.render(), |id| self.labels.name(id))
    }

    fn section(&mut self, section: &Section, level: usize) {
        self.heading(level, &section.name);
        self.text(&section.body);
        for child in &section.sections {
            self.section(child, level + 1);
        }
    }

    fn mark(&mut self, mark: Mark, line: usize, element: Option<ElementId>) {
        self.marked.push(Marked {
            mark,
            line,
            element,
        });
    }
}
