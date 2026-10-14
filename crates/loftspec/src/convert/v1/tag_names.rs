//! The v2 tag names derived from v1 names: a Facet Tag's name and a Vocabulary Entry's name are
//! free text in v1, and a v2 tag name is a lowercase letter followed by lowercase letters, digits
//! and hyphens (RQ-71).

use std::collections::BTreeMap;

use super::ConvertError;
use crate::names::TagName;

/// The tag names derived so far, each with the v1 name it came from, so that two v1 names
/// deriving one tag name are refused and a Facet Tag and a Vocabulary Entry with one v1 name
/// derive one tag name.
#[derive(Debug, Default)]
pub(super) struct TagNames(BTreeMap<TagName, String>);

impl TagNames {
    /// The tag name derived from `v1_name`, refusing one that another v1 name already derived.
    pub(super) fn derive(&mut self, v1_name: &str) -> Result<TagName, ConvertError> {
        let tag = derive(v1_name).ok_or_else(|| ConvertError::UnderivableTag {
            name: v1_name.to_owned(),
        })?;
        match self.0.get(&tag) {
            Some(first) if first != v1_name => Err(ConvertError::TagCollision {
                first: first.clone(),
                second: v1_name.to_owned(),
                tag,
            }),
            Some(_) => Ok(tag),
            None => {
                self.0.insert(tag.clone(), v1_name.to_owned());
                Ok(tag)
            }
        }
    }

    /// The v1 name behind each derived tag name that differs from it.
    pub(super) fn renamed(self) -> BTreeMap<TagName, String> {
        self.0
            .into_iter()
            .filter(|(tag, v1_name)| tag.as_str() != v1_name)
            .collect()
    }
}

/// A v1 name that is already a tag name, as it is; any other in lowercase, each run of
/// characters other than ASCII lowercase letters and digits one hyphen, with no hyphen at its
/// start or its end. None when that is empty or does not begin with a letter.
fn derive(v1_name: &str) -> Option<TagName> {
    if let Ok(tag) = TagName::parse(v1_name) {
        return Some(tag);
    }
    let mut derived = String::with_capacity(v1_name.len());
    for character in v1_name.to_lowercase().chars() {
        if character.is_ascii_lowercase() || character.is_ascii_digit() {
            derived.push(character);
        } else if !derived.is_empty() && !derived.ends_with('-') {
            derived.push('-');
        }
    }
    TagName::parse(derived.trim_end_matches('-')).ok()
}
