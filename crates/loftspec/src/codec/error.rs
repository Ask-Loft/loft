//! A read error with the file and the positions it concerns (RQ-158).

use std::convert::Infallible;
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

/// Writes the sites an error or a finding concerns before its message, each followed by `: `:
/// a site as `<path>:<line>:<column>`, except that a site in the file of the site before it
/// names only its line and column, after a comma (AC-251). Where a subject has no site, its
/// `Err` is written in the site's place.
///
/// # Errors
///
/// The formatter's.
pub fn write_sites<'a, D: fmt::Display>(
    formatter: &mut fmt::Formatter<'_>,
    sites: impl IntoIterator<Item = Result<&'a Site, D>>,
) -> fmt::Result {
    let mut previous: Option<&Site> = None;
    let mut any = false;
    for site in sites {
        let here = site.as_ref().ok().copied();
        match site {
            Ok(site) => match (previous, site.location) {
                (Some(before), Some(location))
                    if before.path == site.path && before.location.is_some() =>
                {
                    write!(formatter, ", {location}")?;
                }
                _ if any => write!(formatter, ": {site}")?,
                _ => write!(formatter, "{site}")?,
            },
            Err(other) if any => write!(formatter, ": {other}")?,
            Err(other) => write!(formatter, "{other}")?,
        }
        previous = here;
        any = true;
    }
    if any {
        formatter.write_str(": ")?;
    }
    Ok(())
}

/// A read error: what is wrong, and every site it concerns (AC-679).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub struct ReadError {
    pub sites: Vec<Site>,
    pub problem: Problem,
}

impl fmt::Display for ReadError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write_sites(formatter, self.sites.iter().map(Ok::<_, Infallible>))?;
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
#[non_exhaustive]
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
    /// A key carried twice that a `record` conflict names: the merge driver, which runs per
    /// file, kept the record in the file one branch moved it from, because the other changed it.
    #[error(
        "the key {0} is carried twice: the record was moved in one branch and changed in the other"
    )]
    MovedAndChanged(Key),
    #[error("{0} is listed without a file")]
    ListedWithoutFile(Key),
    #[error("no {0} lists this file's key")]
    NotListed(&'static str),
    #[error("the key {0} is listed twice")]
    ListedTwice(Key),
    #[error("the Feature {feature} is listed by two Feature Groups, `{first}` and `{second}`")]
    ListedByTwoGroups {
        feature: Key,
        first: String,
        second: String,
    },
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

impl Problem {
    /// The kind of the problem, one name for each variant, which stays the same from release to
    /// release and never depends on the values the message quotes, so a report can count
    /// problems by kind.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::Syntax(_) => "syntax",
            Self::UnknownField(_) => "unknown-field",
            Self::MissingField(_) => "missing-field",
            Self::Expected(_) => "expected",
            Self::Invalid { .. } => "invalid",
            Self::Shape(_) => "shape",
            Self::UnknownFile => "unknown-file",
            Self::MissingManifest => "missing-manifest",
            Self::Unsupported(_) => "unsupported",
            Self::SlugMismatch { .. } => "slug-mismatch",
            Self::DuplicateKey(_) => "duplicate-key",
            Self::MovedAndChanged(_) => "moved-and-changed",
            Self::ListedWithoutFile(_) => "listed-without-file",
            Self::NotListed(_) => "not-listed",
            Self::ListedTwice(_) => "listed-twice",
            Self::ListedByTwoGroups { .. } => "listed-by-two-groups",
            Self::WrongDirectory(_) => "wrong-directory",
            Self::Model(_) => "model",
            Self::ByName => "by-name",
            Self::NameUnresolved { .. } => "name-unresolved",
            Self::NameAmbiguous { .. } => "name-ambiguous",
        }
    }
}
