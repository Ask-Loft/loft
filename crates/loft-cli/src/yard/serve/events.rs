//! The events the mod posts. Each is kept, where its agent's branch has its Yard Log open, in the
//! session's event log on this machine, `~/.yard/events/<session id>.jsonl`, with the tree and
//! branch its agent's directory is on as it arrives, the stage it counts toward and the
//! submissions open; and what it starts or ends, or counts, is kept in the session's activity
//! and token totals in that Yard Log's folder. The page server rebuilds a session's activity and
//! totals from its event log the first time it hears of the session. A totals file of a newer
//! totals format is left as it is, and the session's requests are counted into no file.

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde_json::{Value, json};
use yardlog::adapter::Descriptor;
use yardlog::{Agent, YardLog};
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::SessionFile;
use yardlog_store::{Held, Place, YardStore};

use yardlog::totals::{Totals, TotalsFormat};

use super::activity::Activity;
use super::http::Response;
use super::newest::Newest;
use crate::yard::event::{Event, Who};
use crate::yard::event_ids::Receipts;
use crate::yard::session_files;
use crate::yard::totals;
use crate::yard::working_tree;

/// What one session keeps in one branch's Yard Log.
#[derive(Default)]
struct Kept {
    activity: Option<Activity>,
    activity_written: bool,
    totals: Option<Totals>,
}

/// What one session keeps, by the tree and branch of each Yard Log it keeps it in.
type ByLog = BTreeMap<(String, String), Kept>;

#[derive(Default)]
struct SessionKept {
    logs: ByLog,
    receipts: Receipts,
    /// The last descriptor the session posted, which each event that carries none is kept under.
    adapter: Option<Descriptor>,
}

impl SessionKept {
    /// Takes the descriptor `event` carries, if any, as the session's last.
    fn describe(&mut self, event: &Event) {
        if let Some(adapter) = &event.adapter {
            self.adapter = Some(adapter.clone());
        }
    }
}

/// Each session's activity and totals, by session, and the newest text written of each of their
/// files.
#[derive(Default)]
pub struct Sessions {
    kept: Mutex<HashMap<String, SessionKept>>,
    written: Newest,
}

/// Where an event is kept: its agent's tree and branch, the stage it counts toward and the
/// submissions open as it arrives.
struct Arrival {
    store: YardStore,
    stage: Option<String>,
    open: Vec<String>,
}

/// What the stage an event counts toward depends on: its directory's Yard Log, its agent,
/// whether a subagent's run starts, and the task it names, if any, where that is text. A batch
/// works out each one's stage once.
#[derive(PartialEq, Eq, Hash)]
struct StageQuery<'a> {
    dir: &'a str,
    host: &'a str,
    session: &'a str,
    who: &'a Who,
    starting: bool,
    task: Named<'a>,
}

/// The task an event names.
#[derive(PartialEq, Eq, Hash)]
enum Named<'a> {
    None,
    Task(&'a str),
    /// A `task` that is not text, which names no block.
    Unusable,
}

impl<'a> StageQuery<'a> {
    /// The query of `event`, whose agent's working tree is the directory `dir`.
    fn of(event: &'a Event, dir: &'a str) -> Self {
        Self {
            dir,
            host: &event.host,
            session: &event.session,
            who: &event.who,
            starting: matches!(event.who, Who::Sub { .. }) && event.kind == "subagent.start",
            task: match event.posted.get("task").map(Value::as_str) {
                None => Named::None,
                Some(Some(id)) => Named::Task(id),
                Some(None) => Named::Unusable,
            },
        }
    }

    /// The stage counted, by its key: the agent's working stage, or, as a subagent's run
    /// starts, the stage that run starts in. A posted task that is not text counts toward none.
    fn stage(&self, log: &YardLog) -> Option<String> {
        let agent = agent_of(self.host, self.session, self.who);
        let task = match self.task {
            Named::None => None,
            Named::Task(id) => Some(id),
            Named::Unusable => return None,
        };
        let stage = if self.starting {
            log.starting_stage(&agent, task)
        } else {
            log.working_stage(&agent, task)
        };
        stage.map(|stage| stage.to_string())
    }
}

