//! The totals step of `loft yard upgrade`: each totals file of an open Yard Log in an older totals
//! format, written again in the current one, and where its host is Codex, with each zero
//! cache-write count unavailable, since a Codex adapter that did not classify cache writes at the
//! source may have given a zero it did not measure: counted again from the session's moved events
//! where this machine keeps them, else settled from the file itself. The events of every Codex
//! session this machine keeps are moved first, whatever its file's format and whichever Yard Log
//! they count toward, since an earlier page server wrote Codex files in the current format without
//! moving their events, and the event logs belong to the machine. While a page server holds its
//! lock, nothing is written: it appends to the event logs and would write its own counts over the
//! files. Otherwise the step holds that lock until it has written the last file, so that no page
//! server starts meanwhile.

use std::path::PathBuf;

use serde_json::{Value, json};
use yardlog::totals::{Totals, TotalsFormat};
use yardlog_store::YardStore;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;

use crate::yard::failure::Failure;
use crate::yard::page;
use crate::yard::session_files;
use crate::yard::totals::replayed_totals;

/// What became of the totals files of the open Yard Logs.
pub enum Classifying {
    /// Each file written in the current totals format: its branch, host and session.
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
/// each totals file of an older totals format of the Yard Logs `open`, each by its branch; stops
/// on the first file it fails to settle.
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

/// [`classify`]'s work, under the page server's lock: each file it writes added to `classified`;
/// where a page server holds the lock, nothing is written, and the error gives its port, where it
/// named one.
fn settle(
    open: &[(String, YardStore)],
    classified: &mut Vec<Value>,
) -> Result<Result<(), Option<u16>>, Unsettled> {
    let files: Vec<_> = open
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
    let sessions = home.event_sessions();
    let events = || home.dir().join("events");
    let mut pending = Vec::new();
    for key in sessions.map_err(|error| Unsettled::at(events(), &error))? {
        let found = super::events::pending(&home, &key);
        if found.map_err(|error| Unsettled::at(home.events(&key), &error))? {
            pending.push(key);
        }
    }
    if pending.is_empty() && first_older.is_none() {
        return Ok(Ok(()));
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
    for (branch, store, key, file) in files.into_iter().filter(|(.., file)| older(file)) {
        let host = file["host"].as_str().unwrap_or_default().to_owned();
        let text = if host == "codex" {
            codex_totals(&home, store, &key, file.clone())
        } else {
            Totals::read(&file).text()
        };
        let path = store.session_path(SessionFile::Tokens, &key);
        let written = store.write_session(SessionFile::Tokens, &key, text.as_bytes());
        written.map_err(|error| Unsettled::at(path, &error))?;
        classified.push(json!({ "branch": branch, "host": host, "session": file["session"] }));
    }
    Ok(Ok(()))
}

/// A Codex session's totals, counted as the page server counts them from the session's moved
/// events, so that a later replay writes the same. Where this machine keeps no event of the
/// session in this Yard Log, the file itself is settled: each zero cache-write count is made
/// unavailable, and each positive one is kept, with no word of the zeros it may hold.
fn codex_totals(home: &Home, store: &YardStore, key: &str, mut file: Value) -> String {
    let tree = store.place().tree.display().to_string();
    replayed_totals(home, key, &tree, store.branch()).unwrap_or_else(|| {
        classify_codex(&mut file);
        Totals::read(&file).text()
    })
}

/// Makes each zero cache-write count of a Codex totals file unavailable: in all, under no stage,
/// by stage and within each by agent, by agent and model, and by submission.
fn classify_codex(file: &mut Value) {
    for name in ["tokens", "noStage"] {
        if let Some(counts) = file.get_mut(name) {
            unavailable_zero(counts);
        }
    }
    for stage in items(file, "stages") {
        unavailable_zero(stage);
        items(stage, "agents").for_each(unavailable_zero);
    }
    for name in ["agents", "submissions"] {
        items(file, name).for_each(unavailable_zero);
    }
}

fn items<'a>(value: &'a mut Value, name: &str) -> impl Iterator<Item = &'a mut Value> {
    value
        .get_mut(name)
        .and_then(Value::as_array_mut)
        .into_iter()
        .flatten()
}

/// Makes a zero cache-write count unavailable: a counter the file does not give.
fn unavailable_zero(counts: &mut Value) {
    if counts["cacheWrite"] == 0
        && let Some(fields) = counts.as_object_mut()
    {
        fields.remove("cacheWrite");
    }
}
