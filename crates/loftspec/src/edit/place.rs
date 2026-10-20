//! Where a node sits: its path from the root, as the list of each kind and the position in it,
//! and the lists of a document reached by that path, borrowed mutably.

use crate::document::Document;
use crate::ids::NodeId;
use crate::kind::NodeKind;
use crate::node::Node;
use crate::tree::{
    Component, Constraint, Criterion, Feature, FeatureGroup, Product, QualityAttribute,
    Requirement, RootComponent, Section,
};

use super::subtree::OwnedNode;

/// One step down the tree: the list of nodes of a kind, and the position in it.
pub(super) type Step = (NodeKind, usize);

/// The steps from the root to the node `id`; none when the document does not hold it.
pub(super) fn path(document: &Document, id: NodeId) -> Option<Vec<Step>> {
    let mut steps = Vec::new();
    search(document.root_nodes(), id, &mut steps).then_some(steps)
}

/// Searches `nodes`, a parent's children, and beneath them for `id`, extending `steps` with the
/// way to it.
fn search<'a>(nodes: impl Iterator<Item = Node<'a>>, id: NodeId, steps: &mut Vec<Step>) -> bool {
    let mut counts = [0; NodeKind::ALL.len()];
    for node in nodes {
        let kind = node.kind();
        let count = &mut counts[kind as usize];
        steps.push((kind, *count));
        *count += 1;
        if node.id() == Some(id) || search(node.children().into_iter(), id, steps) {
            return true;
        }
        steps.pop();
    }
    false
}

/// The node the steps lead to; none for the root, or steps that lead nowhere.
pub(super) fn at<'a>(document: &'a Document, steps: &[Step]) -> Option<Node<'a>> {
    let ((first_kind, first), rest) = steps.split_first()?;
    let mut node = document
        .root_nodes()
        .filter(|node| node.kind() == *first_kind)
        .nth(*first)?;
    for &(kind, position) in rest {
        node = node
            .children()
            .into_iter()
            .filter(|child| child.kind() == kind)
            .nth(position)?;
    }
    Some(node)
}

/// The length of the list of `kind` under the node the steps lead to, or under the root.
pub(super) fn length(document: &Document, steps: &[Step], kind: NodeKind) -> usize {
    let of = |nodes: Vec<Node<'_>>| nodes.into_iter().filter(|node| node.kind() == kind).count();
    if steps.is_empty() {
        of(document.root_nodes().collect())
    } else {
        at(document, steps).map_or(0, |node| of(node.children()))
    }
}

