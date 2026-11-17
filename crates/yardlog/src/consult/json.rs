//! The counts, the estimate and a scope as JSON, under the names a consultation file writes them,
//! as `loft yard consultation` reports them and the page reads them.

use serde_json::{Map, Value, json};

use super::{Estimate, Scope, Six};

/// The fields of `values` under `names`.
fn named<const N: usize>(names: [&str; N], values: [u64; N]) -> Value {
    names
        .into_iter()
        .zip(values)
        .map(|(name, value)| (name.to_owned(), json!(value)))
        .collect::<Map<_, _>>()
        .into()
}

impl Six {
    #[must_use]
    pub fn json(self) -> Value {
        named(Self::NAMES, self.values())
    }
}

impl Estimate {
    #[must_use]
    pub fn json(self) -> Value {
        named(Self::NAMES, self.values())
    }
}

/// `counts` and `estimate` as the fields of one object.
#[must_use]
pub fn tallies(counts: Six, estimate: Estimate) -> Map<String, Value> {
    Map::from_iter([
        ("counts".to_owned(), counts.json()),
        ("estimate".to_owned(), estimate.json()),
    ])
}

impl Scope {
    /// The scope's tallies and its order: the times it holds, the edits of code before the first
    /// consultation of the specification and, where code was edited, whether the specification
    /// was consulted first.
    #[must_use]
    pub fn json(&self) -> Map<String, Value> {
        let mut fields = tallies(self.counts, self.estimate);
        let order = self.order;
        if let Some(at) = order.first_spec {
            fields.insert("firstSpec".to_owned(), json!(at.to_string()));
        }
        if let Some(at) = order.first_code_edit {
            fields.insert("firstCodeEdit".to_owned(), json!(at.to_string()));
        }
        fields.insert(
            "codeEditsBeforeSpec".to_owned(),
            json!(order.code_edits_before_spec),
        );
        if let Some(first) = order.spec_first() {
            fields.insert("specFirst".to_owned(), json!(first));
        }
        fields
    }
}
