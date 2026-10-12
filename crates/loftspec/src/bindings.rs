//! The bindings between elements, labels and aliases (D-11 F-2), held in `spec/labels.yaml`.

use std::collections::{BTreeMap, BTreeSet};
use std::num::NonZeroU64;

use crate::identity::{Key, Label, LabelPrefix};
use crate::ids::ElementId;

/// An element's live label and every label it held before, in the order they were replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub element: ElementId,
    pub label: Label,
    pub aliases: Vec<Label>,
}

impl Binding {
    /// The live label, then the aliases.
    pub fn labels(&self) -> impl Iterator<Item = Label> + '_ {
        std::iter::once(self.label).chain(self.aliases.iter().copied())
    }
}

/// The bindings of a document, sorted by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    entries: BTreeMap<Key, Binding>,
    /// The keys whose live label each label is; more than one only in bindings that break RQ-70.
    live_keys: BTreeMap<Label, BTreeSet<Key>>,
    /// The keys listing each label as an alias.
    alias_keys: BTreeMap<Label, BTreeSet<Key>>,
}

/// Records `key` under `label` in a label index.
fn index(keys: &mut BTreeMap<Label, BTreeSet<Key>>, label: Label, key: Key) {
    keys.entry(label).or_default().insert(key);
}

/// Removes `key` from under `label` in a label index.
fn unindex(keys: &mut BTreeMap<Label, BTreeSet<Key>>, label: Label, key: Key) {
    if let Some(holders) = keys.get_mut(&label) {
        holders.remove(&key);
        if holders.is_empty() {
            keys.remove(&label);
        }
    }
}

/// A binding rule a set of bindings breaks.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    #[error("{0} has no label to replace")]
    Unbound(ElementId),
    #[error("{0} already has a label; a label changes only by relabelling")]
    AlreadyBound(ElementId),
    #[error("{element} cannot carry label {label}: its kind takes another prefix or none")]
    PrefixMismatch { element: ElementId, label: Label },
    #[error("label {label} is the live label of both {first} and {second}")]
    LabelUnderTwoKeys {
        label: Label,
        first: ElementId,
        second: ElementId,
    },
    #[error("alias {alias} of {alias_element} is the live label of {label_element}")]
    AliasIsLiveLabel {
        alias: Label,
        alias_element: ElementId,
        label_element: ElementId,
    },
    #[error("alias {alias} is listed under {first} and {second}")]
    AliasUnderTwoKeys {
        alias: Label,
        first: ElementId,
        second: ElementId,
    },
}

fn check_prefix(element: ElementId, label: Label) -> Result<(), BindingError> {
    if element.label_prefix() == Some(label.prefix()) {
        Ok(())
    } else {
        Err(BindingError::PrefixMismatch { element, label })
    }
}

impl Bindings {
    /// Gives an unbound element its first label, whose prefix must be its kind's (AC-243). A
    /// bound element changes label only through [`Bindings::relabel`].
    pub fn bind(
        &mut self,
        element: impl Into<ElementId>,
        label: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        check_prefix(element, label)?;
        self.bind_as_written(element, label)
    }

