//! `loft interface`: pins, moves, checks and removes the copies of other documents' Interfaces
//! that a connected document links to. Each other document is reached at the address its entry
//! of `documents` names, with the person's sign-in to that address; `LOFT_REMOTE_ENDPOINT` and
//! `LOFT_REMOTE_TOKEN` name the document's own remote and change none of these calls. Like
//! `loft remote`, it sends no command event and asks for no update.

mod add;
mod reach;
mod references;
mod status;
mod update;

use std::num::NonZeroU64;
use std::path::Path;

use loftspec::codec::{self, Files};
use loftspec::copy::Copy;
use loftspec::document::{Document, OtherDocument};
use loftspec::identity::{Key, Label};
use loftspec::slug::Slug;
use loftspec_store::FileStore;
use serde_json::Value;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-interface/1";

/// What the command does.
pub enum Action {
    Add {
        interface: String,
        at: Option<NonZeroU64>,
        remote: Option<String>,
        browser: bool,
    },
    Update {
        target: Option<String>,
        at: Option<NonZeroU64>,
        browser: bool,
    },
    Status {
        browser: bool,
    },
}

/// What a subcommand reports: its outcome, its fields, and its lines of text; `failed` where
/// the outcome exits 1.
struct Report {
    outcome: &'static str,
    fields: Value,
    lines: Vec<String>,
    failed: bool,
}

pub fn run(root: &Path, action: &Action, as_json: bool) -> Finished {
    let report = match action {
        Action::Add {
            interface,
            at,
            remote,
            browser,
        } => add::run(root, interface, *at, remote.as_deref(), *browser),
        Action::Update {
            target,
            at,
            browser,
        } => update::run(root, target.as_deref(), *at, *browser),
        Action::Status { browser } => status::run(root, *browser),
    };
    match report {
        Ok(report) => {
            if as_json {
                json::print(&json::result(
                    SCHEMA,
                    report.outcome,
                    report.fields,
                    Vec::new(),
                ));
            } else {
                for line in &report.lines {
                    outln!("{line}");
                }
            }
            if report.failed {
                Finished::failure(report.outcome)
            } else {
                Finished::success(report.outcome)
            }
        }
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

/// A refusal of the command.
fn refused(message: impl Into<String>) -> Stop {
    Stop::one("refused", message.into())
}

/// Refuses a document that is not connected.
fn connected(document: &Document) -> Result<(), Stop> {
    if document.connected {
        return Ok(());
    }
    Err(refused(format!(
        "the document {} is not connected: `loft remote set <address>` connects it to its remote",
        document.id.key()
    )))
}

/// The entry of `documents` that lists the handle of `copy`.
fn entry_of<'a>(document: &'a Document, copy: &Copy) -> &'a OtherDocument {
    (document.documents.iter())
        .find(|entry| entry.handle == copy.handle)
        .expect("the reader refuses a copy whose handle is not listed")
}

/// The tree's store and the document it holds.
fn document(root: &Path) -> Result<(FileStore, Document), Stop> {
    let store = FileStore::open(root)?;
    let document = codec::read(&store.read_files()?).map_err(Stop::from)?;
    Ok((store, document))
}

/// Writes the files at `paths` of `document` as the writer writes them, in one journaled save.
fn save(store: &FileStore, document: &Document, paths: &[codec::SpecPath]) -> Result<(), Stop> {
    let written = codec::write(document);
    let files: Files = (paths.iter())
        .map(|path| {
            let bytes = written.get(path).expect("the writer writes the file");
            (path.clone(), bytes.clone())
        })
        .collect();
    store.write_files(files)?;
    Ok(())
}

/// An Interface of another document, as the command line names it: its handle, and its label
/// or key in that document.
struct Named {
    handle: Slug,
    interface: Interface,
}

/// The Interface, by its label or its key in the other document.
#[derive(Clone, Copy)]
enum Interface {
    Label(Label),
    Key(Key),
}

impl std::fmt::Display for Interface {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Label(label) => write!(formatter, "{label}"),
            Self::Key(key) => write!(formatter, "{key}"),
        }
    }
}

/// `text` as `<handle>:<label>` or `<handle>:<key>`.
fn named(text: &str) -> Result<Named, Stop> {
    let wrong = || {
        refused(format!(
            "{text}: an Interface is named as its handle and its label or key, such as \
             `format:IF-3`"
        ))
    };
    let (handle, interface) = text.split_once(':').ok_or_else(wrong)?;
    let handle = Slug::parse(handle).map_err(|_| wrong())?;
    let interface = match (Label::parse(interface), Key::parse(interface)) {
        (Ok(label), _) => Interface::Label(label),
        (_, Ok(key)) => Interface::Key(key),
        _ => return Err(wrong()),
    };
    Ok(Named { handle, interface })
}
