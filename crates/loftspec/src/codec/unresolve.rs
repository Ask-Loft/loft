//! A typed document as the record tree its files hold: each record at its path (RQ-21..RQ-25),
//! each edge in its source's record (RQ-168), and no derived value anywhere (RQ-29).

use std::collections::BTreeMap;

use serde::Serialize;

use super::error::SpecPath;
use super::files::{CATEGORIES, Content, LABELS, MANIFEST, Role, TAGS, Tree};
use super::record::{Rec, Val};
use super::schema::Kind;
use super::unresolve_parts::{decision_fields, node_ref, statement, target};
use crate::decision::Decision;
use crate::document::{Document, Edge};
use crate::identity::{Key, LabelPrefix};
use crate::ids::{ElementId, NodeId};
use crate::tree::{Component, Constraint, Details, Feature, Product, QualityAttribute, Section};
use crate::yaml::Position;

/// The record tree of a document.
#[must_use]
pub fn unresolve(document: &Document) -> Tree {
    let edges = Edges::of(document);
    let mut tree = Tree::default();
    put(
        &mut tree,
        MANIFEST,
        Role::Manifest,
        Content::Record(manifest(document)),
    );
    put(
        &mut tree,
        LABELS,
        Role::Labels,
        Content::Record(labels(document)),
    );
    let tags = document.tags.iter().map(|tag| {
        entry(
            Kind::TagEntry,
            tag.id.key(),
            tag.name.as_str(),
            &tag.definition,
        )
    });
    put(
        &mut tree,
        TAGS,
        Role::Tags,
        Content::Registry(tags.collect()),
    );
    let categories = document.categories.iter().map(|category| {
        let name = category.name.as_str();
        entry(
            Kind::CategoryEntry,
            category.id.key(),
            name,
            &category.definition,
        )
    });
    put(
        &mut tree,
        CATEGORIES,
        Role::Categories,
        Content::Registry(categories.collect()),
    );
    for product in &document.products {
        products(&mut tree, &edges, product);
    }
    for root in &document.components {
        let mut record = edges.component(&root.component);
        record.kind = Kind::RootComponent;
        record.set_text("slug", root.slug.to_string());
        let path = format!("components/{}.yaml", root.slug);
        let role = Role::Component {
            slug: root.slug.to_string(),
        };
        put(&mut tree, &path, role, Content::Record(record));
    }
    for decision in &document.decisions {
        let path = format!("decisions/{}.yaml", decision.slug);
        let role = Role::Decision {
            slug: decision.slug.to_string(),
        };
        put(
            &mut tree,
            &path,
            role,
            Content::Record(edges.decision(decision)),
        );
    }
    tree
}

fn put(tree: &mut Tree, path: &str, role: Role, content: Content) {
    tree.files.insert(SpecPath::new(path), (role, content));
}

/// A Product's file and each of its Features' files.
fn products(tree: &mut Tree, edges: &Edges, product: &Product) {
    let dir = product.slug.to_string();
    for group in &product.feature_groups {
        for feature in &group.features {
            let role = Role::Feature {
                product: dir.clone(),
                group: group.slug.to_string(),
                feature: feature.slug.to_string(),
            };
            let path = format!("products/{dir}/{}/{}.yaml", group.slug, feature.slug);
            put(tree, &path, role, Content::Record(edges.feature(feature)));
        }
    }
    let path = format!("products/{dir}/product.yaml");
    let role = Role::Product { product: dir };
    put(tree, &path, role, Content::Record(edges.product(product)));
}

/// A scalar value.
pub(super) fn scalar<T: std::fmt::Display + ?Sized>(text: &T) -> Val {
    Val::Scalar {
        text: text.to_string(),
        position: Position::new(1, 1),
    }
}

fn keys(items: impl IntoIterator<Item = Key>) -> Val {
    Val::List(items.into_iter().map(|key| scalar(&key)).collect())
}

