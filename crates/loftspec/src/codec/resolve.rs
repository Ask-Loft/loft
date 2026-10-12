//! The checked tree as a typed document: Products in the manifest's order (RQ-175), root
//! Components and decisions in file-name order (RQ-209), Features in their group's order
//! (RQ-28), and every reference typed by the element whose key it names.

use super::schema::field;
use std::collections::BTreeMap;

use super::error::{Problem, ReadError, SpecPath};
use super::files::{CATEGORIES, LABELS, MANIFEST, Role, TAGS, Tree};
use super::record::{Rec, Val};
use super::schema::Kind;
use crate::canonical::Digest;
use crate::decision::NodeRef;
use crate::document::{CategoryEntry, DerivedFrom, Document, Edge, TagEntry};
use crate::identity::{Key, Label, LabelPrefix};
use crate::ids::{AuthoredStateId, CategoryId, DecisionId, DocumentId, ElementId, NodeId, TagId};
use crate::names::{CategoryName, TagName};
use crate::slug::Slug;
use crate::yaml::Position;

/// Resolves a strictly read tree into a document.
///
/// # Errors
///
/// In each file, the first reference that names no element of the kind it needs; in the
/// bindings, each binding the model cannot hold.
pub fn resolve(tree: &Tree) -> Result<Document, Vec<ReadError>> {
    let manifest_path = SpecPath::new(MANIFEST);
    let manifest = tree
        .record(&manifest_path)
        .expect("a read tree has a manifest");
    let files = tree
        .records(|role| matches!(role, Role::Product { .. } | Role::Feature { .. }))
        .map(|(path, _, record)| (key(record, field::KEY), (path, record)))
        .collect();
    let mut resolver = Resolver {
        tree,
        index: index(tree),
        files,
        path: manifest_path,
        document: Document::empty(DocumentId::new(key(manifest, field::DOCUMENT_KEY))),
        errors: Vec::new(),
    };
    if let Err(error) = resolver.manifest(manifest) {
        resolver.errors.push(error);
    }
    resolver.registries();
    resolver.products(manifest);
    resolver.roots();
    resolver.decisions();
    resolver.labels();
    resolver.document.conflicts = super::resolve_conflicts::collect(tree);
    if resolver.errors.is_empty() {
        Ok(resolver.document)
    } else {
        Err(resolver.errors)
    }
}