fn agent_of(host: &str, session: &str, who: &Who) -> Agent {
    match who {
        Who::Main => Agent::main_in(host, session),
        Who::Sub { id, kind } => Agent::sub_in(host, session, id, kind),
    }
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

/// The directory of the working tree of `event`'s agent: the worktree its session wrote in
/// whose Yard Log holds its working stage, else the directory the event names.
fn working_dir(home: &Home, event: &Event) -> String {
    let agent = agent_of(&event.host, &event.session, &event.who);
    let dir = Path::new(&event.dir);
    working_tree::resolve(Some(home), &agent, dir)
        .map_or_else(|_| event.dir.clone(), |tree| tree.display().to_string())
}

/// A tree's Yard Log, with its open submissions, as a batch opens it.
type Opened = Option<(YardStore, YardLog, Vec<String>)>;

/// Where each of `events` is kept, where its agent's working tree has its Yard Log open:
/// `working` finds that tree once for each agent and directory the batch names, and `open`
/// finds and reads its Yard Log once for each tree, however many events name it.
fn arrivals(
    events: &[Event],
    mut working: impl FnMut(&Event) -> String,
    mut open: impl FnMut(&str) -> Option<(YardStore, YardLog)>,
) -> Vec<Option<Arrival>> {
    let mut trees: HashMap<(&str, &str, &str, &Who), String> = HashMap::new();
    let dirs: Vec<String> = events
        .iter()
        .map(|event| {
            let agent = (
                event.dir.as_str(),
                event.host.as_str(),
                event.session.as_str(),
                &event.who,
            );
            trees.entry(agent).or_insert_with(|| working(event)).clone()
        })
        .collect();
    // Each tree's Yard Log, with its open submissions, worked out once for the batch.
    let mut opened: HashMap<&str, Opened> = HashMap::new();
    let mut stages: HashMap<StageQuery, Option<String>> = HashMap::new();
    events
        .iter()
        .zip(&dirs)
        .map(|(event, dir)| {
            let (store, log, submissions) = opened
                .entry(dir.as_str())
                .or_insert_with(|| {
                    let (store, log) = open(dir)?;
                    let submissions = log
                        .open_submissions()
                        .iter()
                        .map(ToString::to_string)
                        .collect();
                    Some((store, log, submissions))
                })
                .as_ref()?;
            let stage = stages
                .entry(StageQuery::of(event, dir))
                .or_insert_with_key(|query| query.stage(log));
            Some(Arrival {
                stage: stage.clone(),
                open: submissions.clone(),
                store: store.clone(),
            })
        })
        .collect()
}

impl Kept {
    /// Keeps what `event` adds, under its session's last descriptor `adapter`; gives the files
    /// whose text changed.
    fn apply(
        &mut self,
        event: &Event,
        arrival: (Option<&str>, &[String]),
        adapter: Option<&Descriptor>,
    ) -> Vec<(SessionFile, String)> {
        let (stage, open) = arrival;
        let mut changed = Vec::new();
        let activity = self
            .activity
            .get_or_insert_with(|| Activity::new(&event.session));
        if activity.apply(event, stage, adapter) {
            self.activity_written = true;
            changed.push((SessionFile::Activity, activity.text()));
        }
        let totals = self
            .totals
            .get_or_insert_with(|| Totals::new(&event.session));
        if totals::count(totals, event, stage, open) && totals.counted() {
            changed.push((SessionFile::Tokens, totals.text()));
        }
        changed
    }

    fn snapshot(&self) -> Vec<(SessionFile, String)> {
        let mut files = Vec::new();
        if self.activity_written
            && let Some(activity) = &self.activity
        {
            files.push((SessionFile::Activity, activity.text()));
        }
        if let Some(totals) = &self.totals
            && totals.counted()
        {
            files.push((SessionFile::Tokens, totals.text()));
        }
        files
    }
}

/// What a session kept before the page server heard of it, rebuilt from its event log.
fn replay(home: &Home, session: &str) -> SessionKept {
    let mut kept = SessionKept::default();
    kept.receipts = totals::replay(home, session, |logged| {
        kept.describe(&logged.event);
        kept.logs
            .entry((logged.tree.to_owned(), logged.branch.to_owned()))
            .or_default()
            .apply(
                &logged.event,
                (logged.stage, logged.open),
                kept.adapter.as_ref(),
            );
    });
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
        hear(home, &events);
        note_main_dirs(home, &events);
        // git and Yard Log are read before the sessions are locked, so that one session's batch
        // does not hold another's posts while it reads them.
        let arrived = arrivals(&events, |event| working_dir(home, event), open_at);
        // Each session's file a batch changes is written once, as the batch leaves it.
        let mut changed: BTreeMap<PathBuf, Changed> = BTreeMap::new();
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
                Ok(files) => {
                    recorded += 1;
                    let generation = self.written.generation();
                    note(&mut changed, (index, generation), event, &arrival, files);
                }
                Err(message) => rejected.push(json!({ "index": index, "message": message })),
            }
        }
        // The sessions are let go before the files are written, so that no post waits on a disk;
        // a post that writes after a later one leaves that one's newer text in place.
        drop(kept);
        for (path, ((index, generation), store, file, session, text)) in changed {
            if file == SessionFile::Tokens && newer_totals(&store, &session) {
                continue;
            }
            let wrote = self.written.write(&path, generation, || {
                store.write_session(file, &session, text.as_bytes())
            });
            if let Err(error) = wrote {
                rejected.push(json!({ "index": index, "message": error.to_string() }));
            }
        }
        Response::json(
            200,
            &json!({ "recorded": recorded, "skipped": skipped, "rejected": rejected }),
        )
    }
}

