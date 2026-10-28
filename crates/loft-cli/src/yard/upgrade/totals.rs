//! The totals step of `loft yard upgrade`. The events of every Codex session this machine keeps
//! are moved first, whatever its totals file's format and whichever Yard Log they count toward,
//! since the event logs belong to the machine (`events`). Then each Codex totals file of an open
//! Yard Log whose session's events this machine keeps is written as replaying them counts it,
//! whatever its totals format, since an earlier page server may have counted a zero it did not
//! measure. Every other totals file of an older totals format is written in the current one, a
//! Codex one with each zero cache-write count unavailable, since the file cannot tell which zeros
//! were measured. A file whose text would not change keeps its bytes. While a page server holds
//! its lock, nothing is written: it appends to the event logs and would write its own counts over
//! the files. Otherwise the step holds that lock until it has written the last file, so that no
//! page server starts meanwhile.

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde_json::{Value, json};
use yardlog::totals::{Totals, TotalsFormat};
use yardlog::usage::Counts;
use yardlog_store::YardStore;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;

use crate::yard::failure::Failure;
use crate::yard::page;
use crate::yard::session_files;
use crate::yard::totals::replayed;

/// What became of the totals files of the open Yard Logs.
pub enum Classifying {
    /// Each file written: its branch, host and session.
    Done(Vec<Value>),
    /// Every file left as it was, because a page server holds its lock: the one on `port`, or one
    /// that has not named itself yet.
    Left { port: Option<u16> },
    /// The step stopped on a file it failed to settle, after writing `classified`.
    Failed {
        classified: Vec<Value>,
        unsettled: Unsettled,
    },
}

/// The file the totals step failed to settle, and why.
pub struct Unsettled {
    path: PathBuf,
    message: String,
}

impl Unsettled {
    fn at(path: PathBuf, error: &impl ToString) -> Self {
        Self {
            path,
            message: error.to_string(),
        }
    }
}

impl Classifying {
    /// Whether the step failed to settle a file.
    pub const fn failed(&self) -> bool {
        matches!(self, Self::Failed { .. })
    }

    /// Adds what this step did to the command's result `fields`.
    pub fn add_to(&self, fields: &mut Value) {
        match self {
            Self::Done(classified) => fields["classified"] = json!(classified),
            Self::Left { port } => {
                fields["classified"] = json!([]);
                fields["totalsLeft"] = left(*port);
            }
            Self::Failed {
                classified,
                unsettled,
            } => {
                fields["classified"] = json!(classified);
                let path = unsettled.path.display().to_string();
                fields["totalsFailed"] = json!({ "path": path, "message": unsettled.message });
            }
        }
    }
}

/// Why every totals file was left: the page server on `port` runs, or a process holds its lock
/// without naming itself.
fn left(port: Option<u16>) -> Value {
    let again = "run `loft yard upgrade` again to settle the totals files";
    let Some(port) = port else {
        let lock = page::SERVER_LOCK;
        let reason = format!(
            "a process holds the page server's lock `~/.yard/{lock}` without naming itself, as a page server does while it starts: stop it and {again}"
        );
        return json!({ "reason": reason });
    };
    let reason = format!("a page server runs on port {port}: stop it and {again}");
    json!({ "port": port, "reason": reason })
}

/// Moves the unverified zeros of the Codex events in every event log of the machine, then writes
/// each totals file of the Yard Logs `open`, each by its branch, that the step counts otherwise;
/// stops on the first file it fails to settle.
pub fn classify(open: &[(String, YardStore)]) -> Classifying {
    let mut classified = Vec::new();
    match settle(open, &mut classified) {
        Ok(Ok(())) => Classifying::Done(classified),
        Ok(Err(port)) => Classifying::Left { port },
        Err(unsettled) => Classifying::Failed {
            classified,
            unsettled,
        },
    }
}

/// A totals file of an open Yard Log: its branch, the Yard Log's store, its session key and the
/// file as JSON.
type File<'a> = (&'a String, &'a YardStore, String, Value);

