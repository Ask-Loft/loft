//! `~/.yard/flush.json`: for each address of Yard Server, the last run of the flush that sent to
//! it: when it ended and how, why a batch did not go, the time Yard Server asked it not to come
//! back before, and the versions Yard Server will take later, each with its code. Only a run
//! that holds `~/.yard/flush.lock` writes it, through a temporary file renamed into place; a file
//! that does not read holds no record. It holds no token.

use std::collections::{BTreeMap, BTreeSet};

use serde_json::{Map, Value, json};
use yardlog::Time;
use yardlog_store::YardStoreError;
use yardlog_store::home::{Home, HomeFile};

use super::super::home_json;

/// One address's record.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Record {
    /// When the run ended, in seconds since the Unix epoch.
    pub ended: u64,
    pub outcome: String,
    /// Why a batch did not go, where one did not.
    pub why: Option<String>,
    /// The time Yard Server asked the flush not to come back before, in seconds.
    pub not_before: Option<u64>,
    /// The versions Yard Server will take later, by key, each with its code.
    pub later: BTreeMap<String, String>,
}

impl Record {
    /// The time Yard Server asked the flush not to come back before, where it has not come by
    /// `now`, in seconds since the Unix epoch.
    #[must_use]
    pub fn paused(&self, now: u64) -> Option<u64> {
        self.not_before.filter(|time| now < *time)
    }

    /// The keys of the versions Yard Server will take later.
    #[must_use]
    pub fn later_keys(self) -> BTreeSet<String> {
        self.later.into_keys().collect()
    }
}

/// What a run that replaces its address's record did.
pub struct Run<'a> {
    pub ended: u64,
    pub outcome: &'static str,
    pub why: Option<String>,
    pub not_before: Option<u64>,
    /// The versions Yard Server answered `later` in the run, by key, with their codes.
    pub later: &'a BTreeMap<String, String>,
}

/// The record `home` holds for `endpoint`, where it holds one that reads.
#[must_use]
pub fn read(home: &Home, endpoint: &str) -> Option<Record> {
    read_all(home).remove(endpoint)
}

/// The records `home` holds that read, by address.
#[must_use]
pub fn read_all(home: &Home) -> BTreeMap<String, Record> {
    let bytes = home.read(HomeFile::Flush).ok().flatten();
    home_json::object(bytes.as_deref())
        .iter()
        .filter_map(|(endpoint, record)| Some((endpoint.clone(), parse(record)?)))
        .collect()
}

/// Replaces `endpoint`'s record with `run`'s, keeping from the record it replaces a time to come
/// back after that has not passed, and the `later` key of each entry `waiting`, the keys of the
/// entries for the address, still holds, unless Yard Server answered it in the run: the record
/// written. An entry Yard Server stored, held or refused is no longer waiting, and one it put off
/// again takes the code of its new answer.
///
/// # Errors
///
/// `~/.yard/flush.json` cannot be written.
pub fn replace(
    home: &Home,
    endpoint: &str,
    run: &Run<'_>,
    waiting: &BTreeSet<String>,
) -> Result<Record, YardStoreError> {
    let mut written = Record::default();
    home.update(HomeFile::Flush, |held| {
        let mut all = home_json::object(held.as_deref());
        let previous = all.get(endpoint).and_then(parse).unwrap_or_default();
        written = next(&previous, run, waiting);
        all.insert(endpoint.to_owned(), value(&written));
        Some(home_json::ordered(&all))
    })?;
    Ok(written)
}

/// The record that replaces `previous` after `run`.
pub(super) fn next(previous: &Record, run: &Run<'_>, waiting: &BTreeSet<String>) -> Record {
    let kept = previous.not_before.filter(|time| *time > run.ended);
    let mut later: BTreeMap<String, String> = previous
        .later
        .iter()
        .filter(|(key, _)| waiting.contains(*key))
        .map(|(key, code)| (key.clone(), code.clone()))
        .collect();
    later.extend(
        run.later
            .iter()
            .map(|(key, code)| (key.clone(), code.clone())),
    );
    Record {
        ended: run.ended,
        outcome: run.outcome.to_owned(),
        why: run.why.clone(),
        not_before: run.not_before.max(kept),
        later,
    }
}

fn time(seconds: u64) -> Value {
    Time::from_unix_seconds(seconds).map_or(Value::Null, |time| json!(time.to_string()))
}

fn seconds(value: &Value) -> Option<u64> {
    value.as_str()?.parse::<Time>().ok().map(Time::unix_seconds)
}

fn value(record: &Record) -> Value {
    let later: Map<String, Value> = record
        .later
        .iter()
        .map(|(key, code)| (key.clone(), json!(code)))
        .collect();
    json!({
        "ended": time(record.ended),
        "outcome": record.outcome,
        "why": record.why,
        "notBefore": record.not_before.map_or(Value::Null, time),
        "later": later,
    })
}

fn parse(value: &Value) -> Option<Record> {
    Some(Record {
        ended: seconds(value.get("ended")?)?,
        outcome: value.get("outcome")?.as_str()?.to_owned(),
        why: value.get("why").and_then(Value::as_str).map(str::to_owned),
        not_before: value.get("notBefore").and_then(seconds),
        later: value
            .get("later")
            .and_then(Value::as_object)
            .map(|later| {
                later
                    .iter()
                    .filter_map(|(key, code)| Some((key.clone(), code.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default(),
    })
}
