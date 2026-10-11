//! The node tree of a v1 export (D-11 F-13): parentage from the children each node lists
//! (RQ-46), sibling order by ordinal (RQ-68), one v2 node per v1 node of the corresponding kind
//! (RQ-44), keys and labels kept (RQ-45), and each Facet Tag a tag on its parent (RQ-47).

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use super::export::{Export, ExternalId, Node, NodeType, Tier};
use super::{ConvertError, Finding};
use crate::document::Document;
use crate::identity::{Key, Label};
use crate::ids::{
    ComponentId, ConstraintId, CriterionId, ElementId, FeatureGroupId, FeatureId, NodeId,
    ProductId, QualityAttributeId, RequirementId,
};
use crate::names::TagName;
use crate::statement::Statement;
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement,
};

/// The kind of a v1 node, from its type and tier.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Kind {
    Product,
    FeatureGroup,
    Feature,
    Requirement,
    Criterion,
    Component,
    Constraint,
    QualityAttribute,
    FacetTag,
}

fn kind(node: &Node) -> Result<Kind, ConvertError> {
    kind_of(node.node_type, node.tier).ok_or_else(|| missing(node, "tier"))
}

/// The kind of a v1 type and tier; a tier section without a tier has none.
pub(super) const fn kind_of(node_type: NodeType, tier: Option<Tier>) -> Option<Kind> {
    Some(match (node_type, tier) {
        (NodeType::TierSection, Some(Tier::Product)) => Kind::Product,
        (NodeType::TierSection, Some(Tier::FeatureGroup)) => Kind::FeatureGroup,
        (NodeType::TierSection, Some(Tier::Feature)) => Kind::Feature,
        (NodeType::TierSection, Some(Tier::Requirement)) => Kind::Requirement,
        (NodeType::TierSection, None) => return None,
        (NodeType::AcceptanceCriterion, _) => Kind::Criterion,
        (NodeType::Component, _) => Kind::Component,
        (NodeType::Constraint, _) => Kind::Constraint,
        (NodeType::QualityAttribute, _) => Kind::QualityAttribute,
        (NodeType::FacetTag, _) => Kind::FacetTag,
    })
}

/// The typed ID of every node of the export but its Facet Tags, by UUID.
pub(super) fn node_ids(export: &Export) -> Result<BTreeMap<Uuid, NodeId>, ConvertError> {
    let mut ids = BTreeMap::new();
    for node in &export.nodes {
        if let Some(id) = typed_id(kind(node)?, node.node_id) {
            ids.insert(node.node_id, id);
        }
    }
    Ok(ids)
}

/// The typed ID a v1 node of `kind` has under the Format; a Facet Tag has none.
pub(super) fn typed_id(kind: Kind, uuid: Uuid) -> Option<NodeId> {
    let key = Key::from(uuid);
    Some(match kind {
        Kind::Product => ProductId::new(key).into(),
        Kind::FeatureGroup => FeatureGroupId::new(key).into(),
        Kind::Feature => FeatureId::new(key).into(),
        Kind::Requirement => RequirementId::new(key).into(),
        Kind::Criterion => CriterionId::new(key).into(),
        Kind::Component => ComponentId::new(key).into(),
        Kind::Constraint => ConstraintId::new(key).into(),
        Kind::QualityAttribute => QualityAttributeId::new(key).into(),
        Kind::FacetTag => return None,
    })
}

fn missing(node: &Node, field: &'static str) -> ConvertError {
    ConvertError::Missing {
        node: node.external_id.clone(),
        field,
    }
}

fn key(node: &Node) -> Key {
    node.node_id.into()
}

/// The export's nodes with their parents and ordered children.
pub(super) struct Tree<'a> {
    nodes: BTreeMap<Uuid, &'a Node>,
    children: BTreeMap<Uuid, Vec<&'a Node>>,
    roots: Vec<&'a Node>,
}

impl<'a> Tree<'a> {
    pub(super) fn new(export: &'a Export) -> Result<Self, ConvertError> {
        let nodes: BTreeMap<Uuid, &Node> = export
            .nodes
            .iter()
            .map(|node| (node.node_id, node))
            .collect();
        let mut parents: BTreeMap<Uuid, &Node> = BTreeMap::new();
        let mut children = BTreeMap::new();
        for parent in &export.nodes {
            let mut listed = listed_children(parent, &nodes, &mut parents)?;
            // Ascending ordinal; the listed order breaks a tie.
            listed.sort_by_key(|node| node.ordinal);
            children.insert(parent.node_id, listed);
        }
        let mut roots: Vec<&Node> = export
            .nodes
            .iter()
            .filter(|node| !parents.contains_key(&node.node_id))
            .collect();
        roots.sort_by_key(|node| node.ordinal);
        Ok(Self {
            nodes,
            children,
            roots,
        })
    }

