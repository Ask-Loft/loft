//! Lexicons: each entry a phrase of tokens, matched as a run of a text's tokens. A configuration
//! may replace a lexicon, or extend and reduce it (AC-375, AC-424).

use std::collections::BTreeSet;

use crate::text::tokenize;

/// The phrases of one lexicon, each as its tokens, sorted.
pub type Lexicon = Vec<Vec<String>>;

/// How a configuration edits a lexicon.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct Edits {
    pub replace: Option<Vec<String>>,
    pub extend: Vec<String>,
    pub remove: Vec<String>,
}

/// The lexicon `defaults` becomes under `edits`.
///
/// # Errors
///
/// A phrase without a token, a phrase twice, `replace` with `extend` or `remove`, a phrase both
/// added and removed, a removal of an absent phrase, or an extension by a present one.
pub(super) fn effective(defaults: &[&str], edits: Option<&Edits>) -> Result<Lexicon, String> {
    let Some(edits) = edits else {
        return compile(defaults.iter().copied()).map(|set| set.into_iter().collect());
    };
    if let Some(replace) = &edits.replace {
        if !edits.extend.is_empty() || !edits.remove.is_empty() {
            return Err("`replace` cannot be combined with `extend` or `remove`".to_owned());
        }
        return compile(replace.iter().map(String::as_str)).map(|set| set.into_iter().collect());
    }
    let mut lexicon = compile(defaults.iter().copied())?;
    let removals = compile(edits.remove.iter().map(String::as_str))?;
    let extensions = compile(edits.extend.iter().map(String::as_str))?;
    if let Some(both) = removals.intersection(&extensions).next() {
        return Err(format!("`{}` is both extended and removed", both.join(" ")));
    }
    for removal in removals {
        if !lexicon.remove(&removal) {
            return Err(format!("`{}` is not in the lexicon", removal.join(" ")));
        }
    }
    for extension in extensions {
        let text = extension.join(" ");
        if !lexicon.insert(extension) {
            return Err(format!("`{text}` is already in the lexicon"));
        }
    }
    Ok(lexicon.into_iter().collect())
}

fn compile<'a>(
    phrases: impl IntoIterator<Item = &'a str>,
) -> Result<BTreeSet<Vec<String>>, String> {
    let mut compiled = BTreeSet::new();
    for phrase in phrases {
        let tokens = tokenize(phrase);
        if tokens.is_empty() {
            return Err(format!("`{phrase}` has no word"));
        }
        if !compiled.insert(tokens) {
            return Err(format!("`{phrase}` is listed twice"));
        }
    }
    Ok(compiled)
}
