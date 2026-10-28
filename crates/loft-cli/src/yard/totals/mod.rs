//! A session's token totals as the page server counts them from its events, and the replay of
//! the session's event log on this machine, `~/.yard/events/<session id>.jsonl`, through which
//! the page server rebuilds what a session kept and `loft yard upgrade` counts a session again.

use std::collections::BTreeMap;

use serde_json::Value;
use yardlog::totals::{Request, Totals};
use yardlog_store::home::Home;

use super::event::{Event, strings};
use super::event_ids::{Receipt, Receipts};

/// Counts `event` into `totals` where it is a model request or a compaction, `stage` the stage it
/// counts toward and `open` the submissions open when it ended; says whether it counted.
pub fn count(totals: &mut Totals, event: &Event, stage: Option<&str>, open: &[String]) -> bool {
    let compaction = match event.kind.as_str() {
        "request" => false,
        "compaction" => true,
        _ => return false,
    };
    let model = event.model();
    let request = Request {
        host: &event.host,
        at: event.at,
        who: &event.who,
        model: &model,
        compaction,
        tokens: event.tokens(),
    };
    totals.apply(&request, stage, open)
}

/// One line of a session's event log, as the page server kept it: the event, with the tree and
/// branch of the Yard Log it was kept in, the stage it counted toward and the submissions open.
pub struct Logged<'a> {
    pub event: Event,
    pub tree: &'a str,
    pub branch: &'a str,
    pub stage: Option<&'a str>,
    pub open: &'a [String],
}

/// Replays session `key`'s event log in the order it was kept, giving `each` line once: a line
/// whose event was kept before, by its receipt, is skipped, as is a line that does not read.
/// Gives the receipts of the events replayed.
pub fn replay(home: &Home, key: &str, mut each: impl FnMut(Logged<'_>)) -> Receipts {
    let mut receipts = Receipts::default();
    let Ok(Some(bytes)) = home.read_events(key) else {
        return receipts;
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
        let Ok(receipt) = Receipt::replay(&line) else {
            continue;
        };
        if receipts.contains(receipt.as_ref()) != Ok(false) {
            continue;
        }
        receipts.keep(receipt.as_ref());
        let open = strings(&line["submissions"]);
        each(Logged {
            event,
            tree,
            branch,
            stage: line["stage"].as_str(),
            open: &open,
        });
    }
    receipts
}

/// Session `key`'s totals in each Yard Log its event log counts toward, by its tree and branch,
/// rebuilt from the log as the page server would count them.
pub fn replayed(home: &Home, key: &str) -> BTreeMap<(String, String), Totals> {
    let mut by_log: BTreeMap<(String, String), Totals> = BTreeMap::new();
    replay(home, key, |logged| {
        let place = (logged.tree.to_owned(), logged.branch.to_owned());
        let totals = by_log
            .entry(place)
            .or_insert_with(|| Totals::new(&logged.event.session));
        count(totals, &logged.event, logged.stage, logged.open);
    });
    by_log
}

#[cfg(test)]
mod tests;
