//! Each Component's boundary as its subtree takes it on (F-50, ADR-15): a
//! Component without a `boundary` field has its parent Component's, and a root Component without
//! one is `private`.

use std::collections::BTreeSet;

use crate::attributes::Boundary;
use crate::document::Document;
use crate::ids::{ComponentId, NodeId};
use crate::tree::Component;

/// A Component that declares `private` beneath a Component whose boundary is `external`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PrivateUnderExternal {
    pub component: ComponentId,
    /// The nearest Component above it whose boundary is `external`.
    pub ancestor: ComponentId,
}

/// The boundary of every keyed Component of a document.
#[derive(Debug, Default)]
pub struct Boundaries {
    external: BTreeSet<ComponentId>,
    conflicts: Vec<PrivateUnderExternal>,
}

impl Boundaries {
    #[must_use]
    pub fn of(document: &Document) -> Self {
        let mut boundaries = Self::default();
        for root in &document.components {
            boundaries.visit(root, Above::default());
        }
        boundaries
    }

    /// Whether the Component's boundary, written or taken on from above, is `external`.
    #[must_use]
    pub fn is_external(&self, component: ComponentId) -> bool {
        self.external.contains(&component)
    }

    /// Whether the node is a Component whose boundary is `external`.
    #[must_use]
    pub fn is_external_node(&self, node: NodeId) -> bool {
        matches!(node, NodeId::Component(component) if self.is_external(component))
    }

    /// Every Component that declares `private` beneath an `external` one, in document order.
    #[must_use]
    pub fn conflicts(&self) -> &[PrivateUnderExternal] {
        &self.conflicts
    }

    /// Visits `component`, given whether the Component above it is `external` and the nearest
    /// keyed `external` Component above it.
    fn visit(&mut self, component: &Component, above: Above) {
        let external = match component.boundary {
            Some(Boundary::External) => true,
            Some(Boundary::Private) => {
                if let (Some(component), Some(ancestor)) = (component.id, above.ancestor) {
                    self.conflicts.push(PrivateUnderExternal {
                        component,
                        ancestor,
                    });
                }
                false
            }
            None => above.external,
        };
        if external && let Some(id) = component.id {
            self.external.insert(id);
        }
        let below = Above {
            external,
            ancestor: if external {
                component.id.or(above.ancestor)
            } else {
                None
            },
        };
        for child in &component.components {
            self.visit(child, below);
        }
    }
}

/// What a Component takes on from the Components above it.
#[derive(Clone, Copy, Default)]
struct Above {
    external: bool,
    ancestor: Option<ComponentId>,
}
