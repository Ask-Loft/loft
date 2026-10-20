//! `loft check` as a function (F-38): the tree read, then validated, then linted. A read
//! error stops it before the validator; an error in `spec/lint.yaml` stops it before the linter.
//! Every error and finding names the sites of the elements it concerns (RQ-158). With a base
//! tree, [`check_against`] also splits the findings into those the tree introduced and those it
//! kept, as [`lint_against`] does for two documents held in memory.

mod baseline;

use std::convert::Infallible;
use std::fmt;

use crate::codec::{
    Files, LABELS, LINT, MANIFEST, ReadError, Site, Sites, SpecPath, read_sited, write_sites,
};
use crate::conflict::ConflictOwner;
use crate::document::Document;
use crate::hash::{DocumentHashes, hash_document};
use crate::lint::{Finding, LintConfig, lint};
use crate::validate::{ValidationError, conflict_errors, index_errors, validate};

pub use baseline::{
    Against, Baselined, Change, LintConfigs, Split, check_against, lint_against, split,
};

/// A validation error placed at the sites of the elements it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub error: ValidationError,
    pub sites: Vec<Site>,
}

impl fmt::Display for Placed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_sites(formatter, self.sites.iter().map(Ok::<_, Infallible>))?;
        self.error.fmt(formatter)
    }
}

/// What checking a tree finds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Checked {
    /// The tree does not read; nothing else ran (RQ-137).
    Unread(Vec<ReadError>),
    /// The tree reads: its validation errors, the errors of its lint configuration, and the
    /// findings, which are none when the configuration is in error (AC-674).
    Read {
        invalid: Vec<Placed>,
        config: Vec<ReadError>,
        findings: Vec<Finding>,
    },
}

impl Checked {
    /// Whether the check found an error, as opposed to findings alone.
    #[must_use]
    pub fn has_errors(&self) -> bool {
        match self {
            Self::Unread(_) => true,
            Self::Read {
                invalid, config, ..
            } => !invalid.is_empty() || !config.is_empty(),
        }
    }

    /// The findings, which a tree that does not read has none of.
    #[must_use]
    pub fn findings(&self) -> &[Finding] {
        match self {
            Self::Unread(_) => &[],
            Self::Read { findings, .. } => findings,
        }
    }
}

/// Checks a `spec/` tree.
#[must_use]
pub fn check(files: &Files) -> Checked {
    checked_under(files, lint_config(files), None).0
}

/// Checks a `spec/` tree under `config`, its lint configuration as read, and splits its findings
/// against those of `base` where one is given, the tree and its configuration reading.
fn checked_under(
    files: &Files,
    config: Result<LintConfig, Vec<ReadError>>,
    base: Option<(&Document, &Sites)>,
) -> (Checked, Option<Split>) {
    let (document, sites) = match read_sited(files) {
        Ok(read) => read,
        Err(errors) => return (Checked::Unread(errors), None),
    };
    let invalid = placed(validate(&document), &sites);
    let (config, findings, split) = match (config, base) {
        (Ok(config), None) => (Vec::new(), lint(&document, &sites, &config), None),
        (Ok(config), Some((base, base_sites))) => {
            let against = lint_against(&document, &sites, base, base_sites, &config);
            (Vec::new(), against.findings, Some(against.split))
        }
        (Err(errors), _) => (errors, Vec::new(), None),
    };
    let checked = Checked::Read {
        invalid,
        config,
        findings,
    };
    (checked, split)
}

/// The lint configuration `spec/lint.yaml` gives, or the defaults without the file.
fn lint_config(files: &Files) -> Result<LintConfig, Vec<ReadError>> {
    LintConfig::read(files.get(&SpecPath::new(LINT)).map(Vec::as_slice))
}

/// The hashes of a document read from a tree with the sites `sites`.
///
/// # Errors
///
/// The document holds a merge conflict, or does not index, so it has no hashes: each error at
/// the sites of the elements it names.
pub fn hashed(document: &Document, sites: &Sites) -> Result<DocumentHashes, Vec<Placed>> {
    conflicted(document, sites)?;
    hash_document(document).map_err(|errors| placed(index_errors(errors), sites))
}

/// Refuses a document that holds a merge conflict.
///
/// # Errors
///
/// Each conflict the document holds, at the site of the record that holds it.
pub(crate) fn conflicted(document: &Document, sites: &Sites) -> Result<(), Vec<Placed>> {
    let conflicts = conflict_errors(document);
    if conflicts.is_empty() {
        Ok(())
    } else {
        Err(placed(conflicts, sites))
    }
}

/// Validation errors, each placed at the sites of the elements it names.
pub(crate) fn placed(errors: Vec<ValidationError>, sites: &Sites) -> Vec<Placed> {
    errors
        .into_iter()
        .map(|error| place(error, sites))
        .collect()
}

fn place(error: ValidationError, sites: &Sites) -> Placed {
    let file = match &error {
        ValidationError::Conflict {
            owner: ConflictOwner::Manifest,
            ..
        } => Some(MANIFEST),
        ValidationError::Conflict {
            owner: ConflictOwner::Labels,
            ..
        } => Some(LABELS),
        _ => None,
    };
    let sites = file
        .map(|path| Site {
            path: SpecPath::new(path),
            location: None,
        })
        .into_iter()
        .chain(
            error
                .keys()
                .into_iter()
                .filter_map(|key| sites.get(key).cloned()),
        )
        .collect();
    Placed { error, sites }
}