/// Whether session `session`'s totals file in the Yard Log of `store` is of a totals format newer
/// than this `loft`'s, or one it does not know.
fn newer_totals(store: &YardStore, session: &str) -> bool {
    session_files::one(store, SessionFile::Tokens, session)
        .is_some_and(|file| !TotalsFormat::of(&file).is_some_and(TotalsFormat::is_known))
}

/// A session's file a batch changed: the index of the event that last changed it with the
/// number of its text, the store of its Yard Log, which file, whose, and its text.
type Changed = ((usize, u64), YardStore, SessionFile, String, String);

/// Notes the session's files `event` changed, each to be written once the batch is kept.
fn note(
    changed: &mut BTreeMap<PathBuf, Changed>,
    index: (usize, u64),
    event: &Event,
    arrival: &Arrival,
    files: Vec<(SessionFile, String)>,
) {
    for (file, text) in files {
        let path = arrival.store.session_path(file, &event.session_key());
        let entry = (
            index,
            arrival.store.clone(),
            file,
            event.session_key(),
            text,
        );
        changed.insert(path, entry);
    }
}

/// How many sessions the page server keeps in memory; a session it lets go is rebuilt from its
/// event log the next time it is heard of.
const SESSIONS_KEPT: usize = 256;

/// Appends `event` to its session's log and keeps what it adds; gives the session's files of its
/// Yard Log's folder whose text changed.
fn keep(
    home: &Home,
    kept: &mut HashMap<String, SessionKept>,
    event: &Event,
    arrival: &Arrival,
) -> Result<Vec<(SessionFile, String)>, String> {
    let place = arrival.store.place();
    let tree = place.tree.display().to_string();
    let branch = arrival.store.branch().to_owned();
    if !kept.contains_key(&event.session_key()) && kept.len() >= SESSIONS_KEPT {
        let gone = kept.keys().next().cloned();
        gone.map(|gone| kept.remove(&gone));
    }
    let session = kept
        .entry(event.session_key())
        .or_insert_with(|| replay(home, &event.session_key()));
    let place_key = (tree.clone(), branch.clone());
    if session.receipts.contains(event.receipt.as_ref())? {
        return Ok(session
            .logs
            .get(&place_key)
            .map_or_else(Vec::new, Kept::snapshot));
    }
    let mut line = event.posted.clone();
    if let Some(receipt) = &event.receipt {
        receipt.annotate(&mut line);
    }
    line["tree"] = json!(tree);
    line["branch"] = json!(branch);
    line["stage"] = json!(arrival.stage);
    line["submissions"] = json!(arrival.open);
    let text = serde_json::to_vec(&line).map_err(|error| error.to_string())?;
    home.append_event(&event.session_key(), &text)
        .map_err(|error| error.to_string())?;
    session.receipts.keep(event.receipt.as_ref());
    session.describe(event);
    Ok(session.logs.entry(place_key).or_default().apply(
        event,
        (arrival.stage.as_deref(), &arrival.open),
        session.adapter.as_ref(),
    ))
}

/// Records each session a batch holds events of as heard: an event a mod posts tells the page
/// server its session is live, as its beat does.
fn hear(home: &Home, events: &[Event]) {
    let Ok(at) = crate::yard::now() else {
        return;
    };
    let mut sessions: Vec<String> = events.iter().map(Event::session_key).collect();
    sessions.sort_unstable();
    sessions.dedup();
    for session in sessions {
        // Liveness is a hint for take-up: a failed write of it loses no event.
        let _ = crate::yard::sessions::heard(home, &session, at);
    }
}

/// Records, for each session whose main agent `events` name, the directory of its last event
/// there, before the batch finds its agents' working trees, so that its subagents are compared
/// with where the main agent is now.
fn note_main_dirs(home: &Home, events: &[Event]) {
    let mut last: HashMap<(&str, &str), &str> = HashMap::new();
    for event in events.iter().filter(|event| matches!(event.who, Who::Main)) {
        last.insert(
            (event.host.as_str(), event.session.as_str()),
            event.dir.as_str(),
        );
    }
    for ((host, session), dir) in last {
        // Where the main agent works is a hint for its subagents: a failed write loses no event.
        let _ = working_tree::note_main_dir(home, &Agent::main_in(host, session), Path::new(dir));
    }
}

#[cfg(test)]
mod tests;
