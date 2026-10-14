//! The full document: the small document with every optional field of every record filled, for
//! the tests that read the order of a written record's fields.

use loftspec::attributes::{Boundary, ConservationKind};
use loftspec::canonical::Digest;
use loftspec::document::{DerivedFrom, Document, Edge};
use loftspec::ids::{
    AuthoredStateId, ComponentId, ConstraintId, DecisionId, ElementId, FeatureGroupId, FeatureId,
    ProductId, QualityAttributeId, RequirementId, SectionId,
};
use loftspec::names::TagName;
use loftspec::tree::{Details, Section};

use super::{component, key, label, small_document};

/// A Section numbered `n` holding one child Section numbered `n + 1`.
fn section(n: u32) -> Section {
    let child = Section {
        id: Some(SectionId::new(key(n + 1))),
        name: "Detail".to_owned(),
        body: "A nested Section.".to_owned(),
        sections: Vec::new(),
    };
    Section {
        id: Some(SectionId::new(key(n))),
        name: "Notes".to_owned(),
        body: "A Section.".to_owned(),
        sections: vec![child],
    }
}

/// A body, a tag and a Section numbered `n`.
fn details(n: u32) -> Details {
    Details {
        body: Some("A body.".to_owned()),
        tags: vec![TagName::parse("identity").unwrap()],
        sections: vec![section(n)],
    }
}

/// The small document's nodes, each with a body, a tag and a Section.
fn fill_products(document: &mut Document) {
    let product = &mut document.products[0];
    product.details = details(40);
    let mut constraint = document.components[0].constraints[0].clone();
    constraint.id = Some(ConstraintId::new(key(42)));
    constraint.details = details(43);
    constraint.conservation_kind = Some(ConservationKind::Cap);
    product.constraints = vec![constraint];
    let group = &mut product.feature_groups[0];
    group.details = details(45);
    let feature = &mut group.features[0];
    feature.details = details(47);
    feature.quality_attributes[0].details = details(49);
    let requirement = &mut feature.requirements[0];
    requirement.details = details(51);
    requirement.criteria[0].details = details(53);
}

fn fill_components(document: &mut Document) {
    let root = &mut document.components[0];
    root.details = details(55);
    root.boundary = Some(Boundary::Private);
    root.constraints[0].details = details(57);
    root.constraints[0].conservation_kind = Some(ConservationKind::Cap);
    root.quality_attributes[0].details = details(59);
    let mut nested = component(key(61), "Key parser");
    nested.details = details(62);
    root.components = vec![nested];
}

/// A `mentions` edge from every node kind and from the decision, each to the Requirement or,
/// from the Requirement, to the root Component.
fn mentions() -> Vec<Edge> {
    let requirement: ElementId = RequirementId::new(key(4)).into();
    let sources: [ElementId; 10] = [
        ProductId::new(key(1)).into(),
        FeatureGroupId::new(key(2)).into(),
        FeatureId::new(key(3)).into(),
        ConstraintId::new(key(42)).into(),
        QualityAttributeId::new(key(10)).into(),
        ComponentId::new(key(6)).into(),
        ComponentId::new(key(61)).into(),
        QualityAttributeId::new(key(8)).into(),
        SectionId::new(key(40)).into(),
        DecisionId::new(key(20)).into(),
    ];
    let mut edges: Vec<Edge> = sources
        .into_iter()
        .map(|source| Edge::Mentions {
            source,
            target: requirement,
        })
        .collect();
    edges.push(Edge::Mentions {
        source: RequirementId::new(key(4)).into(),
        target: ComponentId::new(key(6)).into(),
    });
    edges
}

/// The small document with every optional field of every record kind filled: each node has a
/// body, a tag, a Section with a child Section and a mention; the root Component holds a nested
/// Component and a boundary; a Constraint has a conservation kind; the decision mentions a node;
/// the manifest names a publish branch and the state it was derived from; and the Requirement's
/// binding holds a withdrawn label.
pub fn full_document() -> Document {
    let mut document = small_document();
    fill_products(&mut document);
    fill_components(&mut document);
    document.edges.extend(mentions());
    document.publish_branch = Some("release".to_owned());
    document.derived_from = Some(DerivedFrom {
        authored_state_id: AuthoredStateId::new(key(70)),
        content_address: "ab".repeat(32).parse::<Digest>().unwrap(),
        publish_sequence: 3,
    });
    document
        .bindings
        .add_withdrawn(RequirementId::new(key(4)), label("RQ-7"))
        .unwrap();
    document
}

/// The full document's files.
pub fn full_files() -> loftspec::codec::Files {
    loftspec::codec::write(&full_document())
}

/// The full document with its nested Component filled as well: a boundary, a Product it is
/// scoped to, a Component of its own (key 66), a Constraint (key 64) and a Quality Attribute
/// (key 65).
pub fn full_nested_document() -> Document {
    let mut document = full_document();
    let root = &mut document.components[0].component;
    let mut constraint = root.constraints[0].clone();
    constraint.id = Some(ConstraintId::new(key(64)));
    constraint.details = Details::default();
    let mut quality = root.quality_attributes[0].clone();
    quality.id = Some(QualityAttributeId::new(key(65)));
    quality.details = Details::default();
    let nested = &mut root.components[0];
    nested.boundary = Some(Boundary::External);
    nested.components = vec![component(key(66), "Key reader")];
    nested.constraints = vec![constraint];
    nested.quality_attributes = vec![quality];
    document.edges.push(Edge::ScopedTo {
        component: ComponentId::new(key(61)),
        product: ProductId::new(key(1)),
    });
    document
}
