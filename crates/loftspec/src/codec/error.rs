//! A read error with the file and the positions it concerns (RQ-158).

use std::fmt;

use crate::identity::Key;
use crate::manifest::ManifestError;
use crate::yaml::{self, Location, Position};

/// A path under `spec/`, with `/` between its parts.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct SpecPath(String);

impl SpecPath {
    #[must_use]
    pub fn new(path: &str) -> Self {
        Self(path.to_owned())
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The parts of the path.
    pub fn parts(&self) -> impl Iterator<Item = &str> {
        self.0.split('/')
    }
}

impl fmt::Display for SpecPath {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "spec/{}", self.0)
    }
}

/// A position in one file.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct Site {
    pub path: SpecPath,
    pub location: Option<Location>,
}

impl fmt::Display for Site {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.location {
            Some(location) => write!(formatter, "{}:{location}", self.path),
            None => self.path.fmt(formatter),
        }
    }
}

/// A read error: what is wrong, and every site it concerns (AC-679).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ReadError {
    pub sites: Vec<Site>,
    pub problem: Problem,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for site in &self.sites {
            write!(formatter, "{site}: ")?;
        }
        self.problem.fmt(formatter)
    }
}

impl ReadError {
    pub(crate) fn at(path: &SpecPath, position: Option<Position>, problem: Problem) -> Self {
        Self {
            sites: vec![Site {
                path: path.clone(),
                location: position.map(Location::At),
            }],
            problem,
        }
    }

    pub(crate) fn yaml(path: &SpecPath, error: yaml::Error) -> Self {
        Self {
            sites: vec![Site {
                path: path.clone(),
                location: Some(error.location),
            }],
            problem: Problem::Syntax(error.kind),
        }
    }
}

/// An error in one record, before its file is known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Located {
    pub position: Position,
    pub problem: Problem,
}

impl Located {
    pub(crate) fn into_error(self, path: &SpecPath) -> ReadError {
        ReadError::at(path, Some(self.position), self.problem)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Problem {
    #[error("{0}")]
    Syntax(yaml::ErrorKind),
    #[error("the field `{0}` is not one the Format defines here")]
    UnknownField(String),
    #[error("the field `{0}` is required")]
    MissingField(&'static str),
    #[error("expected {0}")]
    Expected(&'static str),
    #[error("`{value}` is not {expected}")]
    Invalid {
        value: String,
        expected: &'static str,
    },
    #[error("{0}")]
    Shape(String),
    #[error("a file the Format does not name")]
    UnknownFile,
    #[error("the manifest is missing")]
    MissingManifest,
    #[error("{0}")]
    Unsupported(ManifestError),
    #[error("the slug `{slug}` differs from the name `{name}` it fixes")]
    SlugMismatch { slug: String, name: String },
    #[error("the key {0} is carried twice")]
    DuplicateKey(Key),
    #[error("{0} is listed without a file")]
    ListedWithoutFile(Key),
    #[error("no {0} lists this file's key")]
    NotListed(&'static str),
    #[error("the key {0} is listed twice")]
    ListedTwice(Key),
    #[error("the Feature {0} is listed by two Feature Groups")]
    ListedByTwoGroups(Key),
    #[error("the Feature file is not in the directory of its Feature Group `{0}`")]
    WrongDirectory(String),
    #[error("{0}")]
    Model(String),
    #[error("a reference written by name, which `loft fmt` resolves to a key")]
    ByName,
    #[error("no {wanted} is named \"{name}\"")]
    NameUnresolved { name: String, wanted: &'static str },
    #[error("\"{name}\" names more than one element: {}", keys.join(", "))]
    NameAmbiguous { name: String, keys: Vec<String> },
}
