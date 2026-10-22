//! `loft yard` (the `loft yard` commands Feature): Yard Client's one way in. Each command acts
//! on the Yard Log of the branch that the worktree holding its directory is on, but `protocol`,
//! which needs no repository, and `upgrade`, which acts on every branch's under the tree. None
//! sends a command event or asks for an update.

mod answer;
mod answer_input;
pub mod args;
mod branch;
mod caller;
mod change;
mod close;
mod failure;
mod input;
pub mod merge;
mod page;
mod protocol;
mod published;
mod read;
mod render;
mod render_loft;
mod render_log;
mod sessions;
mod standing;
mod upgrade;
mod views;
mod wait;
mod withdraw;
mod write;

use std::path::Path;

use loft_ports::clock::{Clock as _, SystemClock};
use yardlog::Time;

use crate::finish::Finished;
use args::YardAction;
use failure::Failure;

/// Runs a `loft yard` command in `root`.
pub fn run(root: &Path, action: YardAction) -> Finished {
    match action {
        YardAction::Read(args) => read::run(root, args),
        YardAction::Write(args) => write::run(root, args),
        YardAction::Withdraw(args) => withdraw::run(root, args),
        YardAction::Wait(args) => wait::run(root, args),
        YardAction::Close(args) => close::run(root, args),
        YardAction::Published(args) => published::run(root, args),
        YardAction::Answer(args) => answer::run(root, args),
        YardAction::Protocol { output } => protocol::run(output.json()),
        YardAction::Upgrade(args) => upgrade::run(root, &args),
    }
}

/// The time now, as the clock gives it: `SOURCE_DATE_EPOCH` where it is set.
fn now() -> Result<Time, Failure> {
    let seconds = SystemClock::from_env()
        .now()
        .map_err(|error| Failure::one("clock", error.to_string()))?;
    Time::from_unix_seconds(seconds).map_err(|error| Failure::one("clock", error.to_string()))
}
