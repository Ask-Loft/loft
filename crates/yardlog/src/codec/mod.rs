//! Yard Log as files: `yardlog.yaml` and one `<stage key>.yaml` for each stage, in the strict
//! YAML that `loft` writes for `spec/`, laid out canonically, so that Yard Log holding the same
//! records always gives the same bytes and a change to one block rewrites only its stage's file
//! and the root.

mod acts;
mod bodies;
mod partial;
mod reader;
mod root;
pub mod session;
mod stage;

use std::collections::BTreeSet;
use std::fmt;

use loft_common::yaml::{self, Node, Position, Value};

pub use partial::Partial;
pub use reader::ReadError;

use crate::format::Format;
use crate::keys::{Key, StageKey};
use crate::log::{StageFile, YardLog};

/// The root's file name in Yard Log's folder.
pub const ROOT: &str = "yardlog.yaml";

/// The file name of a stage's file.
#[must_use]
pub fn stage_file(key: StageKey) -> String {
    format!("{key}.yaml")
}

/// A file of Yard Log that does not read: its name within Yard Log's folder, and the error.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileError {
    pub file: String,
    pub error: ReadError,
}

impl fmt::Display for FileError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{}:{}", self.file, self.error)
    }
}

impl std::error::Error for FileError {}

fn parse(file: &str, bytes: &[u8]) -> Result<Node, FileError> {
    let error = |error: ReadError| FileError {
        file: file.to_owned(),
        error,
    };
    match yaml::read(bytes) {
        Ok(Some(node)) => Ok(node),
        Ok(None) => Err(error(ReadError {
            position: None,
            message: "the file is empty".to_owned(),
        })),
        Err(found) => Err(error(ReadError {
            position: match found.location {
                yaml::Location::At(position) => Some(position),
                yaml::Location::Byte(_) => None,
            },
            message: found.to_string(),
        })),
    }
}

/// The format a root names.
///
/// # Errors
///
/// The root does not read, or names no format.
pub fn read_format(root: &[u8]) -> Result<Format, FileError> {
    let node = parse(ROOT, root)?;
    root::format(&node).map_err(|error| FileError {
        file: ROOT.to_owned(),
        error,
    })
}

/// Reads a root alone, as the merge driver meets it.
///
/// # Errors
///
/// The root does not read, or is of a format newer than this `loft` reads.
pub fn read_root(bytes: &[u8]) -> Result<crate::log::Root, FileError> {
    let in_root = |error| FileError {
        file: ROOT.to_owned(),
        error,
    };
    let node = parse(ROOT, bytes)?;
    let format = root::format(&node).map_err(in_root)?;
    if !format.is_known() {
        return Err(in_root(ReadError {
            position: Some(node.position),
            message: format!(
                "{format} is newer than {}, the newest this `loft` reads",
                Format::CURRENT
            ),
        }));
    }
    root::read(&node).map_err(in_root)
}

/// Reads a stage's file alone, as the merge driver meets it.
///
/// # Errors
///
/// The file does not read.
pub fn read_stage(name: &str, bytes: &[u8]) -> Result<StageFile, FileError> {
    let node = parse(name, bytes)?;
    stage::read(&node).map_err(|error| FileError {
        file: name.to_owned(),
        error,
    })
}

/// The text of a root.
#[must_use]
pub fn write_root(root: &crate::log::Root) -> String {
    yaml::write(&root::write(root))
}

/// The text of a stage's file.
#[must_use]
pub fn write_stage(file: &StageFile) -> String {
    yaml::write(&stage::write(file))
}

impl YardLog {
    /// Each file and its text, the root first, then each stage's file in order.
    #[must_use]
    pub fn files(&self) -> Vec<(String, String)> {
        std::iter::once((ROOT.to_owned(), write_root(&self.root)))
            .chain(
                self.stages
                    .iter()
                    .map(|file| (stage_file(file.stage.key.stage()), write_stage(file))),
            )
            .collect()
    }

    /// Reads Yard Log from its root and the file of each stage the root lists, which `stage`
    /// gives by its name.
    ///
    /// # Errors
    ///
    /// A file does not read, a stage's file is missing or holds another stage, or a record
    /// refers to a key that Yard Log does not hold.
    pub fn read(
        root_bytes: &[u8],
        mut stage_bytes: impl FnMut(&str) -> Option<Vec<u8>>,
    ) -> Result<Self, FileError> {
        let in_file = |file: &str| {
            let file = file.to_owned();
            move |error| FileError { file, error }
        };
        let root_node = parse(ROOT, root_bytes)?;
        let mut root = read_root(root_bytes)?;
        let mut stages = Vec::new();
        let mut nodes = Vec::new();
        for entry in &mut root.stages {
            let name = stage_file(entry.key);
            let bytes = stage_bytes(&name).ok_or_else(|| FileError {
                file: name.clone(),
                error: ReadError {
                    position: None,
                    message: format!("the file of stage `{}` is missing", entry.id),
                },
            })?;
            let node = parse(&name, &bytes)?;
            let file = stage::read(&node).map_err(in_file(&name))?;
            agree(&name, &node, &file, entry.key)?;
            // The stage's own record holds its id; the root's copy follows it, since git may
            // take a stage's file whole from a copy whose settling renamed the stage.
            entry.id = file.stage.id.clone();
            stages.push(file);
            nodes.push((name, node));
        }
        let log = Self { root, stages };
        log.references(&root_node, &nodes)?;
        Ok(log)
    }

    /// Refuses a reference to a key Yard Log does not hold, at the file and line of the
    /// reference.
    fn references(&self, root: &Node, stages: &[(String, Node)]) -> Result<(), FileError> {
        let held: BTreeSet<Key> = self.blocks().map(|block| block.key.key()).collect();
        let dangling = |file: &str, found: Vec<(Position, Key)>| {
            found
                .into_iter()
                .find(|(_, key)| !held.contains(key))
                .map_or(Ok(()), |(position, key)| {
                    Err(FileError {
                        file: file.to_owned(),
                        error: ReadError {
                            position: Some(position),
                            message: format!("the reference to {key} names no block of Yard Log"),
                        },
                    })
                })
        };
        dangling(ROOT, keys_under(root, &["submissions", "blocks"]))?;
        for (name, node) in stages {
            dangling(name, keys_under(node, &["blocks", "versions", "question"]))?;
        }
        Ok(())
    }
}

/// Refuses a stage's file that holds another stage than the root names for it.
fn agree(name: &str, node: &Node, file: &StageFile, key: StageKey) -> Result<(), FileError> {
    if file.stage.key.stage() == key {
        return Ok(());
    }
    Err(FileError {
        file: name.to_owned(),
        error: ReadError {
            position: Some(node.position),
            message: format!("the file holds stage {}, not {key}", file.stage.key),
        },
    })
}

/// The keys at the end of `path`, through every sequence along the way, with where each is.
fn keys_under(node: &Node, path: &[&str]) -> Vec<(Position, Key)> {
    match (&node.value, path.split_first()) {
        (Value::Sequence(items), _) => items
            .iter()
            .flat_map(|item| keys_under(item, path))
            .collect(),
        (Value::Mapping(entries), Some((first, rest))) => entries
            .iter()
            .filter(|entry| entry.key == *first)
            .flat_map(|entry| keys_under(&entry.value, rest))
            .collect(),
        (Value::Scalar(scalar), None) => Key::parse(&scalar.text)
            .map(|key| vec![(node.position, key)])
            .unwrap_or_default(),
        _ => Vec::new(),
    }
}
