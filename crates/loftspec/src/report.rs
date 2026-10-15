//! A finding a conversion or a projection reports about the text it reads or writes: its rule,
//! the file and 1-based line it concerns where it has one, the key of the node it concerns where
//! there is one, and a detail such as a count. Every such finding has the severity info; none
//! stops the conversion or the projection.

use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::identity::Key;
use crate::lint::Severity;

/// A rule an [`InfoFinding`] follows: its code and what a finding of it says.
pub trait InfoRule: Copy {
    /// The rule's code, such as `OS-001`.
    fn code(self) -> &'static str;
    /// What a finding of the rule says, before its detail.
    fn message(self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InfoFinding<R> {
    pub rule: R,
    pub file: Option<String>,
    pub line: Option<usize>,
    pub key: Option<Key>,
    pub detail: Option<String>,
}

impl<R> InfoFinding<R> {
    /// A finding about the line `line` of `file`.
    pub const fn at(rule: R, file: String, line: usize, key: Option<Key>) -> Self {
        Self {
            rule,
            file: Some(file),
            line: Some(line),
            key,
            detail: None,
        }
    }

    /// A finding about no file, with a detail.
    pub const fn about(rule: R, detail: String) -> Self {
        Self {
            rule,
            file: None,
            line: None,
            key: None,
            detail: Some(detail),
        }
    }

    /// The finding with its file named by its path under `dir`, without `.` components.
    #[must_use]
    pub fn under(self, dir: &Path) -> Self {
        let file = self.file.map(|file| {
            let path: PathBuf = dir
                .join(file)
                .components()
                .filter(|part| *part != Component::CurDir)
                .collect();
            path.to_string_lossy().into_owned()
        });
        Self { file, ..self }
    }

    /// The finding with `detail`.
    #[must_use]
    pub fn with_detail(self, detail: Option<String>) -> Self {
        Self { detail, ..self }
    }
}

impl<R: InfoRule> fmt::Display for InfoFinding<R> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(file) = &self.file {
            write!(formatter, "{file}:")?;
            if let Some(line) = self.line {
                write!(formatter, "{line}:")?;
            }
            formatter.write_str(" ")?;
        }
        write!(
            formatter,
            "{} {}: {}",
            Severity::Info.as_str(),
            self.rule.code(),
            self.rule.message()
        )?;
        if let Some(detail) = &self.detail {
            write!(formatter, ": {detail}")?;
        }
        Ok(())
    }
}
