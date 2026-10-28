//! The totals step of `loft yard upgrade`: each totals file of an open Yard Log in an older totals
//! format, written again in the current one, and where its host is Codex, with each zero
//! cache-write count unavailable, since a Codex adapter that did not classify cache writes at the
//! source may have given a zero it did not measure: counted again from the session's moved events
//! where this machine keeps them, else settled from the file itself. The events of every Codex
//! session of an open Yard Log are moved first, whatever its file's format, since an earlier page
//! server wrote Codex files in the current format without moving their events. While a page
//! server runs, nothing is written: it appends to the event logs and would write its own counts
//! over the files.

use std::collections::BTreeSet;
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
    /// Every file left as it was, because the page server on `port` runs.
    Left { port: u16 },
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
                let reason = format!(
                    "a page server runs on port {port}: stop it and run `loft yard upgrade` again to settle the totals files"
                );
                fields["totalsLeft"] = json!({ "port": port, "reason": reason });
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

/// Moves the unverified zeros of each Codex session's events, whatever its totals file's format,
/// then writes each totals file of an older totals format of the Yard Logs `open`, each by its
/// branch; stops on the first file it fails to settle.
pub fn classify(open: &[(String, YardStore)]) -> Classifying {
    let mut classified = Vec::new();
    match settle(open, &mut classified) {
        Ok(None) => Classifying::Done(classified),
        Ok(Some(port)) => Classifying::Left { port },
        Err(unsettled) => Classifying::Failed {
            classified,
            unsettled,
        },
    }
}

/// [`classify`]'s work: each file it writes added to `classified`; gives the port of a page
/// server that runs, where one does.
fn settle(
    open: &[(String, YardStore)],
    classified: &mut Vec<Value>,
) -> Result<Option<u16>, Unsettled> {
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
        (None, None) => return Ok(None),
        (None, Some((_, store, key, _))) => {
            let path = store.session_path(SessionFile::Tokens, key);
            return Err(Unsettled::at(path, &Failure::NO_HOME));
        }
    };
    let codex: BTreeSet<_> = files
        .iter()
        .filter(|(.., file)| file["host"] == "codex")
        .map(|(_, _, key, _)| key.clone())
        .collect();
    let mut pending = Vec::new();
    for key in &codex {
        let found = super::events::pending(&home, key);
        if found.map_err(|error| Unsettled::at(home.events(key), &error))? {
            pending.push(key);
        }
    }
    if pending.is_empty() && first_older.is_none() {
        return Ok(None);
    }
    let running = page::running(&home);
    let lock = || home.dir().join(page::SERVER_LOCK);
    if let Some(server) = running.map_err(|error| Unsettled::at(lock(), &error))? {
        return Ok(Some(server.port));
    }
    // The events first: a run stopped after them settles the totals files on the next.
    for key in pending {
        let settled = super::events::settle(&home, key);
        settled.map_err(|error| Unsettled::at(home.events(key), &error))?;
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
    Ok(None)
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