/// [`classify`]'s work, under the page server's lock: each file it writes added to `classified`;
/// where a page server holds the lock, nothing is written, and the error gives its port, where it
/// named one.
fn settle(
    open: &[(String, YardStore)],
    classified: &mut Vec<Value>,
) -> Result<Result<(), Option<u16>>, Unsettled> {
    let files: Vec<File<'_>> = open
        .iter()
        .flat_map(|(branch, store)| {
            let files = session_files::entries(store, SessionFile::Tokens);
            files
                .into_iter()
                .map(move |(key, file)| (branch, store, key, file))
        })
        .collect();
    let older = |file: &Value| TotalsFormat::of(file).is_some_and(|f| f < TotalsFormat::CURRENT);
    let first_older = files.iter().find(|(.., file)| older(file));
    let home = match (Home::from_env(), first_older) {
        (Some(home), _) => home,
        (None, None) => return Ok(Ok(())),
        (None, Some((_, store, key, _))) => {
            let path = store.session_path(SessionFile::Tokens, key);
            return Err(Unsettled::at(path, &Failure::NO_HOME));
        }
    };
    let pending = pending(&home)?;
    if pending.is_empty() && first_older.is_none() {
        // Nothing to move and nothing older: only a Codex file its replay counts otherwise.
        let mut replays = Replays::new(&home);
        if !files
            .iter()
            .any(|file| rewritten(file, &mut replays).is_some())
        {
            return Ok(Ok(()));
        }
    }
    // Held until the last file is written, so that no page server starts meanwhile.
    let quiet = page::quiet(&home);
    let lock = || home.dir().join(page::SERVER_LOCK);
    let server = match quiet.map_err(|error| Unsettled::at(lock(), &error))? {
        Ok(locked) => locked,
        Err(running) => return Ok(Err(running.map(|server| server.port))),
    };
    // The events first: a run stopped after them settles the totals files on the next.
    for key in pending {
        let settled = super::events::settle(&home, &server, &key);
        settled.map_err(|error| Unsettled::at(home.events(&key), &error))?;
    }
    let mut replays = Replays::new(&home);
    for file in &files {
        let Some(text) = rewritten(file, &mut replays) else {
            continue;
        };
        let (branch, store, key, totals) = file;
        let path = store.session_path(SessionFile::Tokens, key);
        let written = store.write_session(SessionFile::Tokens, key, text.as_bytes());
        written.map_err(|error| Unsettled::at(path, &error))?;
        let (host, session) = (&totals["host"], &totals["session"]);
        classified.push(json!({ "branch": branch, "host": host, "session": session }));
    }
    Ok(Ok(()))
}

/// The sessions whose event logs hold a Codex count to move.
fn pending(home: &Home) -> Result<Vec<String>, Unsettled> {
    let sessions = home.event_sessions();
    let events = || home.dir().join("events");
    let mut pending = Vec::new();
    for key in sessions.map_err(|error| Unsettled::at(events(), &error))? {
        let found = super::events::pending(home, &key);
        if found.map_err(|error| Unsettled::at(home.events(&key), &error))? {
            pending.push(key);
        }
    }
    Ok(pending)
}

/// The text the totals step writes for `file`, where it differs from the file's bytes: a Codex
/// session's totals as replaying its events counts them, where this machine keeps events of it
/// in the file's Yard Log, whatever the file's known totals format; else a file of an older
/// totals format in the current one, with each zero cache-write count unavailable where its host
/// is Codex, since the file cannot tell which zeros were measured.
fn rewritten((_, store, key, file): &File<'_>, replays: &mut Replays<'_>) -> Option<String> {
    let format = TotalsFormat::of(file).filter(|format| *format <= TotalsFormat::CURRENT)?;
    let codex = file["host"] == "codex";
    let text = match replays.of(store, key).filter(|_| codex) {
        Some(replayed) => replayed.text(),
        None if format < TotalsFormat::CURRENT => {
            let mut totals = Totals::read(file);
            if codex {
                totals.map_counts(Counts::without_zero_write);
            }
            totals.text()
        }
        None => return None,
    };
    let bytes = store.session_file(SessionFile::Tokens, key);
    (bytes.as_deref() != Some(text.as_bytes())).then_some(text)
}

/// Each session's totals in every Yard Log its events count toward, replayed once.
struct Replays<'a> {
    home: &'a Home,
    sessions: BTreeMap<String, BTreeMap<(String, String), Totals>>,
}

impl<'a> Replays<'a> {
    const fn new(home: &'a Home) -> Self {
        Self {
            home,
            sessions: BTreeMap::new(),
        }
    }

    /// Session `key`'s totals in the Yard Log of `store`, where its events count a request there.
    fn of(&mut self, store: &YardStore, key: &str) -> Option<&Totals> {
        let home = self.home;
        let replayed = self
            .sessions
            .entry(key.to_owned())
            .or_insert_with(|| replayed(home, key));
        let place = (
            store.place().tree.display().to_string(),
            store.branch().to_owned(),
        );
        replayed.get(&place).filter(|totals| totals.counted())
    }
}