pub(super) struct Resolver<'t> {
    pub(super) tree: &'t Tree,
    /// The element each key names.
    pub(super) index: BTreeMap<Key, ElementId>,
    /// The Product and Feature files, by the key their record carries.
    pub(super) files: BTreeMap<Key, (&'t SpecPath, &'t Rec)>,
    /// The file being resolved, for errors.
    pub(super) path: SpecPath,
    pub(super) document: Document,
    /// The first error of each file, and each refused binding.
    errors: Vec<ReadError>,
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
            let key = Key::parse(record.text(field::KEY)?).ok()?;
            let element = match (record.kind, record.kind.node_kind()) {
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

/// A checked enumerated field as its model value.
pub(super) fn checked<T>(parse: fn(&str) -> Option<T>, record: &Rec, field: &str) -> T {
    parse(record.text(field).unwrap_or_default()).expect("a value the schema checked")
}

/// An optional checked enumerated field as its model value.
pub(super) fn optional<T>(parse: fn(&str) -> Option<T>, record: &Rec, field: &str) -> Option<T> {
    record
        .text(field)
        .map(|text| parse(text).expect("a value the schema checked"))
}

pub(super) fn slug(record: &Rec) -> Slug {
    Slug::parse(record.text(field::SLUG).expect("a checked slug")).expect("a checked slug")
}

impl<'t> Resolver<'t> {
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
                let label = Label::parse(former.text(field::FORMER_LABEL).unwrap_or_default())
                    .map_err(|error| self.fail(Some(former.position), error.to_string()))?;
                Ok(NodeRef::Former(label))
            }
            _ => self.node(value, Some).map(NodeRef::Held),
        }
    }

    /// The `mentions` edges a record lists, from `source`.
    pub(super) fn mentions(&mut self, record: &Rec, source: ElementId) -> Result<(), ReadError> {
        for target in record.list(field::MENTIONS) {
            let target = self.element(target)?;
            self.document.edges.push(Edge::Mentions { source, target });
        }
        Ok(())
    }

    fn manifest(&mut self, manifest: &Rec) -> Result<(), ReadError> {
        self.document.publish_branch = manifest.text(field::PUBLISH_BRANCH).map(str::to_owned);
        if let Some(source) = manifest.record(field::DERIVED_FROM) {
            let address = text(source, field::CONTENT_ADDRESS);
            self.document.derived_from = Some(DerivedFrom {
                authored_state_id: AuthoredStateId::new(key(source, field::AUTHORED_STATE_ID)),
                content_address: address
                    .parse::<Digest>()
                    .map_err(|error| self.fail(Some(source.position), error.to_string()))?,
                publish_sequence: text(source, field::PUBLISH_SEQUENCE).parse().unwrap_or(1),
            });
        }
        Ok(())
    }

    fn registries(&mut self) {
        for entry in self.tree.registry(TAGS) {
            self.document.tags.push(TagEntry {
                id: TagId::new(key(entry, field::KEY)),
                name: TagName::parse(&text(entry, field::NAME)).expect("a checked tag name"),
                definition: text(entry, field::DEFINITION),
            });
        }
        for entry in self.tree.registry(CATEGORIES) {
            self.document.categories.push(CategoryEntry {
                id: CategoryId::new(key(entry, field::KEY)),
                name: CategoryName::parse(&text(entry, field::NAME)).expect("a checked name"),
                definition: text(entry, field::DEFINITION),
            });
        }
    }

    /// The file a listed key names; a read tree has one for every listed key (AC-99, AC-699).
    pub(super) fn listed_file(&mut self, listed: &Val) -> &'t Rec {
        let key = Key::parse(listed.text().unwrap_or_default()).expect("a checked key");
        let (path, record) = self.files[&key];
        self.path = path.clone();
        record
    }

    /// Keeps a resolved value, or records its error and goes on to the next file.
    fn keep<T>(&mut self, resolved: Result<T, ReadError>) -> Option<T> {
        resolved.map_err(|error| self.errors.push(error)).ok()
    }

    fn products(&mut self, manifest: &Rec) {
        for listed in manifest.list(field::PRODUCTS) {
            let record = self.listed_file(listed);
            let product = self.product(record);
            if let Some(product) = self.keep(product) {
                self.document.products.push(product);
            }
        }
    }

    fn roots(&mut self) {
        let tree = self.tree;
        for (path, _, record) in tree.records(|role| matches!(role, Role::Component { .. })) {
            self.path = path.clone();
            let root = self.root_component(record);
            if let Some(root) = self.keep(root) {
                self.document.components.push(root);
            }
        }
    }

    fn decisions(&mut self) {
        let tree = self.tree;
        for (path, _, record) in tree.records(|role| matches!(role, Role::Decision { .. })) {
            self.path = path.clone();
            let decision = self.decision(record);
            if let Some(decision) = self.keep(decision) {
                self.document.decisions.push(decision);
            }
        }
    }

    fn labels(&mut self) {
        self.path = SpecPath::new(LABELS);
        let Some(labels) = self.tree.record(&self.path) else {
            return;
        };
        if let Some(counters) = labels.record(field::COUNTERS) {
            let values = counters.fields.iter().filter_map(|(prefix, value)| {
                Some((LabelPrefix::parse(prefix)?, value.text()?.parse().ok()?))
            });
            for (prefix, next) in values {
                self.document.counters.set(prefix, next);
            }
        }
        for binding in labels.list(field::BINDINGS) {
            if let Val::Record(binding) = binding {
                let bound = self.binding(binding);
                self.keep(bound);
            }
        }
    }

    fn binding(&mut self, binding: &Rec) -> Result<(), ReadError> {
        let element = self.element(binding.get(field::KEY).expect("a checked key"))?;
        let fail = |error: crate::bindings::BindingError| {
            ReadError::at(
                &self.path,
                Some(binding.position),
                Problem::Model(error.to_string()),
            )
        };
        let label = Label::parse(&text(binding, field::LABEL)).expect("a checked label");
        let bindings = &mut self.document.bindings;
        bindings.bind_as_written(element, label).map_err(fail)?;
        for alias in binding.list(field::ALIASES) {
            let alias = Label::parse(alias.text().unwrap_or_default()).expect("a checked label");
            bindings.add_alias(element, alias).map_err(fail)?;
        }
        for withdrawn in binding.list(field::WITHDRAWN) {
            let withdrawn =
                Label::parse(withdrawn.text().unwrap_or_default()).expect("a checked label");
            bindings.add_withdrawn(element, withdrawn).map_err(fail)?;
        }
        Ok(())
    }
}
