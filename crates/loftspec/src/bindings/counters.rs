//! The label counters of `spec/labels.yaml` (RQ-7).

use std::collections::BTreeMap;
use std::num::NonZeroU64;

use super::{Binding, Bindings};
use crate::identity::{Label, LabelPrefix};

/// The next label value of each kind prefix (RQ-7), held beside the bindings in
/// `spec/labels.yaml`. Every prefix has a counter; a counter never rewinds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Counters(BTreeMap<LabelPrefix, NonZeroU64>);

impl Default for Counters {
    /// Every counter at its first value, 1.
    fn default() -> Self {
        Self(
            LabelPrefix::ALL
                .into_iter()
                .map(|prefix| (prefix, NonZeroU64::MIN))
                .collect(),
        )
    }
}

impl Counters {
    /// The value the next label of `prefix` receives.
    #[must_use]
    pub fn next(&self, prefix: LabelPrefix) -> NonZeroU64 {
        self.0.get(&prefix).copied().unwrap_or(NonZeroU64::MIN)
    }

    /// Counters one above the highest live, alias or withdrawn label of each prefix, and 1 for a
    /// prefix no label carries (RQ-176, AC-702).
    #[must_use]
    pub fn above(bindings: &Bindings) -> Self {
        let mut counters = Self::default();
        counters.raise_all(bindings.iter().flat_map(Binding::every_label));
        counters
    }

    /// Whether `label` lies below its prefix's counter, so that it was allocated at some point;
    /// a label-shaped text at or above the counter is not a reference (AC-890, AC-891).
    #[must_use]
    pub fn has_allocated(&self, label: Label) -> bool {
        label.number() < self.next(label.prefix())
    }

    /// Sets the next value of `prefix`, as `spec/labels.yaml` states it (AC-23).
    pub fn set(&mut self, prefix: LabelPrefix, next: NonZeroU64) {
        self.0.insert(prefix, next);
    }

    /// Raises each counter above every label of its prefix in `labels`.
    pub fn raise_all(&mut self, labels: impl IntoIterator<Item = Label>) {
        for label in labels {
            self.raise(label);
        }
    }

    /// Raises the counter of `label`'s prefix above `label`, so the value is never allocated
    /// again; a counter already above it is unchanged.
    pub fn raise(&mut self, label: Label) {
        let next = label.number().saturating_add(1);
        let counter = self.0.entry(label.prefix()).or_insert(NonZeroU64::MIN);
        *counter = (*counter).max(next);
    }
}
