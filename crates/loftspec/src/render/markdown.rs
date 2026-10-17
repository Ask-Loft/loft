//! The Markdown projection (F-24): a header naming the state root, the Capability Model with
//! Products, Feature Groups, Features and Requirements as headings of levels 3 to 6 and criteria,
//! Constraints and Quality Attributes as bold paragraphs, then the Component Model, the decisions,
//! the tag registry, the constraint categories and the `realizes` edges, each in its own section.

use std::collections::BTreeMap;
use std::fmt::Write;

use super::{Labels, decisions, state_root};
use crate::attributes::{QualityBand, QualityTarget, TechnicalBinding, VerificationFate};
use crate::document::{Document, Edge};
use crate::hash::DocumentHashes;
use crate::ids::{ElementId, NodeId};
use crate::node::Node;
use crate::tree::{Component, Section};

pub(super) fn render(document: &Document, hashes: &DocumentHashes, labels: &Labels) -> String {
    let title = document
        .products
        .first()
        .map_or("Specification", |product| product.name.as_str());
    let mut out = format!(
        "# {}\n\n> {} · state root `{}`\n\n",
        escape_inline(title),
        crate::FORMAT,
        state_root(hashes)
    );
    let mut nodes = BTreeMap::new();
    document.walk(&mut |node| {
        if let Some(id) = node.id() {
            nodes.insert(id, node);
        }
    });
    let mut renderer = Renderer {
        out: &mut out,
        document,
        labels,
        nodes: &nodes,
    };
    renderer.capabilities();
    renderer.components();
    decisions::render(renderer.out, document, labels, &nodes);
    renderer.registries();
    renderer.realizations();
    while out.ends_with("\n\n") {
        out.pop();
    }
    if !out.ends_with('\n') {
        out.push('\n');
    }
    out
}

struct Renderer<'a> {
    out: &'a mut String,
    document: &'a Document,
    labels: &'a Labels,
    nodes: &'a Nodes<'a>,
}

/// Every keyed node of the document, by ID.
pub(super) type Nodes<'a> = BTreeMap<NodeId, Node<'a>>;

