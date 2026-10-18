//! Groups: named sets of words, the option kind of the term report's noun groups (`TD-001`) and
//! abbreviation pairs (`TD-003`). A configuration may replace the groups, or extend and reduce
//! them: `extend` adds words to a named group, creating it, and `remove` takes words from a group,
//! or the whole group when it names no words.

use std::collections::{BTreeMap, BTreeSet};

use crate::text::tokenize;

/// Each group's words by its name, each word one token in lower case.
pub type Groups = BTreeMap<String, BTreeSet<String>>;

/// A group as a configuration writes it: its name and its words; `remove` may leave the words out.
pub(super) type Written = (String, Option<Vec<String>>);

/// How a configuration edits groups.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Edits {
    pub replace: Option<Vec<Written>>,
    pub extend: Vec<Written>,
    pub remove: Vec<Written>,
}

/// The groups `defaults` become under `edits`.
///
/// # Errors
///
/// A word that is not one word, `replace` with `extend` or `remove`, a group without words where
/// words are needed, a removal of an absent group or word, or an extension by a present word.
pub(super) fn effective(
    defaults: &[(&str, &[&str])],
    edits: Option<&Edits>,
) -> Result<Groups, String> {
    let written = |(name, words): (&&str, &&[&str])| {
        (
            (*name).to_owned(),
            Some(words.iter().map(|w| (*w).to_owned()).collect()),
        )
    };
    let defaults: Vec<Written> = defaults.iter().map(|(n, w)| written((n, w))).collect();
    let Some(edits) = edits else {
        return build(&defaults);
    };
    if let Some(replace) = &edits.replace {
        if !edits.extend.is_empty() || !edits.remove.is_empty() {
            return Err("`replace` cannot be combined with `extend` or `remove`".to_owned());
        }
        return build(replace);
    }
    let mut groups = build(&defaults)?;
    for (name, words) in &edits.remove {
        let Some(group) = groups.get_mut(name) else {
            return Err(format!("`{name}` is not a group"));
        };
        let Some(words) = words else {
            groups.remove(name);
            continue;
        };
        for word in compile(words)? {
            if !group.remove(&word) {
                return Err(format!("`{word}` is not in the group `{name}`"));
            }
        }
    }
    for (name, words) in &edits.extend {
        let words = words
            .as_deref()
            .ok_or(format!("`{name}` lists no words to add"))?;
        let group = groups.entry(name.clone()).or_default();
        for word in compile(words)? {
            if !group.insert(word.clone()) {
                return Err(format!("`{word}` is already in the group `{name}`"));
            }
        }
    }
    Ok(groups)
}

fn build(written: &[Written]) -> Result<Groups, String> {
    let mut groups = Groups::new();
    for (name, words) in written {
        let words = words.as_deref().ok_or(format!("`{name}` lists no words"))?;
        if groups.insert(name.clone(), compile(words)?).is_some() {
            return Err(format!("`{name}` is listed twice"));
        }
    }
    Ok(groups)
}

/// Each word in lower case; an error for one that is not a single word.
fn compile(words: &[String]) -> Result<BTreeSet<String>, String> {
    words
        .iter()
        .map(|word| match tokenize(word).as_slice() {
            [token] => Ok(token.clone()),
            _ => Err(format!("`{word}` is not one word")),
        })
        .collect()
}
