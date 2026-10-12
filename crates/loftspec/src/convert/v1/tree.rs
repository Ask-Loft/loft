//! The node tree of a v1 export (D-11 F-13): parentage from the children each node lists
//! (RQ-46), sibling order by ordinal (RQ-68), one v2 node per v1 node of the corresponding kind
//! (RQ-44), keys and labels kept (RQ-45), and each Facet Tag a tag on its parent (RQ-47).

use std::collections::{BTreeMap, BTreeSet};

use uuid::Uuid;

use super::export::{ComponentScope, Export, ExternalId, Node, NodeType, Tier};
use super::sidecar::{NodeFacts, TagApplication, V1Sidecar};
use super::{Conversion, ConvertError, Finding, label};
use crate::attributes::Boundary;
use crate::ids::{
    ComponentId, ConstraintId, CriterionId, FeatureGroupId, FeatureId, NodeId, ProductId,
    QualityAttributeId, RequirementId,
};
use crate::kind::NodeKind;
use crate::names::TagName;
use crate::slug::Slug;
use crate::statement::Statement;
use crate::tree::{
    Component, Constraint, Criterion, Details, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent,
};

/// The typed ID of every node of an export but its Facet Tags, by UUID.
pub(super) type NodeIds = BTreeMap<Uuid, NodeId>;

/// The typed ID a v1 node of this type and tier has under the Format: none for a Facet Tag, or
/// for a tier section without a tier.
pub(super) fn node_id(node_type: NodeType, tier: Option<Tier>, uuid: Uuid) -> Option<NodeId> {
    let kind = match (node_type, tier) {
        (NodeType::TierSection, Some(Tier::Product)) => NodeKind::Product,
        (NodeType::TierSection, Some(Tier::FeatureGroup)) => NodeKind::FeatureGroup,
        (NodeType::TierSection, Some(Tier::Feature)) => NodeKind::Feature,
        (NodeType::TierSection, Some(Tier::Requirement)) => NodeKind::Requirement,
        (NodeType::AcceptanceCriterion, _) => NodeKind::Criterion,
        (NodeType::Component, _) => NodeKind::Component,
        (NodeType::Constraint, _) => NodeKind::Constraint,
        (NodeType::QualityAttribute, _) => NodeKind::QualityAttribute,
        (NodeType::TierSection, None) | (NodeType::FacetTag, _) => return None,
    };
    Some(NodeId::new(kind, uuid.into()))
}

fn missing(node: &Node, field: &'static str) -> ConvertError {
    ConvertError::Missing {
        node: node.external_id.clone(),
        field,
    }
}

