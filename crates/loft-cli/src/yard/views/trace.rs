//! The `trace` view: the document `loft reconcile` prints, with the branch's criteria, those
//! under each Feature whose file the branch added to or changed against the base revision, so
//! that the view grows with the change and not with the specification.

use std::collections::BTreeSet;
use std::path::Path;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::kind::NodeKind;
use loftspec::node::Node;
use serde_json::{Value, json};

use crate::stop::{self, Stop};
use crate::{reconcile, status};

/// The report of a `trace` view of the tree under `tree`, against `base`.
pub fn report(tree: &Path, base: &str) -> Result<Value, Stop> {
    let reconciled = reconcile::json_document(tree)?;
    let status = status::json_document(tree, Some(base))?;
    let changed: BTreeSet<&str> = ["added", "changed"]
        .iter()
        .filter_map(|change| status["base"][change].as_array())
        .flatten()
        .filter_map(|node| node["path"].as_str())
        .collect();
    let files = stop::read_files(tree)?;
    let (document, sites) = codec::read_sited(&files)?;
    let mut features = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let in_branch = sites
            .get(id.key())
            .is_some_and(|site| changed.contains(site.path.to_string().as_str()));
        if node.kind() == NodeKind::Feature && in_branch {
            features.push(feature(&document, node));
        }
    });
    Ok(json!({
        "reconcile": reconciled,
        "base": status["base"]["revision"].clone(),
        "commit": status["base"]["commit"].clone(),
        "features": features,
    }))
}

/// A Feature with its criteria, each by its key, label and name.
fn feature(document: &Document, node: Node<'_>) -> Value {
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if below.kind() == NodeKind::Criterion {
            criteria.push(named(document, below));
        }
    });
    let mut feature = named(document, node);
    feature["criteria"] = Value::Array(criteria);
    feature
}

fn named(document: &Document, node: Node<'_>) -> Value {
    let id = node.id();
    json!({
        "key": id.map(|id| id.key().to_string()),
        "label": id.and_then(|id| document.bindings.label_of(id)).map(|label| label.to_string()),
        "name": node.name(),
    })
}
