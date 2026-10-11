//! The checked tree as a typed document: Products in the manifest's order (RQ-175), root
//! Components and decisions in file-name order (RQ-209), Features in their group's order
//! (RQ-28), and every reference typed by the element whose key it names.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;

use super::error::{Problem, ReadError, SpecPath};
use super::files::{CATEGORIES, LABELS, MANIFEST, Role, TAGS, Tree};
use super::record::{Rec, Val};
use super::schema::Kind;
use crate::canonical::Digest;
use crate::decision::NodeRef;
use crate::document::{CategoryEntry, DerivedFrom, Document, Edge, TagEntry};
use crate::identity::{Key, Label, LabelPrefix};
use crate::ids::{AuthoredStateId, CategoryId, DecisionId, DocumentId, ElementId, NodeId, TagId};
use crate::kind::NodeKind;
use crate::names::{CategoryName, TagName};
use crate::slug::Slug;
use crate::yaml::Position;

/// Resolves a strictly read tree into a document.
///
/// # Errors
///
/// The first reference that names no element of the kind it needs, or a binding the model
/// cannot hold.
pub fn resolve(tree: &Tree) -> Result<Document, ReadError> {
    let manifest_path = SpecPath::new(MANIFEST);
    let manifest = tree
        .record(&manifest_path)
        .expect("a read tree has a manifest");
    let mut resolver = Resolver {
        tree,
        index: index(tree),
        path: manifest_path,
        document: Document::empty(DocumentId::new(key(manifest, "documentKey"))),
    };
    resolver.manifest(manifest)?;
    resolver.registries();
    resolver.products(manifest)?;
    resolver.roots()?;
    resolver.decisions()?;
    resolver.labels()?;
    Ok(resolver.document)
}

pub(super) struct Resolver<'t> {
    pub(super) tree: &'t Tree,
    /// The element each key names.
    pub(super) index: BTreeMap<Key, ElementId>,
    /// The file being resolved, for errors.
    pub(super) path: SpecPath,
    pub(super) document: Document,
}

/// The element kind each keyed record is.
const fn node_kind(kind: Kind) -> Option<NodeKind> {
    Some(match kind {
        Kind::Product => NodeKind::Product,
        Kind::FeatureGroup => NodeKind::FeatureGroup,
        Kind::Feature => NodeKind::Feature,
        Kind::Requirement => NodeKind::Requirement,
        Kind::Criterion => NodeKind::Criterion,
        Kind::RootComponent | Kind::Component => NodeKind::Component,
        Kind::Constraint => NodeKind::Constraint,
        Kind::QualityAttribute => NodeKind::QualityAttribute,
        Kind::Section => NodeKind::Section,
        _ => return None,
    })
}

/// Every node and decision of the tree, by key.
fn index(tree: &Tree) -> BTreeMap<Key, ElementId> {
    let mut found = Vec::new();
    for (_, _, record) in tree.records(|_| true) {
        super::files::keyed(record, &mut found);
    }
    found
        .into_iter()
        .filter_map(|record| {
            let key = Key::parse(record.text("key")?).ok()?;
            let element = match (record.kind, node_kind(record.kind)) {
                (Kind::Decision, _) => ElementId::Decision(DecisionId::new(key)),
                (_, Some(kind)) => ElementId::Node(NodeId::new(kind, key)),
                _ => return None,
            };
            Some((key, element))
        })
        .collect()
}

/// A checked key field.
pub(super) fn key(record: &Rec, field: &str) -> Key {
    Key::parse(record.text(field).expect("a checked key")).expect("a checked key")
}

/// A checked text field, or empty.
pub(super) fn text(record: &Rec, field: &str) -> String {
    record.text(field).unwrap_or_default().to_owned()
}

/// A checked enumerated value as its model type.
pub(super) fn value<T: DeserializeOwned>(text: &str) -> T {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .expect("a value the schema checked")
}

pub(super) fn slug(record: &Rec) -> Slug {
    Slug::parse(record.text("slug").expect("a checked slug")).expect("a checked slug")
}

