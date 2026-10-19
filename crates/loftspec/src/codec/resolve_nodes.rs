//! Node records as typed nodes, with the edges they carry (RQ-168).

use super::error::ReadError;
use super::record::{Rec, Val};
use super::resolve::{Resolver, checked, key, optional, slug, text};
use super::schema::field;
use crate::attributes::{
    BandKind, Boundary, ConservationKind, QaKind, QualityAggregation, QualityBand, QualityTarget,
    TechnicalBinding, VerificationFate,
};
use crate::document::Edge;
use crate::ext::Ext;
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

/// A record's extension data; empty when it has no `ext`.
fn ext(record: &Rec) -> Ext {
    match record.get(field::EXT) {
        Some(Val::Ext { value, .. }) => value.clone(),
        _ => Ext::new(),
    }
}

impl Resolver<'_> {
    /// A node's body, tags and Sections, and its `mentions` edges.
    fn details(&mut self, record: &Rec, id: NodeId) -> Result<Details, ReadError> {
        self.mentions(record, ElementId::Node(id))?;
        let tags = record
            .list(field::TAGS)
            .iter()
            .map(|tag| TagName::parse(tag.text().unwrap_or_default()).expect("a checked tag"))
            .collect();
        Ok(Details {
            body: record.text(field::BODY).map(str::to_owned),
            tags,
            sections: self.sections(record)?,
            ext: ext(record),
        })
    }

    fn sections(&mut self, record: &Rec) -> Result<Vec<Section>, ReadError> {
        record
            .records(field::SECTIONS)
            .map(|section| {
                let id = SectionId::new(key(section, field::KEY));
                self.mentions(section, ElementId::Node(id.into()))?;
                Ok(Section {
                    id: Some(id),
                    name: text(section, field::NAME),
                    body: text(section, field::BODY),
                    sections: self.sections(section)?,
                    ext: ext(section),
                })
            })
            .collect()
    }

    pub(super) fn product(&mut self, record: &Rec) -> Result<Product, ReadError> {
        let id = ProductId::new(key(record, field::KEY));
        Ok(Product {
            id: Some(id),
            slug: slug(record),
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            feature_groups: record
                .records(field::FEATURE_GROUPS)
                .map(|group| self.feature_group(group))
                .collect::<Result<_, _>>()?,
            constraints: record
                .records(field::CONSTRAINTS)
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
        })
    }

    fn feature_group(&mut self, record: &Rec) -> Result<FeatureGroup, ReadError> {
        let id = FeatureGroupId::new(key(record, field::KEY));
        let product_path = self.path.clone();
        let mut features = Vec::new();
        for listed in record.list(field::FEATURES) {
            let feature = self.listed_file(listed);
            features.push(self.feature(feature)?);
        }
        self.path = product_path;
        Ok(FeatureGroup {
            id: Some(id),
            slug: slug(record),
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            features,
            constraints: record
                .records(field::CONSTRAINTS)
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
            quality_attributes: record
                .records(field::QUALITY_ATTRIBUTES)
                .map(|quality| self.quality_attribute(quality))
                .collect::<Result<_, _>>()?,
        })
    }

    fn feature(&mut self, record: &Rec) -> Result<Feature, ReadError> {
        let id = FeatureId::new(key(record, field::KEY));
        for target in record.list(field::REALIZES) {
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
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            requirements: record
                .records(field::REQUIREMENTS)
                .map(|requirement| self.requirement(requirement))
                .collect::<Result<_, _>>()?,
            quality_attributes: record
                .records(field::QUALITY_ATTRIBUTES)
                .map(|quality| self.quality_attribute(quality))
                .collect::<Result<_, _>>()?,
        })
    }

    fn requirement(&mut self, record: &Rec) -> Result<Requirement, ReadError> {
        let id = RequirementId::new(key(record, field::KEY));
        let statement = record
            .record(field::STATEMENT)
            .expect("a checked statement");
        Ok(Requirement {
            id: Some(id),
            statement: statement_of(statement),
            details: self.details(record, id.into())?,
            criteria: record
                .records(field::CRITERIA)
                .map(|criterion| {
                    let id = CriterionId::new(key(criterion, field::KEY));
                    Ok(Criterion {
                        id: Some(id),
                        name: text(criterion, field::NAME),
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
        let id = ComponentId::new(key(record, field::KEY));
        for target in record.list(field::SCOPED_TO) {
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
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            boundary: optional(Boundary::parse, record, field::BOUNDARY),
            components: record
                .records(field::COMPONENTS)
                .map(|component| self.component(component))
                .collect::<Result<_, _>>()?,
            constraints: record
                .records(field::CONSTRAINTS)
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
            quality_attributes: record
                .records(field::QUALITY_ATTRIBUTES)
                .map(|quality| self.quality_attribute(quality))
                .collect::<Result<_, _>>()?,
        })
    }

    fn constraint(&mut self, record: &Rec) -> Result<Constraint, ReadError> {
        let id = ConstraintId::new(key(record, field::KEY));
        Ok(Constraint {
            id: Some(id),
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            binding: record
                .record(field::CONSTRAINT_BINDING)
                .map(|binding| TechnicalBinding {
                    category: CategoryName::parse(&text(binding, field::CATEGORY))
                        .expect("a checked category"),
                    required_value: text(binding, field::REQUIRED_VALUE),
                }),
            conservation_kind: optional(ConservationKind::parse, record, field::CONSERVATION_KIND),
            verification_fate: optional(VerificationFate::parse, record, field::VERIFICATION_FATE),
        })
    }

    fn quality_attribute(&mut self, record: &Rec) -> Result<QualityAttribute, ReadError> {
        let id = QualityAttributeId::new(key(record, field::KEY));
        let target = record
            .record(field::QUALITY_TARGET)
            .expect("a checked target");
        Ok(QualityAttribute {
            id: Some(id),
            name: text(record, field::NAME),
            details: self.details(record, id.into())?,
            kind: checked(QaKind::parse, record, field::QA_KIND),
            target: QualityTarget {
                metric: text(target, field::METRIC),
                band: band_of(target.record(field::BAND).expect("a checked band")),
                unit: text(target, field::UNIT),
                aggregation: checked(QualityAggregation::parse, target, field::AGGREGATION),
                scenario: text(target, field::SCENARIO),
                environment: text(target, field::ENVIRONMENT),
            },
            verification_fate: optional(VerificationFate::parse, record, field::VERIFICATION_FATE),
        })
    }
}

fn band_of(band: &Rec) -> QualityBand {
    let bound = |field: &str| text(band, field);
    match checked(BandKind::parse, band, field::KIND) {
        BandKind::AtMost => QualityBand::AtMost {
            upper: bound(field::UPPER),
        },
        BandKind::AtLeast => QualityBand::AtLeast {
            lower: bound(field::LOWER),
        },
        BandKind::Exactly => QualityBand::Exactly {
            value: bound(field::VALUE),
        },
        BandKind::Between => QualityBand::Between {
            lower: bound(field::LOWER),
            upper: bound(field::UPPER),
        },
    }
}

/// A statement record as text or as a form with its slots (AC-751, AC-752).
pub(crate) fn statement_of(record: &Rec) -> Statement {
    if let Some(text) = record.text(field::TEXT) {
        return Statement::Text(text.to_owned());
    }
    let form = checked(Form::parse, record, field::FORM);
    let condition = form
        .condition()
        .map(|slot| text(record, slot))
        .unwrap_or_default();
    let (system, shall) = (text(record, field::SYSTEM), text(record, field::SHALL));
    Statement::Form(EarsForm::from_slots(form, condition, system, shall))
}
