//! The bindings between keys, labels and aliases (D-11 F-2), held in `spec/labels.yaml`.

use std::collections::BTreeMap;

use crate::identity::{Key, Label};

/// A key's live label and every label it held before, in the order they were replaced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
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
    #[error("key {0} has no label to replace")]
    Unbound(Key),
    #[error("key {0} already has a label; a label changes only by relabelling")]
    AlreadyBound(Key),
    #[error("alias {alias} of key {alias_key} is the live label of key {label_key}")]
    AliasIsLiveLabel {
        alias: Label,
        alias_key: Key,
        label_key: Key,
    },
    #[error("alias {alias} is listed under key {first} and key {second}")]
    AliasUnderTwoKeys {
        alias: Label,
        first: Key,
        second: Key,
    },
}

impl Bindings {
    /// Gives an unbound key its first label. A bound key changes label only through
    /// [`Bindings::relabel`], which keeps the previous label as an alias.
    pub fn bind(&mut self, key: Key, label: Label) -> Result<(), BindingError> {
        match self.entries.entry(key) {
            std::collections::btree_map::Entry::Occupied(_) => Err(BindingError::AlreadyBound(key)),
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Binding {
                    label,
                    aliases: Vec::new(),
                });
                Ok(())
            }
        }
    }

    /// Replaces a key's live label and records the previous one as a permanent alias (RQ-9).
    pub fn relabel(&mut self, key: Key, label: Label) -> Result<(), BindingError> {
        let binding = self
            .entries
            .get_mut(&key)
            .ok_or(BindingError::Unbound(key))?;
        if binding.label != label {
            let previous = std::mem::replace(&mut binding.label, label);
            binding.aliases.push(previous);
        }
        Ok(())
    }

    /// Appends an alias to a bound key, as a bindings file read from disk may list it.
    pub fn add_alias(&mut self, key: Key, alias: Label) -> Result<(), BindingError> {
        let binding = self
            .entries
            .get_mut(&key)
            .ok_or(BindingError::Unbound(key))?;
        binding.aliases.push(alias);
        Ok(())
    }

    #[must_use]
    pub fn label_of(&self, key: Key) -> Option<Label> {
        self.entries.get(&key).map(|binding| binding.label)
    }

    #[must_use]
    pub fn aliases_of(&self, key: Key) -> &[Label] {
        self.entries
            .get(&key)
            .map_or(&[], |binding| binding.aliases.as_slice())
    }

    /// The key a live label or an alias names (AC-29).
    #[must_use]
    pub fn resolve(&self, label: Label) -> Option<Key> {
        self.entries
            .iter()
            .find(|(_, binding)| binding.label == label)
            .or_else(|| {
                self.entries
                    .iter()
                    .find(|(_, binding)| binding.aliases.contains(&label))
            })
            .map(|(key, _)| *key)
    }

    /// Every binding, in key order.
    pub fn iter(&self) -> impl Iterator<Item = (Key, &Binding)> {
        self.entries.iter().map(|(key, binding)| (*key, binding))
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// The alias rules the validator applies (RQ-10, RQ-66), each error once, in key order.
    #[must_use]
    pub fn validate(&self) -> Vec<BindingError> {
        let live: BTreeMap<Label, Key> = self
            .iter()
            .map(|(key, binding)| (binding.label, key))
            .collect();
        let mut first_owner: BTreeMap<Label, Key> = BTreeMap::new();
        let mut errors = Vec::new();
        for (key, alias) in self.aliases() {
            if let Some(owner) = live.get(&alias).filter(|owner| **owner != key) {
                errors.push(BindingError::AliasIsLiveLabel {
                    alias,
                    alias_key: key,
                    label_key: *owner,
                });
            }
            let first = *first_owner.entry(alias).or_insert(key);
            if first != key {
                errors.push(BindingError::AliasUnderTwoKeys {
                    alias,
                    first,
                    second: key,
                });
            }
        }
        errors
    }

    fn aliases(&self) -> impl Iterator<Item = (Key, Label)> {
        self.iter()
            .flat_map(|(key, binding)| binding.aliases.iter().map(move |alias| (key, *alias)))
    }
}
