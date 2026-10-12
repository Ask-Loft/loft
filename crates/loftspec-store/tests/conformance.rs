//! Document stores: one scenario run against the memory store and the file store. After each
//! step the two report the same roots and the same files written and removed, the file store's
//! tree is the writer's output for the document, it loads back the document saved in the
//! writer's order, and the document is valid.

mod common;
mod fixtures;

use common::Scratch;
use fixtures::{LINT_CONFIG, converted, with_lint};
use loftspec::codec::{self, LINT, SpecPath};
use loftspec::document::{Document, Edge, TagEntry};
use loftspec::identity::Key;
use loftspec::ids::{
    ComponentId, CriterionId, DocumentId, FeatureGroupId, FeatureId, ProductId, RequirementId,
    TagId,
};
use loftspec::names::TagName;
use loftspec::slug::Slug;
use loftspec::statement::{EarsForm, Statement};
use loftspec::tree::{
    Component, Criterion, Details, Feature, FeatureGroup, Product, Requirement, RootComponent,
};
use loftspec::validate::validate;
use loftspec_store::{DocumentStore, FileStore, MemoryStore, Saved};

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");
const D11: &str = include_str!("../../../testdata/v1/d11-sequence-3/loft-authored-state.json");

/// A memory store and a file store saved in step.
struct Pair {
    scratch: Scratch,
    memory: MemoryStore,
    file: FileStore,
}

impl Pair {
    fn new() -> Self {
        let scratch = with_lint();
        let file = FileStore::open(&scratch.0).unwrap();
        Self {
            scratch,
            memory: MemoryStore::new(),
            file,
        }
    }

    /// Saves the document to both stores and checks that they agree.
    fn step(&mut self, document: &Document) -> Saved {
        let saved = self.memory.save(document).unwrap();
        assert_eq!(self.file.save(document).unwrap(), saved, "files saved");
        assert_eq!(
            self.file.roots().unwrap(),
            self.memory.roots().unwrap(),
            "roots"
        );
        let mut tree = self.file.read_files().unwrap();
        assert_eq!(
            tree.remove(&SpecPath::new(LINT)).as_deref(),
            Some(LINT_CONFIG.as_bytes()),
            "lint.yaml"
        );
        assert_eq!(tree, codec::write(&self.memory.load().unwrap()), "tree");
        // The writer puts registries and edges in canonical order, so the document comes back
        // in that order.
        let loaded = self.file.load().unwrap();
        let canonical = codec::read(&codec::write(document)).unwrap();
        assert_eq!(loaded, canonical, "loaded back");
        assert_eq!(validate(&loaded), Vec::new(), "valid");
        assert!(
            self.scratch
                .snapshot()
                .keys()
                .all(|path| !path.starts_with(".loft"))
        );
        saved
    }
}

fn key(n: u32) -> Key {
    Key::parse(&format!("00000000-0000-7000-8000-{n:012x}")).unwrap()
}

fn paths(paths: &[&str]) -> Vec<SpecPath> {
    paths.iter().map(|path| SpecPath::new(path)).collect()
}

fn feature(n: u32, name: &str) -> Feature {
    Feature {
        id: Some(FeatureId::new(key(n))),
        slug: Slug::derive(name, "feature"),
        name: name.to_owned(),
        details: Details::default(),
        requirements: Vec::new(),
        quality_attributes: Vec::new(),
    }
}

fn group(n: u32, name: &str, features: Vec<Feature>) -> FeatureGroup {
    FeatureGroup {
        id: Some(FeatureGroupId::new(key(n))),
        slug: Slug::derive(name, "group"),
        name: name.to_owned(),
        details: Details::default(),
        features,
        constraints: Vec::new(),
    }
}

fn criterion(n: u32, name: &str) -> Criterion {
    Criterion {
        id: Some(CriterionId::new(key(n))),
        name: name.to_owned(),
        details: Details::default(),
    }
}

/// Step 1: a Product with two Feature Groups, each holding one Feature.
fn created() -> Document {
    let mut document = Document::empty(DocumentId::new(key(0)));
    document.products.push(Product {
        id: Some(ProductId::new(key(1))),
        slug: Slug::derive("Shelf", "product"),
        name: "Shelf".to_owned(),
        details: Details::with_body("Books on a shelf."),
        feature_groups: vec![
            group(2, "Lending", vec![feature(3, "Borrowing")]),
            group(4, "Keeping", vec![feature(5, "Returning")]),
        ],
    });
    document
}

fn borrowing(document: &mut Document) -> &mut Feature {
    &mut document.products[0].feature_groups[0].features[0]
}

/// Step 2: a Requirement with two criteria under "Borrowing".
fn with_requirement() -> Document {
    let mut document = created();
    borrowing(&mut document).requirements.push(Requirement {
        id: Some(RequirementId::new(key(10))),
        statement: Statement::Form(EarsForm::EventDriven {
            when: "a reader borrows a book".to_owned(),
            system: "The shelf".to_owned(),
            shall: "record the loan.".to_owned(),
        }),
        details: Details::default(),
        criteria: vec![
            criterion(11, "The loan names the reader."),
            criterion(12, "The loan names the book."),
        ],
    });
    document
}

/// Step 3: a registered tag applied to the Feature and its Requirement.
fn tagged() -> Document {
    let mut document = with_requirement();
    let tag = TagName::parse("loans").unwrap();
    document.tags.push(TagEntry {
        id: TagId::new(key(20)),
        name: tag.clone(),
        definition: "Behaviour about lending books.".to_owned(),
    });
    let feature = borrowing(&mut document);
    feature.details.tags.push(tag.clone());
    feature.requirements[0].details.tags.push(tag);
    document
}

