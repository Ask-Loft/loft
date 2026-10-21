//! A typed document as the record tree its files hold: each record at its path (RQ-21..RQ-25),
//! each edge in its source's record (RQ-168), and no derived value anywhere (RQ-29).

use super::schema::field;
use crate::ext::Ext;
use std::collections::BTreeMap;

use super::files::{Content, Role, Tree};
use super::node_files::{Under, node_files};
use super::record::{Rec, Val};
use super::schema::Kind;
use super::unresolve_parts::{decision_fields, node_ref, statement, target};
use super::unresolve_singles::{category_record, labels, manifest, tag_record, term_record};
use crate::decision::Decision;
use crate::document::{Document, Edge};
use crate::identity::Key;
use crate::ids::{ElementId, NodeId};
use crate::node::Node;
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, Section,
};

/// The record tree of a document.
#[must_use]
pub fn unresolve(document: &Document) -> Tree {
    let mut tree = unresolve_elements(document);
    put(
        &mut tree,
        Role::Manifest,
        Content::Record(manifest(document)),
    );
    put(&mut tree, Role::Labels, Content::Record(labels(document)));
    let tags = document.tags.iter().map(tag_record).collect();
    put(&mut tree, Role::Tags, Content::Registry(tags));
    let categories = document.categories.iter().map(category_record).collect();
    put(&mut tree, Role::Categories, Content::Registry(categories));
    // The term list is written only when it holds an entry, so that a tree written before it
    // existed is written unchanged.
    if !document.terms.is_empty() {
        let terms = document.terms.iter().map(term_record).collect();
        put(&mut tree, Role::Terms, Content::Registry(terms));
    }
    super::resolve_conflicts::attach(&mut tree, &document.conflicts);
    tree
}

/// The files of a document's nodes and decisions alone, without their conflicts.
pub(crate) fn unresolve_elements(document: &Document) -> Tree {
    let edges = Edges::of(&document.edges);
    let mut tree = Tree::default();
    let mut file = |role: Role, node: Node<'_>| {
        let record = match (&role, node) {
            (Role::Component { slug }, Node::Component(component)) => {
                let mut record = edges.component(component);
                record.kind = Kind::RootComponent;
                record.set_text(field::SLUG, slug.as_str());
                record
            }
            (_, Node::Product(product)) => edges.product(product),
            (_, Node::Feature(feature)) => edges.feature(feature),
            _ => unreachable!("only a Product, a Feature and a root Component have a file"),
        };
        put(&mut tree, role, Content::Record(record));
    };
    for product in &document.products {
        node_files(Node::Product(product), None, &Under::Root, &mut file);
    }
    for root in &document.components {
        let node = Node::Component(&root.component);
        node_files(node, Some(&root.slug), &Under::Root, &mut file);
    }
    for decision in &document.decisions {
        let role = Role::Decision {
            slug: decision.slug.to_string(),
        };
        put(&mut tree, role, Content::Record(edges.decision(decision)));
    }
    tree
}

/// The record tree of a document's bindings file alone.
pub(crate) fn unresolve_labels(document: &Document) -> Tree {
    let mut tree = Tree::default();
    put(&mut tree, Role::Labels, Content::Record(labels(document)));
    super::resolve_conflicts::attach(&mut tree, &document.conflicts);
    tree
}

fn put(tree: &mut Tree, role: Role, content: Content) {
    tree.files.insert(role.path(), (role, content));
}

pub(super) fn keys(items: impl IntoIterator<Item = Key>) -> Val {
    Val::List(items.into_iter().map(|key| Val::scalar(&key)).collect())
}

pub(super) fn records(items: impl IntoIterator<Item = Rec>) -> Val {
    Val::List(items.into_iter().map(Val::Record).collect())
}

/// The document's edges, indexed by the record that writes them.
pub(super) struct Edges {
    realizes: BTreeMap<NodeId, Vec<Key>>,
    scoped: BTreeMap<NodeId, Vec<Key>>,
    mentions: BTreeMap<ElementId, Vec<Key>>,
    subjects: BTreeMap<ElementId, Vec<Rec>>,
}

