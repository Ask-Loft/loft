//! The events the mod posts. Each is kept, where its agent's branch has its Yard Log open, in the
//! session's event log on this machine, `~/.yard/events/<session id>.jsonl`, with the tree and
//! branch its agent's directory is on as it arrives, the stage active for its agent and the
//! submissions open; and what it starts or ends, or counts, is kept in the session's activity
//! and token totals in that Yard Log's folder. The page server rebuilds a session's activity and
//! totals from its event log the first time it hears of the session.

use std::collections::{BTreeMap, HashMap};
use std::path::Path;
use std::sync::Mutex;

use serde_json::{Value, json};
use yardlog::YardLog;
use yardlog::query::State;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;
use yardlog_store::{Held, Place, YardStore};

use super::activity::Activity;
use super::event::{Event, Who};
use super::http::Response;
use super::tokens::Totals;

/// What one session keeps in one branch's Yard Log.
#[derive(Default)]
struct Kept {
    activity: Option<Activity>,
    totals: Option<Totals>,
}

/// What one session keeps, by the tree and branch of each Yard Log it keeps it in.
type ByLog = BTreeMap<(String, String), Kept>;

/// Each session's activity and totals, by session.
#[derive(Default)]
pub struct Sessions {
    kept: Mutex<HashMap<String, ByLog>>,
}

/// Where an event is kept: its agent's tree and branch, the stage active for its agent and the
/// submissions open as it arrives.
struct Arrival {
    store: YardStore,
    stage: Option<String>,
    open: Vec<String>,
}

/// The stage active for `who` of `session` in `log`, by its key.
fn active_stage(log: &YardLog, session: &str, who: &Who) -> Option<String> {
    log.stages
        .iter()
        .map(|file| &file.stage)
        .filter(|stage| stage.is_active())
        .find(|stage| {
            stage.owner().is_some_and(|owner| {
                owner.session == session
                    && match who {
                        Who::Main => owner.subagent.is_none(),
                        Who::Sub { id, .. } => {
                            owner.subagent.as_ref().is_some_and(|sub| &sub.id == id)
                        }
                    }
            })
        })
        .map(|stage| stage.key.to_string())
}

/// The store and open Yard Log of the branch the directory `dir` is on, where it has one.
fn open_at(dir: &str) -> Option<(YardStore, YardLog)> {
    let place = Place::locate(&YardGit::default(), Path::new(dir)).ok()?;
    place.branch.as_ref()?;
    let store = YardStore::new(place).ok()?;
    let Ok(Held::Log(log)) = store.read() else {
        return None;
    };
    (!log.is_closed()).then_some((store, *log))
}

/// Where each of `events` is kept, where its agent's branch has its Yard Log open: `open` finds
/// and reads that Yard Log once for each directory the batch names, however many events name it.
fn arrivals(
    events: &[Event],
    mut open: impl FnMut(&str) -> Option<(YardStore, YardLog)>,
) -> Vec<Option<Arrival>> {
    let mut opened: HashMap<&str, Option<(YardStore, YardLog)>> = HashMap::new();
    events
        .iter()
        .map(|event| {
            let (store, log) = opened
                .entry(event.dir.as_str())
                .or_insert_with(|| open(&event.dir))
                .as_ref()?;
            let submissions = log
                .root
                .submissions
                .iter()
                .filter(|submission| submission.state(log) == State::Open)
                .map(|submission| submission.key.to_string())
                .collect();
            Some(Arrival {
                stage: active_stage(log, &event.session, &event.who),
                open: submissions,
                store: store.clone(),
            })
        })
        .collect()
}

impl Kept {
    /// Keeps what `event` adds; gives the files whose text changed.
    fn apply(
        &mut self,
        event: &Event,
        stage: Option<&str>,
        open: &[String],
    ) -> Vec<(SessionFile, String)> {
        let mut changed = Vec::new();
        let activity = self
            .activity
            .get_or_insert_with(|| Activity::new(&event.session));
        if activity.apply(event, stage) {
            changed.push((SessionFile::Activity, activity.text()));
        }
        let totals = self
            .totals
            .get_or_insert_with(|| Totals::new(&event.session));
        if totals.apply(event, stage, open) && totals.counted() {
            changed.push((SessionFile::Tokens, totals.text()));
        }
        changed
    }
}

