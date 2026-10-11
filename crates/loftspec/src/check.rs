//! `loft check` as a function (D-11 F-38): the tree read, then validated, then linted. A read
//! error stops it before the validator; an error in `spec/lint.yaml` stops it before the linter.
//! Every error and finding names the sites of the elements it concerns (RQ-158).

use std::fmt;

use crate::codec::{Files, LINT, ReadError, Site, Sites, SpecPath, read_sited};
use crate::lint::{Finding, LintConfig, lint};
use crate::validate::{ValidationError, validate};

/// A validation error placed at the sites of the elements it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Placed {
    pub error: ValidationError,
    pub sites: Vec<Site>,
}

impl fmt::Display for Placed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for site in &self.sites {
            write!(formatter, "{site}: ")?;
        }
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
    let (document, sites) = match read_sited(files) {
        Ok(read) => read,
        Err(errors) => return Checked::Unread(errors),
    };
    let invalid = validate(&document)
        .into_iter()
        .map(|error| place(error, &sites))
        .collect();
    let config = LintConfig::read(files.get(&SpecPath::new(LINT)).map(Vec::as_slice));
    let (config, findings) = match config {
        Ok(config) => (Vec::new(), lint(&document, &sites, &config)),
        Err(errors) => (errors, Vec::new()),
    };
    Checked::Read {
        invalid,
        config,
        findings,
    }
}

fn place(error: ValidationError, sites: &Sites) -> Placed {
    let sites = error
        .keys()
        .into_iter()
        .filter_map(|key| sites.get(key).cloned())
        .collect();
    Placed { error, sites }
}
