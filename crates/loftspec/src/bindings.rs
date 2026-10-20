//! The bindings between elements, labels and aliases (F-2), held in `spec/labels.yaml`.

mod counters;

use std::collections::{BTreeMap, BTreeSet};

pub use counters::Counters;

use crate::canonical::Digest;
use crate::decision::StateKind;
use crate::identity::{Key, Label};
use crate::ids::ElementId;

/// An element's live label, every label it held before in the order they were replaced, and the
/// labels it lost to a publish that met another (RQ-70's withdrawn labels), which name nothing.
/// A departed binding is the binding of a key that left the document: it keeps its labels, and
/// none of them names anything. A decision's binding records the kind of the state its last
/// publish saw, with the decision's history hash; neither record enters a root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    pub element: ElementId,
    pub label: Label,
    pub departed: bool,
    pub aliases: Vec<Label>,
    pub withdrawn: Vec<Label>,
    pub published_state: Option<StateKind>,
    pub published_history: Option<Digest>,
}

impl Binding {
    /// The live label, then the aliases.
    pub fn labels(&self) -> impl Iterator<Item = Label> + '_ {
        std::iter::once(self.label).chain(self.aliases.iter().copied())
    }

    /// Every label the element was ever given: the live label, the aliases and the withdrawn
    /// labels, none of which is allocated again.
    pub fn every_label(&self) -> impl Iterator<Item = Label> + '_ {
        self.labels().chain(self.withdrawn.iter().copied())
    }
}

/// The bindings of a document, sorted by key.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Bindings {
    entries: BTreeMap<Key, Binding>,
    /// The keys whose live label each label is; more than one only in bindings that break RQ-70.
    live_keys: LabelIndex,
    /// The keys listing each label as an alias.
    alias_keys: LabelIndex,
    /// The keys each label was withdrawn from.
    withdrawn_keys: LabelIndex,
}

/// What the next publish does with a citation of a label in an element, given the keys the
/// element mentions: a label withdrawn from exactly one mentioned key becomes that key's live
/// label, unless the element also mentions the label's live holder.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Citation {
    /// No key the element mentions had the label withdrawn.
    NotWithdrawn,
    /// The citation becomes this label.
    Rewritten(Label),
    /// The element also mentions the label's live holder, so the citation stays.
    HolderMentioned,
    /// Two keys the element mentions had the label withdrawn, so the citation stays.
    Ambiguous,
}

/// The keys each label is recorded under, in one way of holding it.
type LabelIndex = BTreeMap<Label, BTreeSet<Key>>;

/// Records `key` under `label` in a label index.
fn index(keys: &mut LabelIndex, label: Label, key: Key) {
    keys.entry(label).or_default().insert(key);
}

