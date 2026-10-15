//! `loft import v1` as a function (F-37): a v1 export converted into a `spec/` tree, with
//! the label counters taken from a counters file (RQ-131, RQ-132, RQ-173) or set one above the
//! highest label seen (RQ-133), and the conversion findings and the sidecar beside it.

use crate::convert::SidecarFile;
use crate::convert::v1::SIDECAR;
use std::fmt;
use std::num::NonZeroU64;

use crate::codec::{self, Files};
use crate::convert::v1::export::Export;
use crate::convert::v1::{ConvertError, Finding, convert};
use crate::identity::{Key, LabelPrefix};
use crate::lint::{RuleId, Severity};
use crate::yaml::{self, Value};

/// A conversion finding as a line of the lint report (AC-509): the key of the element it is
/// about, the lint rule it anticipates where there is one, its severity and its message.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reported {
    pub key: Key,
    pub rule: Option<RuleId>,
    pub severity: Severity,
    pub message: String,
    pub term: Option<String>,
    pub body_line: Option<usize>,
}

impl Reported {
    /// The finding as the lint rule that would report it on the written tree.
    #[must_use]
    pub fn of(finding: &Finding) -> Self {
        let plain = |key: Key, rule, severity, message: &str| Self {
            key,
            rule,
            severity,
            message: message.to_owned(),
            term: None,
            body_line: None,
        };
        match finding {
            Finding::StatementAsText { requirement } => plain(
                requirement.key(),
                Some(RuleId::V2002),
                Severity::Info,
                "The statement is stored as text, without slots.",
            ),
            Finding::UnresolvedReference { node, reference } => Self {
                term: Some(reference.to_string()),
                ..plain(
                    node.key(),
                    Some(RuleId::V2003),
                    Severity::Warning,
                    "The reference names no element.",
                )
            },
            Finding::HeadingInBody { node, line } => Self {
                body_line: Some(*line),
                ..plain(
                    node.key(),
                    Some(RuleId::V2001),
                    Severity::Info,
                    "The body holds a heading; use a Section.",
                )
            },
            Finding::InternalUnderExternal {
                component,
                ancestor,
            } => plain(
                component.key(),
                None,
                Severity::Info,
                &format!(
                    "v1 scopes this Component internally beneath the external Component \
                     {ancestor}; it now takes that Component's boundary."
                ),
            ),
        }
    }
}

/// A converted tree ready to write: its files, the sidecar and what the converter
/// reported.
#[derive(Debug)]
pub struct Imported {
    pub files: Files,
    pub sidecar: SidecarFile,
    pub findings: Vec<Finding>,
    /// The prefixes whose counters were set one above the highest label seen, which the source
    /// may have exceeded (AC-516); empty when a counters file named every prefix.
    pub derived: Vec<LabelPrefix>,
}

/// Why an import writes nothing.
#[derive(Debug, thiserror::Error)]
pub enum ImportError {
    /// The export is not a v1 export (AC-520).
    #[error("{0}")]
    Export(String),
    /// The export is a v1 export the converter refuses.
    #[error(transparent)]
    Convert(#[from] ConvertError),
    /// The counters file is not a mapping of label prefixes to positive integers (RQ-173).
    #[error("{0}")]
    Counters(CountersError),
    /// A counter that is not above the highest label seen for its prefix (AC-513).
    #[error("the counter {value} for `{}` is not above the highest label seen, {highest}", prefix.as_str())]
    CounterTooLow {
        prefix: LabelPrefix,
        value: NonZeroU64,
        highest: u64,
    },
}

/// What is wrong with a counters file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CountersError {
    Yaml(yaml::Error),
    NotAMapping,
    /// A key that is not one of the Format's label prefixes (AC-669).
    UnknownPrefix(String),
    /// A value that is not a positive integer, with its prefix (AC-670).
    NotPositive {
        prefix: String,
        value: String,
    },
}

impl fmt::Display for CountersError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Yaml(error) => error.fmt(formatter),
            Self::NotAMapping => formatter.write_str("the counters are not a mapping"),
            Self::UnknownPrefix(prefix) => {
                write!(formatter, "`{prefix}` is not a label prefix of the Format")
            }
            Self::NotPositive { prefix, value } => {
                write!(
                    formatter,
                    "the counter for `{prefix}` is `{value}`, not a positive integer"
                )
            }
        }
    }
}

/// Reads a counters file: a YAML mapping of label prefix to next value (AC-510).
///
/// # Errors
///
/// A file outside the YAML subset, not a mapping, a key that is not a prefix, or a value that is
/// not a positive integer.
pub fn read_counters(bytes: &[u8]) -> Result<Vec<(LabelPrefix, NonZeroU64)>, CountersError> {
    let root = yaml::read(bytes).map_err(CountersError::Yaml)?;
    let Some(root) = root else {
        return Ok(Vec::new());
    };
    let Value::Mapping(entries) = root.value else {
        return Err(CountersError::NotAMapping);
    };
    entries
        .into_iter()
        .map(|entry| {
            let prefix = LabelPrefix::parse(&entry.key)
                .ok_or_else(|| CountersError::UnknownPrefix(entry.key.clone()))?;
            let text = match &entry.value.value {
                Value::Scalar(scalar) => scalar.text.clone(),
                _ => String::new(),
            };
            let value = text
                .parse::<NonZeroU64>()
                .ok()
                .filter(|_| text.bytes().all(|byte| byte.is_ascii_digit()))
                .ok_or(CountersError::NotPositive {
                    prefix: entry.key,
                    value: text,
                })?;
            Ok((prefix, value))
        })
        .collect()
}

/// Converts the export text into a `spec/` tree.
///
/// # Errors
///
/// An export that is not a v1 export, one the converter refuses, a counters file that does not
/// read, or a counter not above the highest label seen.
pub fn import(export: &str, counters: Option<&[u8]>) -> Result<Imported, ImportError> {
    let export =
        Export::from_json(export).map_err(|error| ImportError::Export(error.to_string()))?;
    let given = counters
        .map(read_counters)
        .transpose()
        .map_err(ImportError::Counters)?
        .unwrap_or_default();
    let mut conversion = convert(&export)?;
    let seen = conversion.document.counters.clone();
    for &(prefix, value) in &given {
        let next = seen.next(prefix);
        if value < next {
            return Err(ImportError::CounterTooLow {
                prefix,
                value,
                highest: next.get() - 1,
            });
        }
        conversion.document.counters.set(prefix, value);
    }
    let derived = LabelPrefix::ALL
        .into_iter()
        .filter(|prefix| !given.iter().any(|(named, _)| named == prefix))
        .collect();
    Ok(Imported {
        files: codec::write(&conversion.document),
        sidecar: SidecarFile {
            name: SIDECAR,
            bytes: conversion.sidecar.to_json().into_bytes(),
        },
        findings: conversion.findings,
        derived,
    })
}