/// The open submissions an event's line in the log names.
fn named_open(line: &Value) -> Vec<String> {
    line["submissions"]
        .as_array()
        .map(|keys| {
            keys.iter()
                .filter_map(Value::as_str)
                .map(ToOwned::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// What a session kept before the page server heard of it, rebuilt from its event log.
fn replay(home: &Home, session: &str) -> ByLog {
    let mut kept = ByLog::new();
    let Ok(Some(bytes)) = home.read_events(session) else {
        return kept;
    };
    for line in bytes.split(|byte| *byte == b'\n') {
        let Ok(line) = serde_json::from_slice::<Value>(line) else {
            continue;
        };
        let (Ok(event), Some(tree), Some(branch)) = (
            Event::read(&line),
            line["tree"].as_str(),
            line["branch"].as_str(),
        ) else {
            continue;
        };
        kept.entry((tree.to_owned(), branch.to_owned()))
            .or_default()
            .apply(&event, line["stage"].as_str(), &named_open(&line));
    }
    kept
}

impl Sessions {
    /// Records the events `body` holds, one event or a list of them in the order they happened.
    pub fn record(&self, home: &Home, body: &[u8]) -> Response {
        let posted = match serde_json::from_slice::<Value>(body) {
            Ok(Value::Array(events)) => events,
            Ok(event @ Value::Object(_)) => vec![event],
            _ => {
                return Response::refused(
                    400,
                    "input",
                    "the request's body is no event or list of events",
                );
            }
        };
        let mut recorded = 0;
        let mut skipped = 0;
        let mut rejected = Vec::new();
        let mut read = Vec::new();
        for (index, posted) in posted.iter().enumerate() {
            match Event::read(posted) {
                Ok(event) => read.push((index, event)),
                Err(message) => rejected.push(json!({ "index": index, "message": message })),
            }
        }
        let (indices, events): (Vec<usize>, Vec<Event>) = read.into_iter().unzip();
        // git and Yard Log are read before the sessions are locked, so that one session's batch
        // does not hold another's posts while it reads them.
        let arrived = arrivals(&events, open_at);
        let mut kept = self
            .kept
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        for ((index, event), arrival) in indices.into_iter().zip(&events).zip(arrived) {
            let Some(arrival) = arrival else {
                skipped += 1;
                continue;
            };
            match keep(home, &mut kept, event, &arrival) {
                Ok(()) => recorded += 1,
                Err(message) => rejected.push(json!({ "index": index, "message": message })),
            }
        }
        Response::json(
            200,
            &json!({ "recorded": recorded, "skipped": skipped, "rejected": rejected }),
        )
    }
}

/// Appends `event` to its session's log and keeps what it adds in its Yard Log's folder.
fn keep(
    home: &Home,
    kept: &mut HashMap<String, ByLog>,
    event: &Event,
    arrival: &Arrival,
) -> Result<(), String> {
    let place = arrival.store.place();
    let tree = place.tree.display().to_string();
    let branch = arrival.store.branch().to_owned();
    let session = kept
        .entry(event.session.clone())
        .or_insert_with(|| replay(home, &event.session));
    let mut line = event.posted.clone();
    line["tree"] = json!(tree);
    line["branch"] = json!(branch);
    line["stage"] = json!(arrival.stage);
    line["submissions"] = json!(arrival.open);
    let text = serde_json::to_vec(&line).map_err(|error| error.to_string())?;
    home.append_event(&event.session, &text)
        .map_err(|error| error.to_string())?;
    let changed = session.entry((tree, branch)).or_default().apply(
        event,
        arrival.stage.as_deref(),
        &arrival.open,
    );
    for (file, text) in changed {
        arrival
            .store
            .write_session(file, &event.session, text.as_bytes())
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{Event, arrivals};

    #[test]
    fn a_batch_opens_the_yard_log_of_each_directory_it_names_once() {
        let events: Vec<Event> = (0..1000)
            .map(|n| {
                let dir = if n % 2 == 0 { "/work/a" } else { "/work/b" };
                let posted = json!({
                    "session": "s1", "dir": dir, "at": "2026-10-22T09:30:00Z", "kind": "turn.start"
                });
                Event::read(&posted).unwrap()
            })
            .collect();
        let mut opened = Vec::new();
        let found = arrivals(&events, |dir| {
            opened.push(dir.to_owned());
            None
        });
        assert_eq!(found.len(), 1000);
        opened.sort();
        assert_eq!(opened, ["/work/a", "/work/b"]);
    }
}