fn records(items: impl IntoIterator<Item = Rec>) -> Val {
    Val::List(items.into_iter().map(Val::Record).collect())
}

/// An enumerated model value as the text its serde name gives.
fn name_of(value: &impl Serialize) -> String {
    match serde_json::to_value(value) {
        Ok(serde_json::Value::String(name)) => name,
        _ => unreachable!("an enumerated value serializes as a string"),
    }
}

fn entry(kind: Kind, key: Key, name: &str, definition: &str) -> Rec {
    let mut record = Rec::new(kind);
    record.set_text("key", key.to_string());
    record.set_text("name", name);
    record.set_text("definition", definition);
    record
}

fn manifest(document: &Document) -> Rec {
    let mut record = Rec::new(Kind::Manifest);
    record.set_text("format", crate::FORMAT);
    record.set_text("algorithm", crate::manifest::ALGORITHM);
    record.set_text("documentKey", document.id.key().to_string());
    if let Some(branch) = &document.publish_branch {
        record.set_text("publishBranch", branch.as_str());
    }
    let products = document.products.iter().filter_map(|product| product.id);
    record.set("products", keys(products.map(crate::ids::ProductId::key)));
    if let Some(source) = &document.derived_from {
        let mut derived = Rec::new(Kind::DerivedFrom);
        derived.set_text(
            "authoredStateId",
            source.authored_state_id.key().to_string(),
        );
        derived.set_text("contentAddress", source.content_address.to_string());
        derived.set_text("publishSequence", source.publish_sequence.to_string());
        record.set("derivedFrom", Val::Record(derived));
    }
    record
}

fn labels(document: &Document) -> Rec {
    let mut counters = Rec::new(Kind::Counters);
    for prefix in LabelPrefix::ALL {
        counters.set_text(prefix.as_str(), document.counters.next(prefix).to_string());
    }
    let bindings = document.bindings.iter().map(|binding| {
        let mut record = Rec::new(Kind::Binding);
        record.set_text("key", binding.element.key().to_string());
        record.set_text("label", binding.label.to_string());
        record.set(
            "aliases",
            Val::List(binding.aliases.iter().map(scalar).collect()),
        );
        record
    });
    let mut record = Rec::new(Kind::Labels);
    record.set("counters", Val::Record(counters));
    record.set("bindings", records(bindings));
    record
}

/// The document's edges, indexed by the record that writes them.
struct Edges {
    realizes: BTreeMap<NodeId, Vec<Key>>,
    scoped: BTreeMap<NodeId, Vec<Key>>,
    mentions: BTreeMap<ElementId, Vec<Key>>,
    subjects: BTreeMap<ElementId, Vec<Rec>>,
}

impl Edges {
    fn of(document: &Document) -> Self {
        let mut edges = Self {
            realizes: BTreeMap::new(),
            scoped: BTreeMap::new(),
            mentions: BTreeMap::new(),
            subjects: BTreeMap::new(),
        };
        for edge in &document.edges {
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
                    record.set_text("role", role.as_str());
                    record.set("node", node_ref(subject));
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
            record.set_text("key", id.key().to_string());
        }
        if let Some(name) = name {
            record.set_text("name", name);
        }
        self.details(&mut record, id, details);
        record
    }

    fn details(&self, record: &mut Rec, id: Option<NodeId>, details: &Details) {
        if let Some(body) = &details.body {
            record.set_text("body", body.as_str());
        }
        record.set("tags", Val::List(details.tags.iter().map(scalar).collect()));
        self.mentions(record, id.map(ElementId::Node));
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
        record.set("mentions", keys(targets));
    }

    fn section(&self, section: &Section) -> Rec {
        let id = section.id.map(NodeId::from);
        let mut record = Rec::new(Kind::Section);
        if let Some(id) = id {
            record.set_text("key", id.key().to_string());
        }
        record.set_text("name", section.name.as_str());
        record.set_text("body", section.body.as_str());
        self.mentions(&mut record, id.map(ElementId::Node));
        record.set(
            "sections",
            records(section.sections.iter().map(|child| self.section(child))),
        );
        record
    }