impl Resolver<'_> {
    pub(super) fn fail(&self, position: Option<Position>, problem: String) -> ReadError {
        ReadError::at(&self.path, position, Problem::Model(problem))
    }

    /// The element a key field names.
    pub(super) fn element(&self, value: &Val) -> Result<ElementId, ReadError> {
        let text = value.text().unwrap_or_default();
        Key::parse(text)
            .ok()
            .and_then(|key| self.index.get(&key).copied())
            .ok_or_else(|| {
                self.fail(
                    value.position(),
                    format!("no element carries the key {text}"),
                )
            })
    }

    /// The node a key field names, of the kind `want` says.
    pub(super) fn node<T>(
        &self,
        value: &Val,
        want: fn(NodeId) -> Option<T>,
    ) -> Result<T, ReadError> {
        match self.element(value)? {
            ElementId::Node(node) => want(node),
            ElementId::Decision(_) => None,
        }
        .ok_or_else(|| {
            self.fail(
                value.position(),
                "the key names an element of another kind".to_owned(),
            )
        })
    }

    /// A node reference: a held node's key, or a former label (AC-809).
    pub(super) fn node_ref(&self, value: &Val) -> Result<NodeRef, ReadError> {
        match value {
            Val::Record(former) => {
                let label = Label::parse(former.text("formerLabel").unwrap_or_default())
                    .map_err(|error| self.fail(Some(former.position), error.to_string()))?;
                Ok(NodeRef::Former(label))
            }
            _ => self.node(value, Some).map(NodeRef::Held),
        }
    }

    /// The `mentions` edges a record lists, from `source`.
    pub(super) fn mentions(&mut self, record: &Rec, source: ElementId) -> Result<(), ReadError> {
        for target in record.list("mentions") {
            let target = self.element(target)?;
            self.document.edges.push(Edge::Mentions { source, target });
        }
        Ok(())
    }

    fn manifest(&mut self, manifest: &Rec) -> Result<(), ReadError> {
        self.document.publish_branch = manifest.text("publishBranch").map(str::to_owned);
        if let Some(source) = manifest.record("derivedFrom") {
            let address = text(source, "contentAddress");
            self.document.derived_from = Some(DerivedFrom {
                authored_state_id: AuthoredStateId::new(key(source, "authoredStateId")),
                content_address: address
                    .parse::<Digest>()
                    .map_err(|error| self.fail(Some(source.position), error.to_string()))?,
                publish_sequence: text(source, "publishSequence").parse().unwrap_or(1),
            });
        }
        Ok(())
    }

    fn registries(&mut self) {
        for entry in self.tree.registry(TAGS) {
            self.document.tags.push(TagEntry {
                id: TagId::new(key(entry, "key")),
                name: TagName::parse(&text(entry, "name")).expect("a checked tag name"),
                definition: text(entry, "definition"),
            });
        }
        for entry in self.tree.registry(CATEGORIES) {
            self.document.categories.push(CategoryEntry {
                id: CategoryId::new(key(entry, "key")),
                name: CategoryName::parse(&text(entry, "name")).expect("a checked name"),
                definition: text(entry, "definition"),
            });
        }
    }

    fn products(&mut self, manifest: &Rec) -> Result<(), ReadError> {
        let files: BTreeMap<String, (SpecPath, &Rec)> = self
            .tree
            .records(|role| matches!(role, Role::Product { .. }))
            .map(|(path, _, record)| (text(record, "key"), (path.clone(), record)))
            .collect();
        for listed in manifest.list("products") {
            let (path, record) = &files[listed.text().unwrap_or_default()];
            self.path = path.clone();
            let product = self.product(record)?;
            self.document.products.push(product);
        }
        Ok(())
    }

    fn roots(&mut self) -> Result<(), ReadError> {
        let roots: Vec<(SpecPath, &Rec)> = self
            .tree
            .records(|role| matches!(role, Role::Component { .. }))
            .map(|(path, _, record)| (path.clone(), record))
            .collect();
        for (path, record) in roots {
            self.path = path;
            let root = self.root_component(record)?;
            self.document.components.push(root);
        }
        Ok(())
    }

    fn decisions(&mut self) -> Result<(), ReadError> {
        let decisions: Vec<(SpecPath, &Rec)> = self
            .tree
            .records(|role| matches!(role, Role::Decision { .. }))
            .map(|(path, _, record)| (path.clone(), record))
            .collect();
        for (path, record) in decisions {
            self.path = path;
            let decision = self.decision(record)?;
            self.document.decisions.push(decision);
        }
        Ok(())
    }

    fn labels(&mut self) -> Result<(), ReadError> {
        self.path = SpecPath::new(LABELS);
        let Some(labels) = self.tree.record(&self.path) else {
            return Ok(());
        };
        if let Some(counters) = labels.record("counters") {
            let values = counters.fields.iter().filter_map(|(prefix, value)| {
                Some((LabelPrefix::parse(prefix)?, value.text()?.parse().ok()?))
            });
            for (prefix, next) in values {
                self.document.counters.set(prefix, next);
            }
        }
        for binding in labels.list("bindings") {
            let Val::Record(binding) = binding else {
                continue;
            };
            let element = self.element(binding.get("key").expect("a checked key"))?;
            let (path, position) = (self.path.clone(), binding.position);
            let fail = |error: crate::bindings::BindingError| {
                ReadError::at(&path, Some(position), Problem::Model(error.to_string()))
            };
            let label = Label::parse(&text(binding, "label")).expect("a checked label");
            let mut bindings = std::mem::take(&mut self.document.bindings);
            bindings.bind(element, label).map_err(fail)?;
            for alias in binding.list("aliases") {
                let alias =
                    Label::parse(alias.text().unwrap_or_default()).expect("a checked label");
                bindings.add_alias(element, alias).map_err(fail)?;
            }
            self.document.bindings = bindings;
        }
        Ok(())
    }
}