/// The export's nodes with their typed IDs, parents and ordered children.
pub(super) struct Tree<'a> {
    nodes: BTreeMap<Uuid, &'a Node>,
    pub(super) ids: NodeIds,
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
        let mut ids = NodeIds::new();
        for node in &export.nodes {
            match node_id(node.node_type, node.tier, node.node_id) {
                Some(id) => {
                    ids.insert(node.node_id, id);
                }
                None if node.node_type == NodeType::TierSection => {
                    return Err(missing(node, "tier"));
                }
                None => {}
            }
        }
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
            ids,
            children,
            roots,
        })
    }

    /// Builds the Products, the root Components and the bindings of every node, reporting each
    /// Requirement stored as text and recording each node's v1 facts in the sidecar. Every node
    /// must be reached from a root.
    pub(super) fn convert(&self, conversion: &mut Conversion) -> Result<(), ConvertError> {
        let Conversion {
            document,
            sidecar,
            findings,
        } = conversion;
        let mut builder = Builder {
            tree: self,
            sidecar,
            findings,
            reached: BTreeSet::new(),
            labels: Vec::new(),
            external_above: None,
        };
        for root in &self.roots {
            match self.ids.get(&root.node_id).map(|id| id.kind()) {
                Some(NodeKind::Product) => document.products.push(builder.product(root)?),
                Some(NodeKind::Component) => document.components.push(RootComponent {
                    slug: Slug::derive(&root.name, "component"),
                    component: builder.component(root)?,
                }),
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
        for (id, external_id) in builder.labels {
            document.bindings.bind(id, label(external_id)?)?;
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
    sidecar: &'t mut V1Sidecar,
    findings: &'t mut Vec<Finding>,
    reached: BTreeSet<Uuid>,
    labels: Vec<(NodeId, &'a ExternalId)>,
    /// The nearest Component above the one being converted that the export scopes as external.
    external_above: Option<ComponentId>,
}

/// A node's structural children in ordinal order, grouped by kind.
#[derive(Default)]
struct Children<'a>(BTreeMap<NodeKind, Vec<&'a Node>>);

impl<'a> Children<'a> {
    /// Adds a child of `kind` under a parent of `parent_kind`, refusing a child that kind may
    /// not hold (RQ-74).
    fn add(
        &mut self,
        parent_kind: NodeKind,
        kind: NodeKind,
        parent: &Node,
        child: &'a Node,
    ) -> Result<(), ConvertError> {
        if !parent_kind.may_contain(kind) {
            return Err(containment(parent, child));
        }
        self.0.entry(kind).or_default().push(child);
        Ok(())
    }

    /// The children of one kind, in ordinal order.
    fn take(&mut self, kind: NodeKind) -> Vec<&'a Node> {
        self.0.remove(&kind).unwrap_or_default()
    }
}

impl<'a> Builder<'_, 'a> {
    /// Starts a node: marks it reached, records its label and v1 facts, reads its Facet Tags
    /// into its details, and returns its structural children, refusing a child its kind may not
    /// hold (RQ-74).
    fn open(
        &mut self,
        node: &'a Node,
        id: impl Into<NodeId>,
    ) -> Result<(Details, Children<'a>), ConvertError> {
        let id = id.into();
        self.reached.insert(node.node_id);
        self.labels.push((id, &node.external_id));
        self.sidecar.nodes.insert(id, NodeFacts::of(node));
        let mut details = Details {
            body: node.body.clone(),
            ..Details::default()
        };
        let mut children = Children::default();
        let mut applications = Vec::new();
        let listed = self.tree.children.get(&node.node_id);
        for child in listed.map_or(&[][..], Vec::as_slice) {
            if let Some(child_id) = self.tree.ids.get(&child.node_id) {
                children.add(id.kind(), child_id.kind(), node, child)?;
                continue;
            }
            let application = self.facet_tag(child)?;
            details.tags.push(application.tag.clone());
            applications.push(application);
        }
        if !applications.is_empty() {
            self.sidecar.tags.insert(id, applications);
        }
        Ok((details, children))
    }

    /// A Facet Tag read as the tag it applies; a tag holds no body.
    fn facet_tag(&mut self, node: &Node) -> Result<TagApplication, ConvertError> {
        if node.body.is_some() {
            return Err(ConvertError::FacetTagBody {
                node: node.external_id.clone(),
            });
        }
        self.reached.insert(node.node_id);
        let tag = TagName::parse(&node.name).map_err(|source| ConvertError::Tag {
            node: node.external_id.clone(),
            source,
        })?;
        Ok(TagApplication {
            tag,
            node_id: node.node_id,
            external_id: node.external_id.clone(),
            ordinal: node.ordinal,
        })
    }

    /// Builds each child of one kind, in ordinal order.
    fn each<T>(
        &mut self,
        children: &mut Children<'a>,
        kind: NodeKind,
        build: fn(&mut Self, &'a Node) -> Result<T, ConvertError>,
    ) -> Result<Vec<T>, ConvertError> {
        children
            .take(kind)
            .into_iter()
            .map(|child| build(self, child))
            .collect()
    }

    fn product(&mut self, node: &'a Node) -> Result<Product, ConvertError> {
        let id = ProductId::new(node.node_id.into());
        let (details, mut children) = self.open(node, id)?;
        Ok(Product {
            id: Some(id),
            slug: Slug::derive(&node.name, "product"),
            name: node.name.clone(),
            details,
            feature_groups: self.each(
                &mut children,
                NodeKind::FeatureGroup,
                Self::feature_group,
            )?,
        })
    }

    fn feature_group(&mut self, node: &'a Node) -> Result<FeatureGroup, ConvertError> {
        let id = FeatureGroupId::new(node.node_id.into());
        let (details, mut children) = self.open(node, id)?;
        Ok(FeatureGroup {
            id: Some(id),
            slug: Slug::derive(&node.name, "feature-group"),
            name: node.name.clone(),
            details,
            features: self.each(&mut children, NodeKind::Feature, Self::feature)?,
            constraints: self.each(&mut children, NodeKind::Constraint, Self::constraint)?,
        })
    }

    fn feature(&mut self, node: &'a Node) -> Result<Feature, ConvertError> {
        let id = FeatureId::new(node.node_id.into());
        let (details, mut children) = self.open(node, id)?;
        Ok(Feature {
            id: Some(id),
            slug: Slug::derive(&node.name, "feature"),
            name: node.name.clone(),
            details,
            requirements: self.each(&mut children, NodeKind::Requirement, Self::requirement)?,
            quality_attributes: self.each(
                &mut children,
                NodeKind::QualityAttribute,
                Self::quality_attribute,
            )?,
        })
    }

    fn requirement(&mut self, node: &'a Node) -> Result<Requirement, ConvertError> {
        let id = RequirementId::new(node.node_id.into());
        let statement = Statement::parse(&node.name);
        if matches!(statement, Statement::Text(_)) {
            self.findings
                .push(Finding::StatementAsText { requirement: id });
        }
        let (details, mut children) = self.open(node, id)?;
        Ok(Requirement {
            id: Some(id),
            statement,
            details,
            criteria: self.each(&mut children, NodeKind::Criterion, Self::criterion)?,
        })
    }

    fn criterion(&mut self, node: &'a Node) -> Result<Criterion, ConvertError> {
        let id = CriterionId::new(node.node_id.into());
        let (details, _) = self.open(node, id)?;
        Ok(Criterion {
            id: Some(id),
            name: node.name.clone(),
            details,
        })
    }

    fn component(&mut self, node: &'a Node) -> Result<Component, ConvertError> {
        let id = ComponentId::new(node.node_id.into());
        let (details, mut children) = self.open(node, id)?;
        if let Some(scope) = node.component_scope {
            self.sidecar.scopes.insert(id, scope.into());
        }
        let boundary = match (node.component_scope, self.external_above) {
            (Some(ComponentScope::External), _) => Some(Boundary::External),
            (Some(_), Some(ancestor)) => {
                self.findings.push(Finding::InternalUnderExternal {
                    component: id,
                    ancestor,
                });
                None
            }
            _ => None,
        };
        let above = self.external_above;
        if boundary.is_some() {
            self.external_above = Some(id);
        }
        let components = self.each(&mut children, NodeKind::Component, Self::component);
        self.external_above = above;
        Ok(Component {
            id: Some(id),
            name: node.name.clone(),
            details,
            boundary,
            components: components?,
            constraints: self.each(&mut children, NodeKind::Constraint, Self::constraint)?,
            quality_attributes: self.each(
                &mut children,
                NodeKind::QualityAttribute,
                Self::quality_attribute,
            )?,
        })
    }

    fn constraint(&mut self, node: &'a Node) -> Result<Constraint, ConvertError> {
        let id = ConstraintId::new(node.node_id.into());
        let (details, _) = self.open(node, id)?;
        Ok(Constraint {
            id: Some(id),
            name: node.name.clone(),
            details,
            binding: node.technical_binding.clone(),
            conservation_kind: node.conservation_kind,
            verification_fate: node.verification_fate,
        })
    }

    fn quality_attribute(&mut self, node: &'a Node) -> Result<QualityAttribute, ConvertError> {
        let id = QualityAttributeId::new(node.node_id.into());
        let (details, _) = self.open(node, id)?;
        Ok(QualityAttribute {
            id: Some(id),
            name: node.name.clone(),
            details,
            kind: node.qa_kind.ok_or_else(|| missing(node, "qaKind"))?,
            target: node
                .quality_target
                .clone()
                .ok_or_else(|| missing(node, "qualityTarget"))?,
            verification_fate: node.verification_fate,
        })
    }
}

fn containment(parent: &Node, child: &Node) -> ConvertError {
    ConvertError::Containment {
        parent: parent.external_id.clone(),
        child: child.external_id.clone(),
    }
}