    /// Builds the Products, the root Components and the bindings of every node, reporting each
    /// Requirement stored as text. Every node must be reached from a root.
    pub(super) fn convert(
        &self,
        document: &mut Document,
        findings: &mut Vec<Finding>,
    ) -> Result<(), ConvertError> {
        let mut builder = Builder {
            tree: self,
            findings,
            reached: BTreeSet::new(),
            labels: Vec::new(),
        };
        for root in &self.roots {
            match kind(root)? {
                Kind::Product => document.products.push(builder.product(root)?),
                Kind::Component => document.components.push(builder.component(root)?),
                _ => {
                    return Err(ConvertError::Orphan {
                        node: root.external_id.clone(),
                    });
                }
            }
        }
        if let Some(node) = self
            .nodes
            .values()
            .find(|node| !builder.reached.contains(&node.node_id))
        {
            // Reached from no root: a cycle of nodes listing each other.
            return Err(ConvertError::Orphan {
                node: node.external_id.clone(),
            });
        }
        for (element, external_id) in builder.labels {
            let label =
                Label::parse(&external_id.to_string()).map_err(|source| ConvertError::Label {
                    node: external_id.clone(),
                    source,
                })?;
            document.bindings.bind(element, label)?;
        }
        Ok(())
    }
}

/// The nodes `parent` lists, recording `parent` as each one's parent; a child listed by two
/// parents or held by no node is refused.
fn listed_children<'a>(
    parent: &'a Node,
    nodes: &BTreeMap<Uuid, &'a Node>,
    parents: &mut BTreeMap<Uuid, &'a Node>,
) -> Result<Vec<&'a Node>, ConvertError> {
    parent
        .children_hashes
        .iter()
        .map(|child| {
            let node = *nodes
                .get(&child.node_id)
                .ok_or_else(|| ConvertError::UnknownChild {
                    parent: parent.external_id.clone(),
                    child: child.node_id,
                })?;
            match parents.insert(child.node_id, parent) {
                Some(first) => Err(ConvertError::TwoParents {
                    child: node.external_id.clone(),
                    first: first.external_id.clone(),
                    second: parent.external_id.clone(),
                }),
                None => Ok(node),
            }
        })
        .collect()
}

struct Builder<'t, 'a> {
    tree: &'t Tree<'a>,
    findings: &'t mut Vec<Finding>,
    reached: BTreeSet<Uuid>,
    labels: Vec<(ElementId, ExternalId)>,
}

/// One child of a node, sorted into the list its kind belongs in.
enum Child<'a> {
    Node(Kind, &'a Node),
    Tag(TagName),
}

