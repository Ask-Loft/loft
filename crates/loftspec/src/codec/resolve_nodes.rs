//! Node records as typed nodes, with the edges they carry (RQ-168).

use std::collections::BTreeMap;

use super::error::{ReadError, SpecPath};
use super::files::Role;
use super::record::{Rec, Val};
use super::resolve::{Resolver, key, slug, text, value};
use crate::attributes::{QualityBand, QualityTarget, TechnicalBinding};
use crate::document::Edge;
use crate::ids::{
    ComponentId, ConstraintId, CriterionId, ElementId, FeatureGroupId, FeatureId, NodeId,
    ProductId, QualityAttributeId, RequirementId, SectionId,
};
use crate::names::{CategoryName, TagName};
use crate::statement::{EarsForm, Statement};
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent, Section,
};

/// The records of a list field.
fn records<'r>(record: &'r Rec, field: &str) -> impl Iterator<Item = &'r Rec> {
    record.list(field).iter().filter_map(|value| match value {
        Val::Record(record) => Some(record),
        _ => None,
    })
}

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
        records(record, "sections")
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
            feature_groups: records(record, "featureGroups")
                .map(|group| self.feature_group(group))
                .collect::<Result<_, _>>()?,
        })
    }

    fn feature_group(&mut self, record: &Rec) -> Result<FeatureGroup, ReadError> {
        let id = FeatureGroupId::new(key(record, "key"));
        let product_path = self.path.clone();
        let files: BTreeMap<String, (SpecPath, &Rec)> = self
            .tree
            .records(|role| matches!(role, Role::Feature { .. }))
            .map(|(path, _, feature)| (text(feature, "key"), (path.clone(), feature)))
            .collect();
        let mut features = Vec::new();
        for listed in record.list("features") {
            let (path, feature) = &files[listed.text().unwrap_or_default()];
            self.path = path.clone();
            features.push(self.feature(feature)?);
        }
        self.path = product_path;
        Ok(FeatureGroup {
            id: Some(id),
            slug: slug(record),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            features,
            constraints: records(record, "constraints")
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
            requirements: records(record, "requirements")
                .map(|requirement| self.requirement(requirement))
                .collect::<Result<_, _>>()?,
            quality_attributes: records(record, "qualityAttributes")
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
            criteria: records(record, "criteria")
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
            components: records(record, "components")
                .map(|component| self.component(component))
                .collect::<Result<_, _>>()?,
            constraints: records(record, "constraints")
                .map(|constraint| self.constraint(constraint))
                .collect::<Result<_, _>>()?,
            quality_attributes: records(record, "qualityAttributes")
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
            conservation_kind: record.text("conservationKind").map(value),
            verification_fate: record.text("verificationFate").map(value),
        })
    }

    fn quality_attribute(&mut self, record: &Rec) -> Result<QualityAttribute, ReadError> {
        let id = QualityAttributeId::new(key(record, "key"));
        let target = record.record("qualityTarget").expect("a checked target");
        Ok(QualityAttribute {
            id: Some(id),
            name: text(record, "name"),
            details: self.details(record, id.into())?,
            kind: value(&text(record, "qaKind")),
            target: QualityTarget {
                metric: text(target, "metric"),
                band: band_of(target.record("band").expect("a checked band")),
                unit: text(target, "unit"),
                aggregation: value(&text(target, "aggregation")),
                scenario: text(target, "scenario"),
                environment: text(target, "environment"),
            },
            verification_fate: record.text("verificationFate").map(value),
        })
    }
}

fn band_of(band: &Rec) -> QualityBand {
    let bound = |field: &str| text(band, field);
    match band.text("kind").unwrap_or_default() {
        "at_most" => QualityBand::AtMost {
            upper: bound("upper"),
        },
        "at_least" => QualityBand::AtLeast {
            lower: bound("lower"),
        },
        "exactly" => QualityBand::Exactly {
            value: bound("value"),
        },
        _ => QualityBand::Between {
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
    let slot = |name: &str| text(record, name);
    let (system, shall) = (slot("system"), slot("shall"));
    Statement::Form(match record.text("form").unwrap_or_default() {
        "event_driven" => EarsForm::EventDriven {
            when: slot("when"),
            system,
            shall,
        },
        "state_driven" => EarsForm::StateDriven {
            r#while: slot("while"),
            system,
            shall,
        },
        "unwanted" => EarsForm::Unwanted {
            r#if: slot("if"),
            system,
            shall,
        },
        "optional" => EarsForm::Optional {
            r#where: slot("where"),
            system,
            shall,
        },
        _ => EarsForm::Ubiquitous { system, shall },
    })
}