impl Renderer<'_> {
    fn none_if_empty(&mut self, empty: bool) -> bool {
        if empty {
            self.out.push_str("_None._\n\n");
        }
        empty
    }

    fn capabilities(&mut self) {
        self.out.push_str("## Capability Model\n\n");
        if self.none_if_empty(self.document.products.is_empty()) {
            return;
        }
        for product in &self.document.products {
            self.capability(Node::Product(product), 3);
        }
    }

    /// A capability node, its overlays, then its structural children one level down.
    fn capability(&mut self, node: Node<'_>, level: usize) {
        self.heading(node, level);
        self.body_tags_and_sections(node, level);
        let children = node.children();
        for child in children.iter().filter(|child| is_overlay(**child)) {
            self.overlay(*child);
        }
        for child in children.iter().filter(|child| is_structural(**child)) {
            self.capability(*child, (level + 1).min(6));
        }
    }

    fn components(&mut self) {
        self.out.push_str("## Component Model\n\n");
        if self.none_if_empty(self.document.components.is_empty()) {
            return;
        }
        for root in &self.document.components {
            self.component(&root.component, 3);
        }
    }

    fn component(&mut self, component: &Component, level: usize) {
        let node = Node::Component(component);
        self.heading(node, level);
        let scopes = self.document.edges.iter().filter_map(|edge| match *edge {
            Edge::ScopedTo {
                component: from,
                product,
            } if Some(from) == component.id => Some(product),
            _ => None,
        });
        for product in scopes {
            let _ = writeln!(self.out, "Scope: Product `{}`\n", self.labels.name(product));
        }
        if let Some(boundary) = component.boundary {
            let _ = writeln!(self.out, "Boundary: `{}`\n", boundary.as_str());
        }
        self.body_tags_and_sections(node, level);
        for child in node
            .children()
            .into_iter()
            .filter(|child| is_overlay(*child))
        {
            self.overlay(child);
        }
        for child in &component.components {
            self.component(child, (level + 1).min(6));
        }
    }

    fn heading(&mut self, node: Node<'_>, level: usize) {
        let name = node.name();
        let label = node
            .id()
            .map_or_else(String::new, |id| self.labels.name(id));
        let _ = write!(
            self.out,
            "{} {label} · {}\n\n",
            "#".repeat(level),
            escape_inline(&name)
        );
    }

    /// A node's body, its tags and its Sections, which render as headings one level down.
    fn body_tags_and_sections(&mut self, node: Node<'_>, level: usize) {
        let Some(details) = node.details() else {
            return;
        };
        body(self.out, details.body.as_deref());
        if !details.tags.is_empty() {
            let tags: Vec<String> = details.tags.iter().map(|tag| format!("`{tag}`")).collect();
            let _ = writeln!(self.out, "Tags: {}\n", tags.join(", "));
        }
        for section in &details.sections {
            self.section(section, (level + 1).min(6));
        }
    }

    fn section(&mut self, section: &Section, level: usize) {
        let _ = write!(
            self.out,
            "{} {}\n\n",
            "#".repeat(level),
            escape_inline(&section.name)
        );
        body(self.out, Some(&section.body));
        for child in &section.sections {
            self.section(child, (level + 1).min(6));
        }
    }

    /// A criterion, Constraint or Quality Attribute: `**<label> · <kind> · <name>**`.
    fn overlay(&mut self, node: Node<'_>) {
        let label = node
            .id()
            .map_or_else(String::new, |id| self.labels.name(id));
        let _ = write!(
            self.out,
            "**{label} · {} · {}**\n\n",
            kind_label(node),
            escape_inline(&node.name())
        );
        match node {
            Node::Constraint(constraint) => {
                if let Some(binding) = &constraint.binding {
                    technical_binding(self.out, binding);
                }
            }
            Node::QualityAttribute(attribute) => quality_target(
                self.out,
                attribute.kind.as_str(),
                &attribute.target,
                attribute.verification_fate,
            ),
            _ => {}
        }
        self.body_tags_and_sections(node, 6);
    }

    fn registries(&mut self) {
        self.out.push_str("## Domain Vocabulary\n\n");
        let mut tags: Vec<_> = self.document.tags.iter().collect();
        tags.sort_by_key(|entry| (entry.name.as_str(), entry.id));
        if !self.none_if_empty(tags.is_empty()) {
            for entry in tags {
                let name = escape_inline(entry.name.as_str());
                let _ = write!(self.out, "### {name}\n\n{}\n\n", entry.definition);
            }
        }
        self.glossary();
        self.out.push_str("## Technical Constraint Categories\n\n");
        let mut categories: Vec<_> = self.document.categories.iter().collect();
        categories.sort_by_key(|entry| (entry.name.as_str(), entry.id));
        if !self.none_if_empty(categories.is_empty()) {
            for entry in categories {
                let _ = write!(self.out, "### {}\n\n{}\n\n", entry.name, entry.definition);
            }
        }
    }

    /// The term list, after the tag registry; no section for a document without a term.
    fn glossary(&mut self) {
        if self.document.terms.is_empty() {
            return;
        }
        self.out.push_str("## Glossary\n\n");
        let mut terms: Vec<_> = self.document.terms.iter().collect();
        terms.sort_by_key(|term| (term.name.as_str(), term.id));
        for term in terms {
            let name = escape_inline(&term.name);
            let _ = write!(self.out, "### {name}\n\n{}\n\n", term.definition);
            if !term.aliases.is_empty() {
                let mut aliases: Vec<String> =
                    term.aliases.iter().map(|a| escape_inline(a)).collect();
                aliases.sort_unstable();
                let _ = write!(self.out, "Aliases: {}\n\n", aliases.join(", "));
            }
        }
    }

    fn realizations(&mut self) {
        self.out.push_str("## Realization Links\n\n");
        let mut links: Vec<_> = self
            .document
            .edges
            .iter()
            .filter_map(|edge| match *edge {
                Edge::Realizes { feature, component } => Some((feature, component)),
                _ => None,
            })
            .collect();
        links.sort_unstable();
        if links.is_empty() {
            self.out.push_str("_None._\n");
            return;
        }
        self.out
            .push_str("| Feature | Component |\n| --- | --- |\n");
        for (feature, component) in links {
            let feature = self.link_label(feature.into());
            let component = self.link_label(component.into());
            let _ = writeln!(self.out, "| {feature} | {component} |");
        }
    }

    fn link_label(&self, id: NodeId) -> String {
        let name = self
            .nodes
            .get(&id)
            .copied()
            .map(|node| node.name().into_owned())
            .unwrap_or_default();
        format!(
            "{} · {}",
            self.labels.name(ElementId::Node(id)),
            escape_inline(&name)
        )
    }
}

