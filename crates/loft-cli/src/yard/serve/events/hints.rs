//! What a batch of events tells the machine beside the session's files: that each session is
//! live, and where each session's main agent works. Both are hints, which an event the person
//! made, such as an approval's, never gives.

use std::collections::HashMap;
use std::path::Path;

use yardlog::Agent;
use yardlog_store::home::Home;

use crate::yard::event::{Event, Who};
use crate::yard::working_tree;

/// Records each session a batch holds events of as heard: an event a mod posts tells the page
/// server its session is live, as its beat does.
pub(super) fn hear(home: &Home, events: &[Event]) {
    let Ok(at) = crate::yard::now() else {
        return;
    };
    let mut sessions: Vec<String> = events
        .iter()
        .filter(|event| !event.by_person())
        .map(Event::session_key)
        .collect();
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
pub(super) fn note_main_dirs(home: &Home, events: &[Event]) {
    let mut last: HashMap<(&str, &str), &str> = HashMap::new();
    for event in events
        .iter()
        .filter(|event| matches!(event.who, Who::Main) && !event.by_person())
    {
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
