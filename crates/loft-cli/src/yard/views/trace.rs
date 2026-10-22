//! The `trace` view: the document `loft reconcile` prints, with the branch's criteria, those
//! under each Feature whose file the branch added to or changed against the base revision, so
//! that the view grows with the change and not with the specification.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::kind::NodeKind;
use loftspec::node::Node;
use loftspec::trace::{self, Evidence};
use loftspec_store::FileStore;
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
    let unverified = unverified(tree)?;
    let mut features = Vec::new();
    document.walk(&mut |node| {
        let Some(id) = node.id() else { return };
        let in_branch = sites
            .get(id.key())
            .is_some_and(|site| changed.contains(site.path.to_string().as_str()));
        if node.kind() == NodeKind::Feature && in_branch {
            features.push(feature(&document, node, &unverified));
        }
    });
    Ok(json!({
        "reconcile": reconciled,
        "base": status["base"]["revision"].clone(),
        "commit": status["base"]["commit"].clone(),
        "features": features,
    }))
}

/// The reason each node linked `unverified` in the trace file gives, by its key.
fn unverified(tree: &Path) -> Result<BTreeMap<Key, String>, Stop> {
    let store = FileStore::open(tree)?;
    let trace = trace::read(store.trace()?.as_deref())?;
    Ok(trace
        .links
        .into_iter()
        .filter_map(|link| match link.evidence {
            Evidence::Unverified(reason) => Some((link.node, reason)),
            Evidence::Tests(_) => None,
        })
        .collect())
}

/// A Feature with its criteria, each by its key, label and name, and the reason one linked
/// `unverified` gives.
fn feature(document: &Document, node: Node<'_>, unverified: &BTreeMap<Key, String>) -> Value {
    let mut criteria = Vec::new();
    node.walk(&mut |below| {
        if below.kind() == NodeKind::Criterion {
            let mut criterion = named(document, below);
            let reason = below.id().and_then(|id| unverified.get(&id.key()));
            if let Some(reason) = reason {
                criterion["unverified"] = json!(reason);
            }
            criteria.push(criterion);
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