impl<'a> Builder<'_, 'a> {
    /// Marks a node reached and records its label under its typed ID.
    fn visit(&mut self, node: &Node, element: impl Into<ElementId>) -> Details {
        self.reached.insert(node.node_id);
        self.labels.push((element.into(), node.external_id.clone()));
        Details {
            body: node.body.clone(),
            tags: Vec::new(),
            sections: Vec::new(),
        }
    }

    /// The node's children in ordinal order, with its Facet Tags read as tags.
    fn children(&mut self, node: &Node) -> Result<Vec<Child<'a>>, ConvertError> {
        let listed = self
            .tree
            .children
            .get(&node.node_id)
            .map_or(&[][..], Vec::as_slice);
        listed
            .iter()
            .map(|child| match kind(child)? {
                Kind::FacetTag => {
                    self.reached.insert(child.node_id);
                    TagName::parse(&child.name)
                        .map(Child::Tag)
                        .map_err(|source| ConvertError::Tag {
                            node: child.external_id.clone(),
                            source,
                        })
                }
                kind => Ok(Child::Node(kind, child)),
            })
            .collect()
    }

    fn product(&mut self, node: &Node) -> Result<Product, ConvertError> {
        let mut product = Product {
            id: Some(ProductId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, ProductId::new(key(node))),
            feature_groups: Vec::new(),
        };
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => product.details.tags.push(tag),
                Child::Node(Kind::FeatureGroup, child) => {
                    product.feature_groups.push(self.feature_group(child)?);
                }
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(product)
    }

    fn feature_group(&mut self, node: &Node) -> Result<FeatureGroup, ConvertError> {
        let mut group = FeatureGroup {
            id: Some(FeatureGroupId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, FeatureGroupId::new(key(node))),
            features: Vec::new(),
            constraints: Vec::new(),
        };
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => group.details.tags.push(tag),
                Child::Node(Kind::Feature, child) => group.features.push(self.feature(child)?),
                Child::Node(Kind::Constraint, child) => {
                    group.constraints.push(self.constraint(child)?);
                }
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(group)
    }

    fn feature(&mut self, node: &Node) -> Result<Feature, ConvertError> {
        let mut feature = Feature {
            id: Some(FeatureId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, FeatureId::new(key(node))),
            requirements: Vec::new(),
            quality_attributes: Vec::new(),
        };
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => feature.details.tags.push(tag),
                Child::Node(Kind::Requirement, child) => {
                    feature.requirements.push(self.requirement(child)?);
                }
                Child::Node(Kind::QualityAttribute, child) => {
                    feature
                        .quality_attributes
                        .push(self.quality_attribute(child)?);
                }
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(feature)
    }

    fn requirement(&mut self, node: &Node) -> Result<Requirement, ConvertError> {
        let id = RequirementId::new(key(node));
        let statement = Statement::parse(&node.name);
        if matches!(statement, Statement::Text(_)) {
            self.findings
                .push(Finding::StatementAsText { requirement: id });
        }
        let mut requirement = Requirement {
            id: Some(id),
            statement,
            details: self.visit(node, id),
            criteria: Vec::new(),
        };
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => requirement.details.tags.push(tag),
                Child::Node(Kind::Criterion, child) => {
                    requirement.criteria.push(self.criterion(child)?);
                }
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(requirement)
    }

    fn criterion(&mut self, node: &Node) -> Result<Criterion, ConvertError> {
        let mut criterion = Criterion {
            id: Some(CriterionId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, CriterionId::new(key(node))),
        };
        self.leaf(node, &mut criterion.details)?;
        Ok(criterion)
    }

    fn component(&mut self, node: &Node) -> Result<Component, ConvertError> {
        let mut component = Component {
            id: Some(ComponentId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, ComponentId::new(key(node))),
            components: Vec::new(),
            constraints: Vec::new(),
            quality_attributes: Vec::new(),
        };
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => component.details.tags.push(tag),
                Child::Node(Kind::Component, child) => {
                    component.components.push(self.component(child)?);
                }
                Child::Node(Kind::Constraint, child) => {
                    component.constraints.push(self.constraint(child)?);
                }
                Child::Node(Kind::QualityAttribute, child) => {
                    component
                        .quality_attributes
                        .push(self.quality_attribute(child)?);
                }
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(component)
    }

    fn constraint(&mut self, node: &Node) -> Result<Constraint, ConvertError> {
        let mut constraint = Constraint {
            id: Some(ConstraintId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, ConstraintId::new(key(node))),
            binding: node.technical_binding.clone(),
            conservation_kind: node.conservation_kind,
            verification_fate: node.verification_fate,
        };
        self.leaf(node, &mut constraint.details)?;
        Ok(constraint)
    }

    fn quality_attribute(&mut self, node: &Node) -> Result<QualityAttribute, ConvertError> {
        let mut quality = QualityAttribute {
            id: Some(QualityAttributeId::new(key(node))),
            name: node.name.clone(),
            details: self.visit(node, QualityAttributeId::new(key(node))),
            kind: node.qa_kind.ok_or_else(|| missing(node, "qaKind"))?,
            target: node
                .quality_target
                .clone()
                .ok_or_else(|| missing(node, "qualityTarget"))?,
            verification_fate: node.verification_fate,
        };
        self.leaf(node, &mut quality.details)?;
        Ok(quality)
    }

    /// A node that holds no structural children: only Facet Tags may stand under it.
    fn leaf(&mut self, node: &Node, details: &mut Details) -> Result<(), ConvertError> {
        for child in self.children(node)? {
            match child {
                Child::Tag(tag) => details.tags.push(tag),
                Child::Node(_, child) => return Err(containment(node, child)),
            }
        }
        Ok(())
    }
}

fn containment(parent: &Node, child: &Node) -> ConvertError {
    ConvertError::Containment {
        parent: parent.external_id.clone(),
        child: child.external_id.clone(),
    }
}