/// Removes `key` from under `label` in a label index.
fn unindex(keys: &mut LabelIndex, label: Label, key: Key) {
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
    #[error("label {label} is withdrawn from {element}, which still holds it")]
    WithdrawnIsOwnLabel { element: ElementId, label: Label },
    #[error("label {label}, which departed with {departed}, is held by {holder}")]
    DepartedLabelHeld {
        label: Label,
        departed: ElementId,
        holder: ElementId,
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
    /// element's kind prefix is kept, for the next publish to replace (RQ-63); an element whose
    /// kind takes no prefix, or an element bound twice, is refused.
    pub(crate) fn bind_as_written(
        &mut self,
        element: impl Into<ElementId>,
        label: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        if element.label_prefix().is_none() {
            return Err(BindingError::PrefixMismatch { element, label });
        }
        match self.entries.entry(element.key()) {
            std::collections::btree_map::Entry::Occupied(_) => {
                Err(BindingError::AlreadyBound(element))
            }
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(Binding {
                    element,
                    label,
                    departed: false,
                    aliases: Vec::new(),
                    withdrawn: Vec::new(),
                    published_state: None,
                    published_history: None,
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
        let binding = self.entry_mut(element)?;
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
        let binding = self.entry_mut(element)?;
        binding.aliases.push(alias);
        if !binding.departed {
            index(&mut self.alias_keys, alias, element.key());
        }
        Ok(())
    }

    /// Records a label the element lost to a publish that met another; it resolves to nothing.
    pub fn add_withdrawn(
        &mut self,
        element: impl Into<ElementId>,
        label: Label,
    ) -> Result<(), BindingError> {
        let element = element.into();
        let binding = self.entry_mut(element)?;
        binding.withdrawn.push(label);
        if !binding.departed {
            index(&mut self.withdrawn_keys, label, element.key());
        }
        Ok(())
    }

    /// Marks an element's binding departed: its labels stay with it and name nothing.
    pub fn depart(&mut self, element: impl Into<ElementId>) -> Result<(), BindingError> {
        let element = element.into();
        let binding = self.entry_mut(element)?;
        if binding.departed {
            return Ok(());
        }
        binding.departed = true;
        self.reindex(element.key(), unindex);
        Ok(())
    }

    /// Gives a departed binding back to `element`, the element that carries its key again, perhaps
    /// as another kind: its labels name it once more.
    pub fn restore(&mut self, element: impl Into<ElementId>) -> Result<(), BindingError> {
        let element = element.into();
        let binding = self.entry_mut(element)?;
        if !binding.departed {
            return Ok(());
        }
        binding.departed = false;
        binding.element = element;
        self.reindex(element.key(), index);
        Ok(())
    }

    /// The binding of `element`'s key.
    fn entry_mut(&mut self, element: ElementId) -> Result<&mut Binding, BindingError> {
        (self.entries.get_mut(&element.key())).ok_or(BindingError::Unbound(element))
    }

    /// Records every label of the binding of `key` in its index, or removes it, by `change`.
    fn reindex(&mut self, key: Key, change: fn(&mut LabelIndex, Label, Key)) {
        let binding = &self.entries[&key];
        change(&mut self.live_keys, binding.label, key);
        for alias in &binding.aliases {
            change(&mut self.alias_keys, *alias, key);
        }
        for withdrawn in &binding.withdrawn {
            change(&mut self.withdrawn_keys, *withdrawn, key);
        }
    }

    /// Whether the binding of `key` is departed.
    #[must_use]
    pub fn is_departed(&self, key: Key) -> bool {
        self.entries
            .get(&key)
            .is_some_and(|binding| binding.departed)
    }

    /// Records in an element's binding the kind of the state a publish saw and its history hash.
    pub fn record_published(
        &mut self,
        element: impl Into<ElementId>,
        state: Option<StateKind>,
        history: Option<Digest>,
    ) -> Result<(), BindingError> {
        let binding = self.entry_mut(element.into())?;
        binding.published_state = state;
        binding.published_history = history;
        Ok(())
    }

    /// Removes an element's binding and returns it, freeing its live label and its aliases.
    pub fn unbind(&mut self, element: impl Into<ElementId>) -> Option<Binding> {
        let key = element.into().key();
        if self.entries.contains_key(&key) {
            self.reindex(key, unindex);
        }
        self.entries.remove(&key)
    }

    #[must_use]
    pub fn label_of(&self, element: impl Into<ElementId>) -> Option<Label> {
        self.label_of_key(element.into().key())
    }

    /// The live label of the element whose key is `key`; none where its binding is departed.
    #[must_use]
    pub fn label_of_key(&self, key: Key) -> Option<Label> {
        self.held(key).map(|binding| binding.label)
    }

    #[must_use]
    pub fn aliases_of(&self, element: impl Into<ElementId>) -> &[Label] {
        self.held(element.into().key())
            .map_or(&[], |binding| binding.aliases.as_slice())
    }

    #[must_use]
    pub fn withdrawn_of(&self, element: impl Into<ElementId>) -> &[Label] {
        self.held(element.into().key())
            .map_or(&[], |binding| binding.withdrawn.as_slice())
    }

    /// The binding of `key` where it is not departed.
    fn held(&self, key: Key) -> Option<&Binding> {
        self.entries.get(&key).filter(|binding| !binding.departed)
    }

    /// The element a live label or an alias names (AC-29): a live label before an alias, and the
    /// lowest key where bindings that break RQ-70 give a label to several.
    #[must_use]
    pub fn resolve(&self, label: Label) -> Option<ElementId> {
        let first = |keys: &LabelIndex| keys.get(&label)?.first().copied();
        let key = first(&self.live_keys).or_else(|| first(&self.alias_keys))?;
        self.entries.get(&key).map(|binding| binding.element)
    }

    /// Whether any binding has a withdrawn label.
    #[must_use]
    pub fn has_withdrawn(&self) -> bool {
        !self.withdrawn_keys.is_empty()
    }

    /// Whether `label` was withdrawn from any key.
    #[must_use]
    pub fn is_withdrawn(&self, label: Label) -> bool {
        self.withdrawn_keys.contains_key(&label)
    }

    /// What the next publish does with an unqualified citation of `label` in an element that
    /// mentions the keys `mentioned` accepts.
    #[must_use]
    pub fn citation(&self, label: Label, mentioned: impl Fn(Key) -> bool) -> Citation {
        let from = self.withdrawn_keys.get(&label).into_iter().flatten();
        let mut from = from.copied().filter(|key| mentioned(*key));
        let Some(key) = from.next() else {
            return Citation::NotWithdrawn;
        };
        if from.next().is_some() {
            return Citation::Ambiguous;
        }
        if self
            .resolve(label)
            .is_some_and(|holder| mentioned(holder.key()))
        {
            return Citation::HolderMentioned;
        }
        Citation::Rewritten(self.entries[&key].label)
    }

    /// Every binding, departed or not, in key order.
    pub fn iter(&self) -> impl Iterator<Item = &Binding> {
        self.entries.values()
    }

    /// Every departed binding, in key order.
    pub fn departed(&self) -> impl Iterator<Item = &Binding> {
        self.iter().filter(|binding| binding.departed)
    }

    /// Every binding that is not departed, in key order.
    pub fn live(&self) -> impl Iterator<Item = &Binding> {
        self.iter().filter(|binding| !binding.departed)
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
        for binding in self.live() {
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
        errors.extend(self.live().flat_map(|binding| {
            binding
                .withdrawn
                .iter()
                .filter(|label| binding.labels().any(|held| held == **label))
                .map(|label| BindingError::WithdrawnIsOwnLabel {
                    element: binding.element,
                    label: *label,
                })
        }));
        errors.extend(self.departed_labels_held());
        errors
    }

    /// Each departed label that another key holds, live or as an alias, as an alias is checked.
    fn departed_labels_held(&self) -> impl Iterator<Item = BindingError> + '_ {
        self.departed().flat_map(move |binding| {
            let label = binding.label;
            let live = self.live_keys.get(&label).into_iter().flatten();
            let aliased = self.alias_keys.get(&label).into_iter().flatten();
            live.chain(aliased)
                .filter(move |key| **key != binding.element.key())
                .map(move |key| BindingError::DepartedLabelHeld {
                    label,
                    departed: binding.element,
                    holder: self.entries[key].element,
                })
        })
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
