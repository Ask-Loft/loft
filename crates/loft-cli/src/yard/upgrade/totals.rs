//! The totals step of `loft yard upgrade`: each totals file of an open Yard Log that carries no
//! `cacheWriteClassified` marker, written again with the marker, and where its host is Codex,
//! with each zero cache-write count unavailable, since a Codex adapter that did not classify
//! cache writes at the source may have given a zero it did not measure. While a page server
//! runs, nothing is written: it appends to the event logs and would write its own counts over
//! the files.

use std::collections::BTreeSet;

use serde_json::{Value, json};
use yardlog_store::YardStore;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;

use crate::yard::failure::Failure;
use crate::yard::page;
use crate::yard::serve::session_files;
use crate::yard::serve::tokens::Totals;

/// What became of the totals files of the open Yard Logs.
pub enum Classifying {
    /// Each file written with the marker: its branch, host and session.
    Done(Vec<Value>),
    /// Every file left as it was, because the page server on `port` runs.
    Left { port: u16 },
}

impl Classifying {
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
        }
    }
}

/// Writes each unmarked totals file of the Yard Logs `open`, each by its branch.
pub fn classify(open: &[(String, YardStore)]) -> Result<Classifying, Failure> {
    let unmarked: Vec<_> = open
        .iter()
        .flat_map(|(branch, store)| {
            let files = session_files::entries(&store.folder(), SessionFile::Tokens);
            files
                .into_iter()
                .map(move |(key, file)| (branch, store, key, file))
        })
        .filter(|(_, _, _, file)| file["cacheWriteClassified"] != Value::Bool(true))
        .collect();
    if unmarked.is_empty() {
        return Ok(Classifying::Done(Vec::new()));
    }
    let home = Home::from_env().ok_or_else(Failure::no_home)?;
    if let Some(server) = page::running(&home)? {
        return Ok(Classifying::Left { port: server.port });
    }
    let mut classified = Vec::new();
    let mut settled = BTreeSet::new();
    for (branch, store, key, mut file) in unmarked {
        let host = file["host"].as_str().unwrap_or_default().to_owned();
        if host == "codex" {
            // The events first: a run stopped after them settles the file on the next.
            if settled.insert(key.clone()) {
                super::events::settle(&home, &key)?;
            }
            classify_codex(&mut file);
        }
        let text = Totals::read(&file).text();
        store.write_session(SessionFile::Tokens, &key, text.as_bytes())?;
        classified.push(json!({ "branch": branch, "host": host, "session": file["session"] }));
    }
    Ok(Classifying::Done(classified))
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
