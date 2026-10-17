//! The subjects of each Product's statements: the `system` slot of every formed statement of the
//! Requirements beneath it, counted, each marked when it is a term's name, an alias or a node's
//! name.

use std::collections::BTreeMap;

use super::super::context::Context;
use super::super::terms::phrase;
use super::{Mark, ProductSubjects, Subject};
use crate::document::Document;
use crate::kind::NodeKind;
use crate::statement::Statement;
use crate::text::lowercase;
use crate::tree::Product;

/// Each Product's subjects, in the manifest's order.
pub(super) fn of(context: &Context<'_>) -> Vec<ProductSubjects> {
    let names = node_names(context);
    context
        .document
        .products
        .iter()
        .map(|product| ProductSubjects {
            product: product.id.map(crate::ids::ProductId::key),
            subjects: subjects(product, context.document, &names),
        })
        .collect()
}

/// A subject as compared: in lower case, without a leading `the`, its words joined by single
/// spaces.
fn compared(text: &str) -> String {
    let words: Vec<String> = text.split_whitespace().map(lowercase).collect();
    let start = usize::from(words.len() > 1 && words[0] == "the");
    words[start..].join(" ")
}

/// A written subject without a leading `The` or `the`.
fn without_article(text: &str) -> &str {
    let text = text.trim();
    ["The ", "the "]
        .iter()
        .find_map(|article| text.strip_prefix(article))
        .unwrap_or(text)
        .trim_start()
}

fn subjects(product: &Product, document: &Document, names: &[String]) -> Vec<Subject> {
    let mut counted: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
    let requirements = product
        .feature_groups
        .iter()
        .flat_map(|group| &group.features)
        .flat_map(|feature| &feature.requirements);
    for requirement in requirements {
        if let Statement::Form(form) = &requirement.statement {
            let system = without_article(form.parts().1);
            if !system.is_empty() {
                *counted
                    .entry(compared(system))
                    .or_default()
                    .entry(system)
                    .or_default() += 1;
            }
        }
    }
    let mut subjects: Vec<Subject> = counted
        .into_iter()
        .map(|(key, forms)| {
            let text = forms
                .iter()
                .max_by(|left, right| left.1.cmp(right.1).then(right.0.cmp(left.0)))
                .map_or("", |(form, _)| form)
                .to_owned();
            Subject {
                mark: mark(&text, &key, document, names),
                count: forms.values().sum(),
                text,
            }
        })
        .collect();
    subjects.sort_by(|left, right| {
        right
            .count
            .cmp(&left.count)
            .then(left.text.cmp(&right.text))
    });
    subjects
}

/// Whether a subject is a term's name, a term's alias, or the name of a node.
fn mark(text: &str, key: &str, document: &Document, names: &[String]) -> Option<Mark> {
    let words = phrase(text);
    for term in &document.terms {
        if phrase(&term.name) == words {
            return Some(Mark::Term);
        }
        if term.aliases.iter().any(|alias| phrase(alias) == words) {
            return Some(Mark::Alias(term.name.clone()));
        }
    }
    names
        .binary_search_by(|name| name.as_str().cmp(key))
        .is_ok()
        .then_some(Mark::Node)
}

/// The names of the Products, Feature Groups, Features and Components, compared as subjects are,
/// sorted.
fn node_names(context: &Context<'_>) -> Vec<String> {
    let mut names: Vec<String> = context
        .items
        .iter()
        .filter(|item| {
            matches!(
                item.id.kind(),
                NodeKind::Product
                    | NodeKind::FeatureGroup
                    | NodeKind::Feature
                    | NodeKind::Component
            )
        })
        .map(|item| compared(&item.name()))
        .collect();
    names.sort_unstable();
    names.dedup();
    names
}
