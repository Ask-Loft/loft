//! The rules of Interfaces: an Interface that holds no Requirement and so promises nothing
//! (`V2-012`), and one that no Component of its document provides or requires (`V2-013`).

use std::collections::BTreeSet;

use super::context::Context;
use super::{Raw, RuleId};
use crate::document::Edge;
use crate::ids::InterfaceId;

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    let document = context.document;
    let linked: BTreeSet<InterfaceId> = document
        .edges
        .iter()
        .filter_map(|edge| match edge {
            Edge::Provides { interface, .. } | Edge::Requires { interface, .. } => Some(*interface),
            _ => None,
        })
        .collect();
    for interface in &document.interfaces {
        let Some(id) = interface.id else {
            continue;
        };
        if interface.requirements.is_empty() {
            out.push(Raw::new(
                RuleId::V2012,
                vec![id.key()],
                "The Interface holds no Requirement, so it promises nothing; state what a \
                 Component that provides it does.",
            ));
        }
        if !linked.contains(&id) {
            out.push(Raw::new(
                RuleId::V2013,
                vec![id.key()],
                "No Component of the document provides or requires the Interface.",
            ));
        }
    }
}
