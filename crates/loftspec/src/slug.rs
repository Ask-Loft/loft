//! The slug that fixes a record's file or directory name (D-11 F-6, RQ-26, AC-245, AC-246): set
//! when the record is created and never changed by the formatter.

use std::collections::BTreeSet;
use std::fmt;

/// Lowercase ASCII letters and digits in runs joined by single hyphens.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Slug(String);

/// Text that is not a slug.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("`{0}` is not a slug: lowercase ASCII letters and digits in runs joined by single hyphens")]
pub struct SlugError(pub String);

/// The longest slug the derive rule writes.
const MAX_LENGTH: usize = 48;

/// Names an operating system reserves for devices, which no file or directory may take.
const RESERVED: [&str; 22] = [
    "con", "prn", "aux", "nul", "com1", "com2", "com3", "com4", "com5", "com6", "com7", "com8",
    "com9", "lpt1", "lpt2", "lpt3", "lpt4", "lpt5", "lpt6", "lpt7", "lpt8", "lpt9",
];

impl Slug {
    /// Reads a slug, refusing text that breaks the syntax.
    ///
    /// # Errors
    ///
    /// The text, when it is not lowercase ASCII letters and digits in runs joined by single
    /// hyphens.
    pub fn parse(text: &str) -> Result<Self, SlugError> {
        let valid = !text.is_empty()
            && text.split('-').all(|run| {
                !run.is_empty()
                    && run
                        .bytes()
                        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit())
            });
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(SlugError(text.to_owned()))
        }
    }

    /// The slug the Format's rule derives from a name, before collisions:
    /// - `A`..`Z` become `a`..`z`, and nothing else is lowercased;
    /// - each run of other characters becomes one hyphen, and the ends are trimmed;
    /// - a slug longer than 48 characters is cut to its longest prefix of at most 48 that a
    ///   hyphen follows, or to 48 when there is none;
    /// - an empty result takes `fallback`, the kind's name;
    /// - a reserved device name takes the suffix `-1`.
    #[must_use]
    pub fn derive(name: &str, fallback: &str) -> Self {
        let mut slug = String::with_capacity(name.len());
        for character in name.chars() {
            match character {
                'a'..='z' | '0'..='9' => slug.push(character),
                'A'..='Z' => slug.push(character.to_ascii_lowercase()),
                _ if !slug.is_empty() && !slug.ends_with('-') => slug.push('-'),
                _ => {}
            }
        }
        let mut slug = slug.trim_end_matches('-').to_owned();
        if slug.len() > MAX_LENGTH {
            let cut = slug[..=MAX_LENGTH].rfind('-').unwrap_or(MAX_LENGTH);
            slug.truncate(cut);
        }
        if slug.is_empty() {
            fallback.clone_into(&mut slug);
        }
        if RESERVED.contains(&slug.as_str()) {
            slug.push_str("-1");
        }
        Self(slug)
    }

    /// This slug, or the first of `-2`, `-3`, … appended to it that `taken` does not hold; the
    /// result is added to `taken`.
    #[must_use]
    pub fn unique_in(self, taken: &mut BTreeSet<Self>) -> Self {
        let mut candidate = self.clone();
        let mut suffix = 2;
        while taken.contains(&candidate) {
            candidate = Self(format!("{}-{suffix}", self.0));
            suffix += 1;
        }
        taken.insert(candidate.clone());
        candidate
    }

    /// Makes the slugs of one directory unique, in order: each slug a later one repeats takes
    /// the first free suffix (AC-246).
    pub fn make_unique<'a>(slugs: impl IntoIterator<Item = &'a mut Self>) {
        let mut taken = BTreeSet::new();
        for slug in slugs {
            *slug = std::mem::replace(slug, Self(String::new())).unique_in(&mut taken);
        }
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Slug {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}
