//! Node records as typed nodes, with the edges they carry (RQ-168).

use super::error::ReadError;
use super::record::Rec;
use super::resolve::{Resolver, checked, key, optional, slug, text};
use crate::attributes::{
    BandKind, ConservationKind, QaKind, QualityAggregation, QualityBand, QualityTarget,
    TechnicalBinding, VerificationFate,
};
use crate::document::Edge;
use crate::ids::{
    ComponentId, ConstraintId, CriterionId, ElementId, FeatureGroupId, FeatureId, NodeId,
    ProductId, QualityAttributeId, RequirementId, SectionId,
};
use crate::names::{CategoryName, TagName};
use crate::statement::{EarsForm, Form, Statement};
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent, Section,
};

impl Resolver<'_> {
    /// A node's body, tags and Sections, and its `mentions` edges.
    fn details(&mut self, record: &Rec, id: NodeId) -> Result<Details, ReadError> {
        self.mentions(record, ElementId::Node(id))?;
        let tags = record
            .list("tags")
            .iter()
            .map(|tag| TagName::parse(tag.text().unwrap_or_default()).expect("a checked tag"))
            .collect();
        Ok(Details {
            body: record.text("body").map(str::to_owned),
            tags,
            sections: self.sections(record)?,
        })
    }

    fn sections(&mut self, record: &Rec) -> Result<Vec<Section>, ReadError> {
        record
            .records("sections")
            .map(|section| {
                let id = SectionId::new(key(section, "key"));
                self.mentions(section, ElementId::Node(id.into()))?;
                Ok(Section {
                    id: Some(id),
                    name: text(section, "name"),
                    body: text(section, "body"),
                    sections: self.sections(section)?,
                })
            })
            .collect()
    }

    pub(super) fn product(&mut self, record: &Rec) -> Result<Product, ReadError> {
        let id = ProductId::new(key(record, "key"));
        Ok(Product {
            id: Some(id),
            slug: slug(record),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            feature_groups: record
                .records("featureGroups")
                .map(|group| self.feature_group(group))
                .collect::<Result<_, _>>()?,
        })
    }

    fn feature_group(&mut self, record: &Rec) -> Result<FeatureGroup, ReadError> {
        let id = FeatureGroupId::new(key(record, "key"));
        let product_path = self.path.clone();
        let mut features = Vec::new();
        for listed in record.list("features") {
            let feature = self.listed_file(listed);
            features.push(self.feature(feature)?);
        }
        self.path = product_path;
        Ok(FeatureGroup {
            id: Some(id),
            slug: slug(record),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            features,
            constraints: record
                .records("constraints")
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
        })
    }

    fn feature(&mut self, record: &Rec) -> Result<Feature, ReadError> {
        let id = FeatureId::new(key(record, "key"));
        for target in record.list("realizes") {
            let component = self.node(target, |node| match node {
                NodeId::Component(component) => Some(component),
                _ => None,
            })?;
            self.document.edges.push(Edge::Realizes {
                feature: id,
                component,
            });
        }
        Ok(Feature {
            id: Some(id),
            slug: slug(record),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            requirements: record
                .records("requirements")
                .map(|requirement| self.requirement(requirement))
                .collect::<Result<_, _>>()?,
            quality_attributes: record
                .records("qualityAttributes")
                .map(|quality| self.quality_attribute(quality))
                .collect::<Result<_, _>>()?,
        })
    }

    fn requirement(&mut self, record: &Rec) -> Result<Requirement, ReadError> {
        let id = RequirementId::new(key(record, "key"));
        let statement = record.record("statement").expect("a checked statement");
        Ok(Requirement {
            id: Some(id),
            statement: statement_of(statement),
            details: self.details(record, id.into())?,
            criteria: record
                .records("criteria")
                .map(|criterion| {
                    let id = CriterionId::new(key(criterion, "key"));
                    Ok(Criterion {
                        id: Some(id),
                        name: text(criterion, "name"),
                        details: self.details(criterion, id.into())?,
                    })
                })
                .collect::<Result<_, ReadError>>()?,
        })
    }

    pub(super) fn root_component(&mut self, record: &Rec) -> Result<RootComponent, ReadError> {
        Ok(RootComponent {
            slug: slug(record),
            component: self.component(record)?,
        })
    }

    fn component(&mut self, record: &Rec) -> Result<Component, ReadError> {
        let id = ComponentId::new(key(record, "key"));
        for target in record.list("scopedTo") {
            let product = self.node(target, |node| match node {
                NodeId::Product(product) => Some(product),
                _ => None,
            })?;
            self.document.edges.push(Edge::ScopedTo {
                component: id,
                product,
            });
        }
        Ok(Component {
            id: Some(id),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            components: record
                .records("components")
                .map(|component| self.component(component))
                .collect::<Result<_, _>>()?,
            constraints: record
                .records("constraints")
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
            quality_attributes: record
                .records("qualityAttributes")
                .map(|quality| self.quality_attribute(quality))
                .collect::<Result<_, _>>()?,
        })
    }

    fn constraint(&mut self, record: &Rec) -> Result<Constraint, ReadError> {
        let id = ConstraintId::new(key(record, "key"));
        Ok(Constraint {
            id: Some(id),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            binding: record
                .record("constraintBinding")
                .map(|binding| TechnicalBinding {
                    category: CategoryName::parse(&text(binding, "category"))
                        .expect("a checked category"),
                    required_value: text(binding, "requiredValue"),
                }),
            conservation_kind: optional(ConservationKind::parse, record, "conservationKind"),
            verification_fate: optional(VerificationFate::parse, record, "verificationFate"),
        })
    }

    fn quality_attribute(&mut self, record: &Rec) -> Result<QualityAttribute, ReadError> {
        let id = QualityAttributeId::new(key(record, "key"));
        let target = record.record("qualityTarget").expect("a checked target");
        Ok(QualityAttribute {
            id: Some(id),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            kind: checked(QaKind::parse, record, "qaKind"),
            target: QualityTarget {
                metric: text(target, "metric"),
                band: band_of(target.record("band").expect("a checked band")),
                unit: text(target, "unit"),
                aggregation: checked(QualityAggregation::parse, target, "aggregation"),
                scenario: text(target, "scenario"),
                environment: text(target, "environment"),
            },
            verification_fate: optional(VerificationFate::parse, record, "verificationFate"),
        })
    }
}

fn band_of(band: &Rec) -> QualityBand {
    let bound = |field: &str| text(band, field);
    match checked(BandKind::parse, band, "kind") {
        BandKind::AtMost => QualityBand::AtMost {
            upper: bound("upper"),
        },
        BandKind::AtLeast => QualityBand::AtLeast {
            lower: bound("lower"),
        },
        BandKind::Exactly => QualityBand::Exactly {
            value: bound("value"),
        },
        BandKind::Between => QualityBand::Between {
            lower: bound("lower"),
            upper: bound("upper"),
        },
    }
}

/// A statement record as text or as a form with its slots (AC-751, AC-752).
fn statement_of(record: &Rec) -> Statement {
    if let Some(text) = record.text("text") {
        return Statement::Text(text.to_owned());
    }
    let form = checked(Form::parse, record, "form");
    let condition = form
        .condition()
        .map(|slot| text(record, slot))
        .unwrap_or_default();
    let (system, shall) = (text(record, "system"), text(record, "shall"));
    Statement::Form(EarsForm::from_slots(form, condition, system, shall))
}