    fn product(&self, product: &Product) -> Rec {
        let mut record = self.node(
            Kind::Product,
            product.id.map(NodeId::from),
            Some(&product.name),
            &product.details,
        );
        record.set_text("slug", product.slug.to_string());
        let groups = product.feature_groups.iter().map(|group| {
            let id = group.id.map(NodeId::from);
            let mut record = self.node(Kind::FeatureGroup, id, Some(&group.name), &group.details);
            record.set_text("slug", group.slug.to_string());
            let features = group.features.iter().filter_map(|feature| feature.id);
            record.set("features", keys(features.map(crate::ids::FeatureId::key)));
            record.set(
                "constraints",
                records(group.constraints.iter().map(|each| self.constraint(each))),
            );
            record
        });
        record.set("featureGroups", records(groups));
        record
    }

    fn feature(&self, feature: &Feature) -> Rec {
        let id = feature.id.map(NodeId::from);
        let mut record = self.node(Kind::Feature, id, Some(&feature.name), &feature.details);
        record.set_text("slug", feature.slug.to_string());
        let realizes = id
            .and_then(|id| self.realizes.get(&id))
            .cloned()
            .unwrap_or_default();
        record.set("realizes", keys(realizes));
        let requirements = feature.requirements.iter().map(|requirement| {
            let id = requirement.id.map(NodeId::from);
            let mut record = self.node(Kind::Requirement, id, None, &requirement.details);
            record.set("statement", Val::Record(statement(&requirement.statement)));
            let criteria = requirement.criteria.iter().map(|criterion| {
                self.node(
                    Kind::Criterion,
                    criterion.id.map(NodeId::from),
                    Some(&criterion.name),
                    &criterion.details,
                )
            });
            record.set("criteria", records(criteria));
            record
        });
        record.set("requirements", records(requirements));
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

    fn component(&self, component: &Component) -> Rec {
        let id = component.id.map(NodeId::from);
        let mut record = self.node(
            Kind::Component,
            id,
            Some(&component.name),
            &component.details,
        );
        let scoped = id
            .and_then(|id| self.scoped.get(&id))
            .cloned()
            .unwrap_or_default();
        record.set("scopedTo", keys(scoped));
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

    fn constraint(&self, constraint: &Constraint) -> Rec {
        let id = constraint.id.map(NodeId::from);
        let mut record = self.node(
            Kind::Constraint,
            id,
            Some(&constraint.name),
            &constraint.details,
        );
        if let Some(binding) = &constraint.binding {
            let mut nested = Rec::new(Kind::ConstraintBinding);
            nested.set_text("category", binding.category.to_string());
            nested.set_text("requiredValue", binding.required_value.as_str());
            record.set("constraintBinding", Val::Record(nested));
        }
        if let Some(kind) = constraint.conservation_kind {
            record.set_text("conservationKind", name_of(&kind));
        }
        if let Some(fate) = constraint.verification_fate {
            record.set_text("verificationFate", name_of(&fate));
        }
        record
    }

    fn quality(&self, quality: &QualityAttribute) -> Rec {
        let id = quality.id.map(NodeId::from);
        let mut record = self.node(
            Kind::QualityAttribute,
            id,
            Some(&quality.name),
            &quality.details,
        );
        record.set_text("qaKind", name_of(&quality.kind));
        record.set("qualityTarget", Val::Record(target(&quality.target)));
        if let Some(fate) = quality.verification_fate {
            record.set_text("verificationFate", name_of(&fate));
        }
        record
    }

    fn decision(&self, decision: &Decision) -> Rec {
        let mut record = decision_fields(decision);
        let subjects = self
            .subjects
            .get(&decision.id.into())
            .cloned()
            .unwrap_or_default();
        record.set("subjects", records(subjects));
        self.mentions(&mut record, Some(decision.id.into()));
        record
    }
}
