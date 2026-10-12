//! References and headings in v1 prose (D-11 F-14): each resolvable label reference in a node's
//! name or body becomes one `mentions` edge from the node to the element it names (RQ-53); a
//! reference that does not resolve and a heading in a body are reported (AC-191, AC-193). A
//! label-shaped text above every label of its prefix in the export, such as the curve name
//! `P-256`, is not a reference (AC-891). Bodies themselves are kept byte for byte (RQ-54).

use std::collections::BTreeSet;

use super::Finding;
use super::export::Export;
use super::tree::NodeIds;
use crate::bindings::{Bindings, Counters};
use crate::document::Edge;
use crate::ids::ElementId;
use crate::prose::{heading_lines, label_references};

/// The `mentions` edges of every node, in export order, and the findings about its prose.
pub(super) fn mentions(
    export: &Export,
    nodes: &NodeIds,
    bindings: &Bindings,
) -> (Vec<Edge>, Vec<Finding>) {
    let mut edges = Vec::new();
    let mut findings = Vec::new();
    let allocated = Counters::above(bindings);
    for v1 in &export.nodes {
        let Some(node) = nodes.get(&v1.node_id).copied() else {
            continue;
        };
        let source = ElementId::from(node);
        let body = v1.body.as_deref().unwrap_or("");
        let mut targets = BTreeSet::new();
        for reference in label_references(&v1.name)
            .into_iter()
            .chain(label_references(body))
            .filter(|reference| allocated.has_allocated(*reference))
        {
            match bindings.resolve(reference) {
                Some(target) if targets.insert(target) => {
                    edges.push(Edge::Mentions { source, target });
                }
                Some(_) => {}
                None => findings.push(Finding::UnresolvedReference { node, reference }),
            }
        }
        findings.extend(
            heading_lines(body)
                .into_iter()
                .map(|line| Finding::HeadingInBody { node, line }),
        );
    }
    (edges, findings)
}
