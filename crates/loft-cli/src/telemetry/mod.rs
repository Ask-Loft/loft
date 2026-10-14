//! Telemetry: one command event at the end of each run of a command that sends one, on until a
//! person switches it off. The switches are read before a token is read or written and before a
//! connection is opened; the notice is printed before the first event leaves the machine; and
//! nothing about sending ever reaches standard output or standard error.

mod event;
mod notice;
mod tokens;

use std::path::Path;
use std::time::Instant;

use crate::finish::Finished;
use crate::network::{self, Job};
use crate::out::errln;
use crate::switches::TELEMETRY;
use crate::{endpoints, settings};

/// The work that sends the event of a run of `command` that started at `started` and finished
/// as `finished` in `root`, or none while telemetry is off.
pub fn prepare(command: &str, finished: &Finished, started: Instant, root: &Path) -> Option<Job> {
    let duration = started.elapsed();
    let state = TELEMETRY.state();
    if !state.on {
        return None;
    }
    let address = endpoints::collector();
    let mut kept = settings::read(TELEMETRY.file);
    let (installation, drawn) = tokens::installation(&mut kept);
    let unseen = !kept["noticeShown"].as_bool().unwrap_or(false);
    if unseen {
        errln!("{}", notice::text(&address).trim_end());
        kept["noticeShown"] = serde_json::json!(true);
    }
    if drawn || unseen {
        // A setting that cannot be kept is drawn or printed again on the next run.
        let _ = settings::write(TELEMETRY.file, &kept);
    }
    let checkout = tokens::checkout(root);
    let event = event::build(
        command,
        finished,
        duration,
        &installation,
        checkout.as_ref(),
        state.internal,
    )?;
    let body = serde_json::to_vec(&event).expect("a JSON value serializes");
    Some(Box::new(move |left| {
        let _ = network::post_json(&address, &body, left);
    }))
}
