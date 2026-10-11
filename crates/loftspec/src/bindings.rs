//! The bindings between elements, labels and aliases (D-11 F-2), held in `spec/labels.yaml`.

use std::collections::BTreeMap;
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

/// The bindings of a document, sorted by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    entries: BTreeMap<Key, Binding>,
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

    /// The element a live label or an alias names (AC-29).
    #[must_use]
    pub fn resolve(&self, label: Label) -> Option<ElementId> {
        let bindings = || self.entries.values();
        bindings()
            .find(|binding| binding.label == label)
            .or_else(|| bindings().find(|binding| binding.aliases.contains(&label)))
            .map(|binding| binding.element)
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
        let labels = bindings.iter().flat_map(|binding| {
            std::iter::once(binding.label).chain(binding.aliases.iter().copied())
        });
        for label in labels {
            let next = label.number().saturating_add(1);
            let counter = counters.0.entry(label.prefix()).or_insert(NonZeroU64::MIN);
            *counter = (*counter).max(next);
        }
        counters
    }
}