/// Step 4: a root Component the Feature realizes, scoped to the Product, and a criterion that
/// mentions its Requirement.
fn linked() -> Document {
    let mut document = tagged();
    document.components.push(RootComponent {
        slug: Slug::derive("Ledger", "component"),
        component: Component {
            id: Some(ComponentId::new(key(30))),
            name: "Ledger".to_owned(),
            details: Details::default(),
            boundary: None,
            components: Vec::new(),
            constraints: Vec::new(),
            quality_attributes: Vec::new(),
        },
    });
    document.edges = vec![
        Edge::Realizes {
            feature: FeatureId::new(key(3)),
            component: ComponentId::new(key(30)),
        },
        Edge::ScopedTo {
            component: ComponentId::new(key(30)),
            product: ProductId::new(key(1)),
        },
        Edge::Mentions {
            source: CriterionId::new(key(11)).into(),
            target: RequirementId::new(key(10)).into(),
        },
    ];
    document
}

/// Step 5: the Requirement moves from "Borrowing" to "Returning".
fn requirement_moved() -> Document {
    let mut document = linked();
    let requirement = borrowing(&mut document).requirements.remove(0);
    document.products[0].feature_groups[1].features[0]
        .requirements
        .push(requirement);
    document
}

/// Step 6: "Borrowing" moves from "Lending" to "Keeping", leaving "Lending" empty.
fn feature_moved() -> Document {
    let mut document = requirement_moved();
    let groups = &mut document.products[0].feature_groups;
    let feature = groups[0].features.remove(0);
    groups[1].features.push(feature);
    document
}

/// Step 7: "Borrowing" is deleted, and the edge it owns with it.
fn feature_deleted() -> Document {
    let mut document = feature_moved();
    document.products[0].feature_groups[1].features.remove(1);
    document
        .edges
        .retain(|edge| !matches!(edge, Edge::Realizes { .. }));
    document
}

/// Saves each document in turn to a fresh pair, and returns what the last save did.
fn walk(steps: &[fn() -> Document]) -> Saved {
    let mut pair = Pair::new();
    let mut last = Saved::default();
    for step in steps {
        last = pair.step(&step());
    }
    last
}

#[test]
fn creating_a_product_with_its_groups_and_features_agrees() {
    let saved = walk(&[created]);
    assert_eq!(
        saved.written,
        paths(&[
            "categories.yaml",
            "labels.yaml",
            "loftspec.yaml",
            "products/shelf/keeping/returning.yaml",
            "products/shelf/lending/borrowing.yaml",
            "products/shelf/product.yaml",
            "tags.yaml",
        ])
    );
    assert_eq!(saved.removed, Vec::new());
}

#[test]
fn adding_a_requirement_with_criteria_writes_only_its_feature() {
    let saved = walk(&[created, with_requirement]);
    assert_eq!(
        saved.written,
        paths(&["products/shelf/lending/borrowing.yaml"])
    );
}

#[test]
fn tagging_writes_the_registry_and_the_tagged_feature() {
    let saved = walk(&[created, with_requirement, tagged]);
    assert_eq!(
        saved.written,
        paths(&["products/shelf/lending/borrowing.yaml", "tags.yaml"])
    );
}

#[test]
fn linking_writes_the_component_and_the_linking_feature() {
    let saved = walk(&[created, with_requirement, tagged, linked]);
    assert_eq!(
        saved.written,
        paths(&[
            "components/ledger.yaml",
            "products/shelf/lending/borrowing.yaml",
        ])
    );
}

#[test]
fn moving_a_requirement_writes_the_two_feature_files() {
    let saved = walk(&[created, with_requirement, tagged, linked, requirement_moved]);
    assert_eq!(
        saved.written,
        paths(&[
            "products/shelf/keeping/returning.yaml",
            "products/shelf/lending/borrowing.yaml",
        ])
    );
    assert_eq!(saved.removed, Vec::new());
}

#[test]
fn moving_a_feature_removes_its_former_file() {
    let steps = [
        created,
        with_requirement,
        tagged,
        linked,
        requirement_moved,
        feature_moved,
    ];
    let saved = walk(&steps);
    assert_eq!(
        saved.written,
        paths(&[
            "products/shelf/keeping/borrowing.yaml",
            "products/shelf/product.yaml",
        ])
    );
    assert_eq!(
        saved.removed,
        paths(&["products/shelf/lending/borrowing.yaml"])
    );
}

#[test]
fn a_directory_the_save_leaves_empty_is_removed() {
    let mut pair = Pair::new();
    for step in [created, feature_moved] {
        pair.step(&step());
    }
    assert!(!pair.scratch.0.join("spec/products/shelf/lending").exists());
    assert!(pair.scratch.0.join("spec/products/shelf/keeping").is_dir());
}

#[test]
fn deleting_a_feature_removes_its_file() {
    let steps = [
        created,
        with_requirement,
        tagged,
        linked,
        requirement_moved,
        feature_moved,
        feature_deleted,
    ];
    let saved = walk(&steps);
    assert_eq!(saved.written, paths(&["products/shelf/product.yaml"]));
    assert_eq!(
        saved.removed,
        paths(&["products/shelf/keeping/borrowing.yaml"])
    );
}

#[test]
fn saving_an_unchanged_document_writes_and_removes_nothing() {
    let saved = walk(&[linked, linked]);
    assert_eq!(saved, Saved::default());
}

#[test]
fn converted_v1_exports_agree() {
    for export in [READING_LIST, D11] {
        let document = converted(export);
        let mut pair = Pair::new();
        assert_ne!(pair.step(&document).written, Vec::new());
        assert_eq!(pair.step(&document), Saved::default());
    }
}
