//! One Feature's `spec.md`: its heading, its Purpose, each Requirement with its statement,
//! metadata lines, body, scenarios and Sections, then the Feature's own Sections.

use std::ops::Range;

use serde::Serialize;

use crate::convert::openspec::ext;
use crate::convert::openspec::read::{fence_mask, heading_level};
use crate::convert::openspec::statement::metadata_entry;
use crate::identity::Key;
use crate::ids::{CriterionId, ElementId, FeatureId, RequirementId, SectionId};
use crate::render::Labels;
use crate::statement::Statement;
use crate::tree::{Feature, Requirement, Section};

/// The deepest heading level Markdown has.
const MAX_LEVEL: usize = 6;

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

/// What a node holds that a second import would read differently.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum Unreadable {
    Heading,
    Metadata,
    LineBreak,
    Depth,
}

impl Unreadable {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Heading => "a body line that is a heading",
            Self::Metadata => "a Requirement body line that opens with a bold label and a colon",
            Self::LineBreak => "a name or a text statement that holds a line break",
            Self::Depth => "a Section deeper than level 6",
        }
    }
}

/// A node whose content a second import would read differently: its key, the line of its
/// heading and what it holds.
pub(super) struct Unread {
    pub key: Option<Key>,
    pub line: usize,
    pub kind: Unreadable,
}

/// A written Section: its key, the line and level of its heading and its name.
pub(super) struct WrittenSection {
    pub key: Option<Key>,
    pub line: usize,
    pub level: usize,
    pub name: String,
}

/// A Feature's `spec.md` and what the renderer reports from it.
pub(super) struct Spec {
    pub lines: Vec<String>,
    pub purpose_line: usize,
    /// The Purpose as written.
    pub purpose: String,
    pub marked: Vec<Marked>,
    pub requirements: Vec<Written>,
    pub sections: Vec<WrittenSection>,
    pub unread: Vec<Unread>,
}

pub(super) fn write(feature: &Feature, labels: &Labels) -> Spec {
    let mut writer = Writer {
        labels,
        lines: Vec::new(),
        marked: Vec::new(),
        requirements: Vec::new(),
        sections: Vec::new(),
        unread: Vec::new(),
    };
    let key = feature.id.map(FeatureId::key);
    let line = writer.heading(1, &format!("{} Specification", feature.name));
    writer.named(&feature.name, key, line);
    let purpose_line = writer.heading(2, "Purpose");
    let purpose = feature
        .details
        .body
        .as_deref()
        .or_else(|| ext::purpose(&feature.details.ext))
        .unwrap_or_default();
    writer.body(purpose, key, line, false);
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
        purpose: purpose.to_owned(),
        marked: writer.marked,
        requirements: writer.requirements,
        sections: writer.sections,
        unread: writer.unread,
    }
}

struct Writer<'a> {
    labels: &'a Labels,
    lines: Vec<String>,
    marked: Vec<Marked>,
    requirements: Vec<Written>,
    sections: Vec<WrittenSection>,
    unread: Vec<Unread>,
}

impl Writer<'_> {
    /// A heading after a blank line, with a blank line after it when its level is 1 or 2; its
    /// 1-based line.
    fn heading(&mut self, level: usize, title: &str) -> usize {
        self.blank();
        self.lines
            .push(format!("{} {title}", "#".repeat(level.min(MAX_LEVEL))));
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

    /// The body of the node `key`, whose heading is at `line`, as written, noting each heading
    /// outside fenced code and, in a Requirement's body, each metadata line.
    fn body(&mut self, body: &str, key: Option<Key>, line: usize, requirement: bool) {
        let lines: Vec<String> = body.lines().map(str::to_owned).collect();
        for (text, fenced) in lines.iter().zip(fence_mask(&lines)) {
            if fenced {
                continue;
            }
            if heading_level(text).is_some() {
                self.note(key, line, Unreadable::Heading);
            }
            if requirement && metadata_entry(text).is_some() {
                self.note(key, line, Unreadable::Metadata);
            }
        }
        self.lines.extend(lines);
    }

    /// Notes a name or a text statement of the node `key` that holds a line break.
    fn named(&mut self, name: &str, key: Option<Key>, line: usize) {
        if name.contains('\n') {
            self.note(key, line, Unreadable::LineBreak);
        }
    }

    fn note(&mut self, key: Option<Key>, line: usize, kind: Unreadable) {
        self.unread.push(Unread { key, line, kind });
    }

    fn requirement(&mut self, requirement: &Requirement) {
        let key = requirement.id.map(RequirementId::key);
        let name = self.name(requirement);
        let line = self.heading(3, &format!("Requirement: {name}"));
        self.named(&name, key, line);
        self.mark(Mark::Requirement, line, requirement.id.map(ElementId::from));
        let start = self.lines.len();
        match &requirement.statement {
            Statement::Form(form) => {
                self.text(&form.render_with_modal(ext::modal(&requirement.details.ext)));
            }
            Statement::Text(text) => {
                self.named(text, key, line);
                self.text(text);
            }
        }
        for (label, value) in ext::metadata(&requirement.details.ext) {
            self.lines.push(format!("**{label}**: {value}"));
        }
        if let Some(body) = &requirement.details.body {
            self.blank();
            self.body(body, key, line, true);
        }
        for criterion in &requirement.criteria {
            let key = criterion.id.map(CriterionId::key);
            let line = self.heading(4, &format!("Scenario: {}", criterion.name));
            self.named(&criterion.name, key, line);
            self.mark(Mark::Scenario, line, criterion.id.map(ElementId::from));
            match &criterion.details.body {
                Some(body) => self.body(body, key, line, false),
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
        let key = section.id.map(SectionId::key);
        let line = self.heading(level, &section.name);
        self.named(&section.name, key, line);
        if level > MAX_LEVEL {
            self.note(key, line, Unreadable::Depth);
        }
        self.sections.push(WrittenSection {
            key,
            line,
            level: level.min(MAX_LEVEL),
            name: section.name.clone(),
        });
        self.body(&section.body, key, line, false);
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
