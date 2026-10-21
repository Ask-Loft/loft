//! The label and alias rules the validator applies to a document's bindings (RQ-70, RQ-10,
//! RQ-66).

use std::collections::BTreeMap;

use super::{BindingError, Bindings};
use crate::identity::Label;
use crate::ids::ElementId;

impl Bindings {
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
