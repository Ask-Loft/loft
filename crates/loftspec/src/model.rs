//! The document model (CMP-2): nodes in their tree, edges between keys, the registries, the
//! decisions, the bindings and the record of a v1 source.

use crate::attributes::Attributes;
use crate::bindings::Bindings;
use crate::identity::Key;
use crate::kind::NodeKind;
use crate::statement::Statement;

/// What a node says in one line: a Requirement's statement, or any other node's name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Title {
    Name(String),
    Statement(Statement),
}

impl Title {
    /// The line as the content view takes it: a statement enters as its rendered sentence
    /// (AC-242).
    #[must_use]
    pub fn text(&self) -> String {
        match self {
            Self::Name(name) => name.clone(),
            Self::Statement(statement) => statement.render(),
        }
    }
}

/// A node with its ordered children. A node read without a key has none until the formatter
/// mints one (AC-209).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Node {
    pub key: Option<Key>,
    pub kind: NodeKind,
    pub title: Title,
    pub body: Option<String>,
    pub tags: Vec<String>,
    pub attributes: Attributes,
    pub children: Vec<Node>,
}

impl Node {
    /// A node with a name, no body, no tags, no attributes and no children.
    #[must_use]
    pub fn named(key: Option<Key>, kind: NodeKind, name: &str) -> Self {
        Self {
            key,
            kind,
            title: Title::Name(name.to_owned()),
            body: None,
            tags: Vec::new(),
            attributes: Attributes::default(),
            children: Vec::new(),
        }
    }

    /// This node and every node beneath it, depth first, parents before children.
    pub fn walk(&self) -> impl Iterator<Item = &Self> {
        let mut stack = vec![self];
        std::iter::from_fn(move || {
            let node = stack.pop()?;
            stack.extend(node.children.iter().rev());
            Some(node)
        })
    }
}

/// The type of an edge (D-11 F-13, F-14).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EdgeKind {
    Realizes,
    ScopedTo,
    Mentions,
    Subject,
}

impl EdgeKind {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Realizes => "realizes",
            Self::ScopedTo => "scoped_to",
            Self::Mentions => "mentions",
            Self::Subject => "subject",
        }
    }
}

/// The role a `subject` edge carries from a decision to a node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SubjectRole {
    Affected,
    Outcome,
}

impl SubjectRole {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Affected => "affected",
            Self::Outcome => "outcome",
        }
    }
}

/// A typed edge between two keyed elements, a node or a decision at either end.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Edge {
    pub source: Key,
    pub kind: EdgeKind,
    pub target: Key,
    pub role: Option<SubjectRole>,
}

/// An entry of the tag registry or of the constraint category registry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegistryEntry {
    pub key: Key,
    pub name: String,
    pub definition: String,
}

/// The v1 Authored State a converted document was derived from (RQ-20).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DerivedFrom {
    pub authored_state_id: Key,
    pub content_address: String,
    pub publish_sequence: u64,
}

/// A whole document. Products are in the manifest's order (AC-241); root Components, edges,
/// registry entries and decisions are in no meaningful order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Document {
    pub key: Key,
    pub products: Vec<Node>,
    pub components: Vec<Node>,
    pub edges: Vec<Edge>,
    pub tags: Vec<RegistryEntry>,
    pub categories: Vec<RegistryEntry>,
    pub decisions: Vec<crate::decision::Decision>,
    pub bindings: Bindings,
    pub derived_from: Option<DerivedFrom>,
}

impl Document {
    /// An empty document with the given document key.
    #[must_use]
    pub fn empty(key: Key) -> Self {
        Self {
            key,
            products: Vec::new(),
            components: Vec::new(),
            edges: Vec::new(),
            tags: Vec::new(),
            categories: Vec::new(),
            decisions: Vec::new(),
            bindings: Bindings::default(),
            derived_from: None,
        }
    }

    /// Every node of the document: the Products' trees in order, then the Components' trees.
    pub fn nodes(&self) -> impl Iterator<Item = &Node> {
        self.products
            .iter()
            .chain(&self.components)
            .flat_map(Node::walk)
    }
}