    /// Records a binding as `spec/labels.yaml` holds it. A label whose prefix is no longer its
    /// element's kind prefix is kept, for the next publish to replace (RQ-63); only an element
    /// bound twice is refused.
    pub fn bind_as_written(
        &mut self,
        element: impl Into<ElementId>,
        label: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        match self.entries.entry(element.key()) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(BindingError::AlreadyBound(element))
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Binding {
                    element,
                    label,
                    aliases: Vec::new(),
                });
                index(&mut self.live_keys, label, element.key());
                Ok(())
            }
        }
    }

    /// Replaces an element's live label and records the previous one as a permanent alias
    /// (RQ-9). The element may have changed kind with the same key, as a promoted Requirement
    /// does (RQ-63).
    pub fn relabel(
        &mut self,
        element: impl Into<ElementId>,
        label: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        check_prefix(element, label)?;
        let binding = self
            .entries
            .get_mut(&element.key())
            .ok_or(BindingError::Unbound(element))?;
        binding.element = element;
        if binding.label != label {
            let previous = std::mem::replace(&mut binding.label, label);
            binding.aliases.push(previous);
            let key = element.key();
            unindex(&mut self.live_keys, previous, key);
            index(&mut self.live_keys, label, key);
            index(&mut self.alias_keys, previous, key);
        }
        Ok(())
    }

    /// Appends an alias to a bound element, as a bindings file read from disk may list it.
    pub fn add_alias(
        &mut self,
        element: impl Into<ElementId>,
        alias: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        let binding = self
            .entries
            .get_mut(&element.key())
            .ok_or(BindingError::Unbound(element))?;
        binding.aliases.push(alias);
        index(&mut self.alias_keys, alias, element.key());
        Ok(())
    }

    #[must_use]
    pub fn label_of(&self, element: impl Into<ElementId>) -> Option<Label> {
        self.entries
            .get(&element.into().key())
            .map(|binding| binding.label)
    }

    #[must_use]
    pub fn aliases_of(&self, element: impl Into<ElementId>) -> &[Label] {
        self.entries
            .get(&element.into().key())
            .map_or(&[], |binding| binding.aliases.as_slice())
    }

    /// The element a live label or an alias names (AC-29): a live label before an alias, and the
    /// lowest key where bindings that break RQ-70 give a label to several.
    #[must_use]
    pub fn resolve(&self, label: Label) -> Option<ElementId> {
        let first = |keys: &BTreeMap<Label, BTreeSet<Key>>| keys.get(&label)?.first().copied();
        let key = first(&self.live_keys).or_else(|| first(&self.alias_keys))?;
        self.entries.get(&key).map(|binding| binding.element)
    }

    /// Every binding, in key order.
    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.entries.values()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The label and alias rules the validator applies (RQ-70, RQ-10, RQ-66), each error once,
    /// in key order.
    #[must_use]
    pub fn validate(&self) -> Vec<BindingError> {
        let mut live: BTreeMap<Label, ElementId> = BTreeMap::new();
        let mut errors = Vec::new();
        for binding in self.iter() {
            let first = *live.entry(binding.label).or_insert(binding.element);
            if first != binding.element {
                errors.push(BindingError::LabelUnderTwoKeys {
                    label: binding.label,
                    first,
                    second: binding.element,
                });
            }
        }
        let mut first_owner: BTreeMap<Label, ElementId> = BTreeMap::new();
        for (element, alias) in self.aliases() {
            if let Some(owner) = live.get(&alias).filter(|owner| **owner != element) {
                errors.push(BindingError::AliasIsLiveLabel {
                    alias,
                    alias_element: element,
                    label_element: *owner,
                });
            }
            let first = *first_owner.entry(alias).or_insert(element);
            if first != element {
                errors.push(BindingError::AliasUnderTwoKeys {
                    alias,
                    first,
                    second: element,
                });
            }
        }
        errors
    }

    fn aliases(&self) -> impl Iterator<Item = (ElementId, Label)> {
        self.iter().flat_map(|binding| {
            binding
                .aliases
                .iter()
                .map(move |alias| (binding.element, *alias))
        })
    }
}

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

    /// Counters one above the highest live label or alias of each prefix, and 1 for a prefix no
    /// label carries (RQ-176, AC-702).
    #[must_use]
    pub fn above(bindings: &Bindings) -> Self {
        let mut counters = Self::default();
        for label in bindings.iter().flat_map(Binding::labels) {
            counters.raise(label);
        }
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

    /// Raises the counter of `label`'s prefix above `label`, so the value is never allocated
    /// again; a counter already above it is unchanged.
    pub fn raise(&mut self, label: Label) {
        let next = label.number().saturating_add(1);
        let counter = self.0.entry(label.prefix()).or_insert(NonZeroU64::MIN);
        *counter = (*counter).max(next);
    }
}