/// A node of any kind, borrowed mutably.
pub(super) enum NodeMut<'a> {
    Product(&'a mut Product),
    FeatureGroup(&'a mut FeatureGroup),
    Feature(&'a mut Feature),
    Requirement(&'a mut Requirement),
    Criterion(&'a mut Criterion),
    Component(&'a mut Component),
    Constraint(&'a mut Constraint),
    QualityAttribute(&'a mut QualityAttribute),
    Section(&'a mut Section),
}

impl<'a> NodeMut<'a> {
    /// The node's list of children of `kind`; none for a kind it cannot hold.
    fn list(self, kind: NodeKind) -> Option<ListMut<'a>> {
        use NodeKind as K;
        Some(match (self, kind) {
            (Self::Product(node), K::FeatureGroup) => {
                ListMut::FeatureGroups(&mut node.feature_groups)
            }
            (Self::Product(node), K::Constraint) => ListMut::Constraints(&mut node.constraints),
            (Self::FeatureGroup(node), K::Feature) => ListMut::Features(&mut node.features),
            (Self::FeatureGroup(node), K::Constraint) => {
                ListMut::Constraints(&mut node.constraints)
            }
            (Self::FeatureGroup(node), K::QualityAttribute) => {
                ListMut::QualityAttributes(&mut node.quality_attributes)
            }
            (Self::Feature(node), K::Requirement) => ListMut::Requirements(&mut node.requirements),
            (Self::Feature(node), K::QualityAttribute) => {
                ListMut::QualityAttributes(&mut node.quality_attributes)
            }
            (Self::Requirement(node), K::Criterion) => ListMut::Criteria(&mut node.criteria),
            (Self::Component(node), K::Component) => ListMut::Components(&mut node.components),
            (Self::Component(node), K::Constraint) => ListMut::Constraints(&mut node.constraints),
            (Self::Component(node), K::QualityAttribute) => {
                ListMut::QualityAttributes(&mut node.quality_attributes)
            }
            (node, K::Section) => ListMut::Sections(node.sections()),
            _ => return None,
        })
    }

    fn sections(self) -> &'a mut Vec<Section> {
        match self {
            Self::Product(node) => &mut node.details.sections,
            Self::FeatureGroup(node) => &mut node.details.sections,
            Self::Feature(node) => &mut node.details.sections,
            Self::Requirement(node) => &mut node.details.sections,
            Self::Criterion(node) => &mut node.details.sections,
            Self::Component(node) => &mut node.details.sections,
            Self::Constraint(node) => &mut node.details.sections,
            Self::QualityAttribute(node) => &mut node.details.sections,
            Self::Section(node) => &mut node.sections,
        }
    }
}

/// A parent's list of nodes of one kind, borrowed mutably. The root holds Products and root
/// Components.
pub(super) enum ListMut<'a> {
    Products(&'a mut Vec<Product>),
    RootComponents(&'a mut Vec<RootComponent>),
    FeatureGroups(&'a mut Vec<FeatureGroup>),
    Features(&'a mut Vec<Feature>),
    Requirements(&'a mut Vec<Requirement>),
    Criteria(&'a mut Vec<Criterion>),
    Components(&'a mut Vec<Component>),
    Constraints(&'a mut Vec<Constraint>),
    QualityAttributes(&'a mut Vec<QualityAttribute>),
    Sections(&'a mut Vec<Section>),
}

impl<'a> ListMut<'a> {
    /// The document's root list of `kind`; none for a kind the root cannot hold.
    pub(super) fn root(document: &'a mut Document, kind: NodeKind) -> Option<Self> {
        match kind {
            NodeKind::Product => Some(Self::Products(&mut document.products)),
            NodeKind::Component => Some(Self::RootComponents(&mut document.components)),
            _ => None,
        }
    }

    /// The list of `kind` under the node the steps lead to, or the root's when there are none.
    pub(super) fn under(
        document: &'a mut Document,
        steps: &[Step],
        kind: NodeKind,
    ) -> Option<Self> {
        let Some(((first_kind, first), rest)) = steps.split_first() else {
            return Self::root(document, kind);
        };
        let mut node = Self::root(document, *first_kind)?.get(*first)?;
        for &(step_kind, position) in rest {
            node = node.list(step_kind)?.get(position)?;
        }
        node.list(kind)
    }

    fn get(self, position: usize) -> Option<NodeMut<'a>> {
        match self {
            Self::Products(list) => list.get_mut(position).map(NodeMut::Product),
            Self::RootComponents(list) => list
                .get_mut(position)
                .map(|root| NodeMut::Component(&mut root.component)),
            Self::FeatureGroups(list) => list.get_mut(position).map(NodeMut::FeatureGroup),
            Self::Features(list) => list.get_mut(position).map(NodeMut::Feature),
            Self::Requirements(list) => list.get_mut(position).map(NodeMut::Requirement),
            Self::Criteria(list) => list.get_mut(position).map(NodeMut::Criterion),
            Self::Components(list) => list.get_mut(position).map(NodeMut::Component),
            Self::Constraints(list) => list.get_mut(position).map(NodeMut::Constraint),
            Self::QualityAttributes(list) => list.get_mut(position).map(NodeMut::QualityAttribute),
            Self::Sections(list) => list.get_mut(position).map(NodeMut::Section),
        }
    }

    /// Takes the node at `position` out of the list.
    pub(super) fn remove(self, position: usize) -> OwnedNode {
        match self {
            Self::Products(list) => list.remove(position).into(),
            Self::RootComponents(list) => list.remove(position).into(),
            Self::FeatureGroups(list) => list.remove(position).into(),
            Self::Features(list) => list.remove(position).into(),
            Self::Requirements(list) => list.remove(position).into(),
            Self::Criteria(list) => list.remove(position).into(),
            Self::Components(list) => list.remove(position).into(),
            Self::Constraints(list) => list.remove(position).into(),
            Self::QualityAttributes(list) => list.remove(position).into(),
            Self::Sections(list) => list.remove(position).into(),
        }
    }

    /// Puts `node` into the list at `position`. A Component put at the root takes the slug it
    /// carries, and one put under a Component leaves its slug, which a nested Component has no
    /// file to fix. Whether the list took it: it takes no node of another kind, nor a Component
    /// without a slug at the root.
    #[must_use]
    pub(super) fn insert(self, position: usize, node: OwnedNode) -> bool {
        match (self, node) {
            (Self::Products(list), OwnedNode::Product(node)) => list.insert(position, node),
            (Self::RootComponents(list), OwnedNode::RootComponent(node)) => {
                list.insert(position, node);
            }
            (Self::FeatureGroups(list), OwnedNode::FeatureGroup(node)) => {
                list.insert(position, node);
            }
            (Self::Features(list), OwnedNode::Feature(node)) => list.insert(position, node),
            (Self::Requirements(list), OwnedNode::Requirement(node)) => list.insert(position, node),
            (Self::Criteria(list), OwnedNode::Criterion(node)) => list.insert(position, node),
            (Self::Components(list), OwnedNode::Component(node)) => list.insert(position, node),
            (Self::Components(list), OwnedNode::RootComponent(root)) => {
                list.insert(position, root.component);
            }
            (Self::Constraints(list), OwnedNode::Constraint(node)) => list.insert(position, node),
            (Self::QualityAttributes(list), OwnedNode::QualityAttribute(node)) => {
                list.insert(position, node);
            }
            (Self::Sections(list), OwnedNode::Section(node)) => list.insert(position, node),
            _ => return false,
        }
        true
    }
}
