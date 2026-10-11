//! The structural rules `SH-001`..`SH-010` (F-19), with the ids and severities of `loft-lint`
//! 1.4.0.

use std::collections::{BTreeMap, BTreeSet};

use super::context::{Context, Item};
use super::{Raw, RuleId};
use crate::ids::NodeId;
use crate::kind::NodeKind;
use crate::node::Node;

pub(super) fn check(context: &Context<'_>, out: &mut Vec<Raw>) {
    for item in &context.items {
        let key = item.id.key();
        let one = |rule, message: &str| Raw::new(rule, vec![key], message);
        match item.node {
            Node::Feature(feature) => {
                if feature
                    .id
                    .is_some_and(|id| !context.realizing.contains(&id))
                {
                    out.push(one(RuleId::Sh001, "Feature has no `realizes` edge."));
                }
                if feature.requirements.is_empty() {
                    out.push(one(RuleId::Sh004, "Feature has no Requirement."));
                }
            }
            Node::Component(component) => {
                let realized = component
                    .id
                    .is_some_and(|id| context.realized.contains(&id));
                if component.components.is_empty() && !realized {
                    out.push(one(RuleId::Sh002, "Leaf Component has no `realizes` edge."));
                }
                if looks_like_a_path(&component.name) {
                    out.push(one(
                        RuleId::Sh010,
                        "Component name resembles a concrete file or directory.",
                    ));
                }
            }
            Node::Requirement(requirement) if requirement.criteria.is_empty() => {
                out.push(one(RuleId::Sh003, "Requirement has no criterion."));
            }
            Node::Product(product) if product.feature_groups.is_empty() => {
                out.push(one(RuleId::Sh005, "Product has no Feature Group."));
            }
            Node::FeatureGroup(group) if group.features.is_empty() => {
                out.push(one(RuleId::Sh005, "Feature Group has no Feature."));
            }
            _ => {}
        }
        for tag in item.tags() {
            if !context
                .document
                .tags
                .iter()
                .any(|entry| entry.name.as_str() == tag)
            {
                out.push(one(RuleId::Sh009, "Tag names no tag registry entry.").term(tag));
            }
        }
    }
    unused_registry_entries(context, out);
    duplicate_sibling_names(context, out);
    bare_component_subtrees(context, out);
}

/// `SH-006`: a registry entry no node lists.
fn unused_registry_entries(context: &Context<'_>, out: &mut Vec<Raw>) {
    let applied: BTreeSet<&str> = context.items.iter().flat_map(Item::tags).collect();
    for entry in &context.document.tags {
        if !applied.contains(entry.name.as_str()) {
            out.push(
                Raw::new(
                    RuleId::Sh006,
                    vec![entry.id.key()],
                    "Tag registry entry is applied by no node.",
                )
                .term(entry.name.as_str()),
            );
        }
    }
}

/// Siblings compare within these groups of kinds, as `loft-lint` 1.4.0 ranks its node types.
const fn rank(kind: NodeKind) -> u8 {
    match kind {
        NodeKind::Product | NodeKind::FeatureGroup | NodeKind::Feature | NodeKind::Requirement => 0,
        NodeKind::Component => 1,
        NodeKind::Constraint => 2,
        NodeKind::QualityAttribute => 3,
        NodeKind::Criterion => 5,
        NodeKind::Section => 6,
    }
}

/// `SH-007`: siblings of one rank whose names are equal after trimming and lower-casing; one
/// finding names them all.
fn duplicate_sibling_names(context: &Context<'_>, out: &mut Vec<Raw>) {
    let mut groups = BTreeMap::<(Option<NodeId>, u8, String), Vec<_>>::new();
    for item in &context.items {
        let name = item.name().trim().to_lowercase();
        let group = (item.parent, rank(item.id.kind()), name);
        groups.entry(group).or_default().push(item.id.key());
    }
    for mut keys in groups.into_values().filter(|keys| keys.len() > 1) {
        keys.sort_unstable();
        out.push(Raw::new(
            RuleId::Sh007,
            keys,
            "Siblings have the same name after trimming and lower-casing.",
        ));
    }
}

/// `SH-008`: the topmost Component of a subtree that holds no Constraint and no Quality
/// Attribute, when its parent's subtree does or it has no parent.
fn bare_component_subtrees(context: &Context<'_>, out: &mut Vec<Raw>) {
    let mut envelopes = BTreeMap::new();
    for item in &context.items {
        if let Node::Component(component) = item.node {
            envelope(component, &mut envelopes);
        }
    }
    for item in &context.items {
        let Node::Component(component) = item.node else {
            continue;
        };
        let parent_has = item
            .parent
            .is_none_or(|parent| envelopes.get(&parent.key()).copied().unwrap_or(false));
        if !envelope(component, &mut envelopes) && parent_has {
            out.push(Raw::new(
                RuleId::Sh008,
                vec![item.id.key()],
                "Component subtree has no Constraint or Quality Attribute.",
            ));
        }
    }
}

fn envelope(
    component: &crate::tree::Component,
    memo: &mut BTreeMap<crate::identity::Key, bool>,
) -> bool {
    if let Some(known) = component.id.and_then(|id| memo.get(&id.key())) {
        return *known;
    }
    let attached = !component.constraints.is_empty() || !component.quality_attributes.is_empty();
    let held = component
        .components
        .iter()
        .any(|child| envelope(child, memo));
    let result = attached || held;
    if let Some(id) = component.id {
        memo.insert(id.key(), result);
    }
    result
}

/// `SH-010`: a name with a path separator, or ending in a file extension of one to four ASCII
/// letters and digits.
fn looks_like_a_path(name: &str) -> bool {
    let extension = name.rsplit_once('.').is_some_and(|(_, extension)| {
        (1..=4).contains(&extension.len()) && extension.chars().all(|c| c.is_ascii_alphanumeric())
    });
    name.contains('/') || name.contains('\\') || extension
}