impl Edges {
    pub(super) fn of<'e>(all: impl IntoIterator<Item = &'e Edge>) -> Self {
        let mut edges = Self {
            realizes: BTreeMap::new(),
            scoped: BTreeMap::new(),
            mentions: BTreeMap::new(),
            subjects: BTreeMap::new(),
        };
        for edge in all {
            match *edge {
                Edge::Realizes { feature, component } => {
                    edges
                        .realizes
                        .entry(feature.into())
                        .or_default()
                        .push(component.key());
                }
                Edge::ScopedTo { component, product } => {
                    edges
                        .scoped
                        .entry(component.into())
                        .or_default()
                        .push(product.key());
                }
                Edge::Mentions { source, target } => {
                    edges.mentions.entry(source).or_default().push(target.key());
                }
                Edge::Subject {
                    decision,
                    subject,
                    role,
                } => {
                    let mut record = Rec::new(Kind::Subject);
                    record.set_text(field::ROLE, role.as_str());
                    record.set(field::NODE, node_ref(subject));
                    edges
                        .subjects
                        .entry(decision.into())
                        .or_default()
                        .push(record);
                }
            }
        }
        edges
    }

    /// Key, then the fields every node record shares, as each kind orders them.
    fn node(&self, kind: Kind, id: Option<NodeId>, name: Option<&str>, details: &Details) -> Rec {
        let mut record = Rec::new(kind);
        if let Some(id) = id {
            record.set_text(field::KEY, id.key().to_string());
        }
        if let Some(name) = name {
            record.set_text(field::NAME, name);
        }
        self.details(&mut record, id, details);
        record
    }

    fn details(&self, record: &mut Rec, id: Option<NodeId>, details: &Details) {
        if let Some(body) = &details.body {
            record.set_text(field::BODY, body.as_str());
        }
        record.set(
            "tags",
            Val::List(details.tags.iter().map(Val::scalar).collect()),
        );
        self.mentions(record, id.map(ElementId::Node));
        set_ext(record, &details.ext);
        record.set(
            "sections",
            records(details.sections.iter().map(|section| self.section(section))),
        );
    }

    fn mentions(&self, record: &mut Rec, id: Option<ElementId>) {
        let targets = id
            .and_then(|id| self.mentions.get(&id))
            .cloned()
            .unwrap_or_default();
        record.set(field::MENTIONS, keys(targets));
    }

    pub(super) fn section(&self, section: &Section) -> Rec {
        let id = section.id.map(NodeId::from);
        let mut record = Rec::new(Kind::Section);
        if let Some(id) = id {
            record.set_text(field::KEY, id.key().to_string());
        }
        record.set_text(field::NAME, section.name.as_str());
        record.set_text(field::BODY, section.body.as_str());
        self.mentions(&mut record, id.map(ElementId::Node));
        set_ext(&mut record, &section.ext);
        record.set(
            "sections",
            records(section.sections.iter().map(|child| self.section(child))),
        );
        record
    }

    pub(super) fn product(&self, product: &Product) -> Rec {
        let mut record = self.node(
            Kind::Product,
            product.id.map(NodeId::from),
            Some(&product.name),
            &product.details,
        );
        record.set_text(field::SLUG, product.slug.to_string());
        let groups = product.feature_groups.iter().map(|group| self.group(group));
        record.set(field::FEATURE_GROUPS, records(groups));
        record.set(
            field::CONSTRAINTS,
            records(product.constraints.iter().map(|each| self.constraint(each))),
        );
        record
    }

    pub(super) fn group(&self, group: &FeatureGroup) -> Rec {
        let id = group.id.map(NodeId::from);
        let mut record = self.node(Kind::FeatureGroup, id, Some(&group.name), &group.details);
        record.set_text(field::SLUG, group.slug.to_string());
        let features = group.features.iter().filter_map(|feature| feature.id);
        let features = features.map(crate::ids::FeatureId::key);
        record.set(field::FEATURES, keys(features));
        let constraints = group.constraints.iter().map(|each| self.constraint(each));
        record.set(field::CONSTRAINTS, records(constraints));
        let qualities = group
            .quality_attributes
            .iter()
            .map(|each| self.quality(each));
        record.set(field::QUALITY_ATTRIBUTES, records(qualities));
        record
    }

    pub(super) fn feature(&self, feature: &Feature) -> Rec {
        let id = feature.id.map(NodeId::from);
        let mut record = self.node(Kind::Feature, id, Some(&feature.name), &feature.details);
        record.set_text(field::SLUG, feature.slug.to_string());
        let realizes = id
            .and_then(|id| self.realizes.get(&id))
            .cloned()
            .unwrap_or_default();
        record.set(field::REALIZES, keys(realizes));
        let requirements = feature.requirements.iter();
        let requirements = requirements.map(|requirement| self.requirement(requirement));
        record.set(field::REQUIREMENTS, records(requirements));
        record.set(
            "qualityAttributes",
            records(
                feature
                    .quality_attributes
                    .iter()
                    .map(|each| self.quality(each)),
            ),
        );
        record
    }

    pub(super) fn requirement(&self, requirement: &Requirement) -> Rec {
        let id = requirement.id.map(NodeId::from);
        let mut record = self.node(Kind::Requirement, id, None, &requirement.details);
        let written = Val::Record(statement(&requirement.statement));
        record.set(field::STATEMENT, written);
        let criteria = requirement.criteria.iter();
        record.set(
            field::CRITERIA,
            records(criteria.map(|each| self.criterion(each))),
        );
        record
    }

    pub(super) fn criterion(&self, criterion: &Criterion) -> Rec {
        let id = criterion.id.map(NodeId::from);
        self.node(
            Kind::Criterion,
            id,
            Some(&criterion.name),
            &criterion.details,
        )
    }

    pub(super) fn component(&self, component: &Component) -> Rec {
        let id = component.id.map(NodeId::from);
        let mut record = self.node(
            Kind::Component,
            id,
            Some(&component.name),
            &component.details,
        );
        if let Some(boundary) = component.boundary {
            record.set_text(field::BOUNDARY, boundary.as_str());
        }
        let scoped = id
            .and_then(|id| self.scoped.get(&id))
            .cloned()
            .unwrap_or_default();
        record.set(field::SCOPED_TO, keys(scoped));
        record.set(
            "components",
            records(
                component
                    .components
                    .iter()
                    .map(|child| self.component(child)),
            ),
        );
        record.set(
            "constraints",
            records(
                component
                    .constraints
                    .iter()
                    .map(|each| self.constraint(each)),
            ),
        );
        record.set(
            "qualityAttributes",
            records(
                component
                    .quality_attributes
                    .iter()
                    .map(|each| self.quality(each)),
            ),
        );
        record
    }

    pub(super) fn constraint(&self, constraint: &Constraint) -> Rec {
        let id = constraint.id.map(NodeId::from);
        let mut record = self.node(
            Kind::Constraint,
            id,
            Some(&constraint.name),
            &constraint.details,
        );
        if let Some(binding) = &constraint.binding {
            let mut nested = Rec::new(Kind::ConstraintBinding);
            nested.set_text(field::CATEGORY, binding.category.to_string());
            nested.set_text(field::REQUIRED_VALUE, binding.required_value.as_str());
            record.set(field::CONSTRAINT_BINDING, Val::Record(nested));
        }
        if let Some(kind) = constraint.conservation_kind {
            record.set_text(field::CONSERVATION_KIND, kind.as_str());
        }
        if let Some(fate) = constraint.verification_fate {
            record.set_text(field::VERIFICATION_FATE, fate.as_str());
        }
        record
    }

    pub(super) fn quality(&self, quality: &QualityAttribute) -> Rec {
        let id = quality.id.map(NodeId::from);
        let mut record = self.node(
            Kind::QualityAttribute,
            id,
            Some(&quality.name),
            &quality.details,
        );
        record.set_text(field::QA_KIND, quality.kind.as_str());
        record.set(field::QUALITY_TARGET, Val::Record(target(&quality.target)));
        if let Some(fate) = quality.verification_fate {
            record.set_text(field::VERIFICATION_FATE, fate.as_str());
        }
        record
    }

    pub(super) fn decision(&self, decision: &Decision) -> Rec {
        let mut record = decision_fields(decision);
        let subjects = self
            .subjects
            .get(&decision.id.into())
            .cloned()
            .unwrap_or_default();
        record.set(field::SUBJECTS, records(subjects));
        self.mentions(&mut record, Some(decision.id.into()));
        record
    }
}

/// Sets a record's `ext`, which an empty map leaves out.
fn set_ext(record: &mut Rec, ext: &Ext) {
    if !ext.is_empty() {
        record.set(
            field::EXT,
            Val::Ext {
                value: ext.clone(),
                position: record.position,
            },
        );
    }
}