const fn is_overlay(node: Node<'_>) -> bool {
    matches!(
        node,
        Node::Criterion(_) | Node::Constraint(_) | Node::QualityAttribute(_)
    )
}

const fn is_structural(node: Node<'_>) -> bool {
    matches!(
        node,
        Node::FeatureGroup(_) | Node::Feature(_) | Node::Requirement(_)
    )
}

const fn kind_label(node: Node<'_>) -> &'static str {
    match node {
        Node::Criterion(_) => "Acceptance Criterion",
        Node::Constraint(_) => "Constraint",
        Node::QualityAttribute(_) => "Quality Attribute",
        _ => "Node",
    }
}

fn body(out: &mut String, body: Option<&str>) {
    if let Some(body) = body {
        out.push_str(body);
        if !body.ends_with('\n') {
            out.push('\n');
        }
        out.push('\n');
    }
}

fn technical_binding(out: &mut String, binding: &TechnicalBinding) {
    let value = serde_json::to_string(&binding.required_value).expect("a string serializes");
    let _ = writeln!(
        out,
        "Technical constraint: `{}` requires {}\n",
        binding.category,
        code_span(&value)
    );
}

fn quality_target(
    out: &mut String,
    kind: &str,
    target: &QualityTarget,
    fate: Option<VerificationFate>,
) {
    let band = match &target.band {
        QualityBand::AtMost { upper } => format!("<= {}", upper.as_str()),
        QualityBand::AtLeast { lower } => format!(">= {}", lower.as_str()),
        QualityBand::Exactly { value } => format!("= {}", value.as_str()),
        QualityBand::Between { lower, upper } => {
            format!("[{}, {}]", lower.as_str(), upper.as_str())
        }
    };
    let fate = fate.map_or_else(String::new, |fate| format!("; fate `{}`", fate.as_str()));
    let _ = writeln!(
        out,
        "Quality target: {}{fate}; metric {}; aggregation {}; band {} {}; scenario {}; environment {}\n",
        code_span(kind),
        code_span(&target.metric),
        code_span(target.aggregation.as_str()),
        code_span(&band),
        code_span(&target.unit),
        code_span(&target.scenario),
        code_span(&target.environment),
    );
}

/// A code span whose fence is one backtick longer than the longest run inside.
pub(super) fn code_span(value: &str) -> String {
    let longest = value.split(|c| c != '`').map(str::len).max().unwrap_or(0);
    let fence = "`".repeat(longest + 1);
    format!("{fence}{value}{fence}")
}

/// A name written inline: backslashes and Markdown markers escaped, line breaks as spaces.
pub(super) fn escape_inline(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut copied = 0;
    for span in crate::text::code_spans(value) {
        out.push_str(&escape_prose(&value[copied..span.start]));
        out.push_str(&escape_code(&value[span.clone()]));
        copied = span.end;
    }
    out.push_str(&escape_prose(&value[copied..]));
    out
}

/// Text outside code spans: every character Markdown would read as markup.
fn escape_prose(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(['\n', '\r'], " ")
        .replace('|', "\\|")
        .replace('*', "\\*")
        .replace('_', "\\_")
        .replace('[', "\\[")
        .replace(']', "\\]")
        .replace('#', "\\#")
}

/// A code span, which Markdown shows as written: only a line break and `|`, which would end a
/// table cell, change.
fn escape_code(value: &str) -> String {
    value.replace(['\n', '\r'], " ").replace('|', "\\|")
}
