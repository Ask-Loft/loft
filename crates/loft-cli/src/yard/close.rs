//! `loft yard close` (the Close Yard Log Feature): marks the branch's Yard Log closed with the
//! Act of the close, withdrawing every open submission; closed Yard Log takes no more writes or
//! answers.

use std::path::Path;

use super::args::CloseArgs;
use super::{change, failure};
use crate::finish::Finished;

const SCHEMA: &str = "loft-yard-close/1";
const OUTCOME: &str = "closed";

pub fn run(root: &Path, args: CloseArgs) -> Finished {
    let applied = change::apply(root, args.writer, args.beyond, |log, agent, at| {
        log.close(agent, at)
    });
    failure::end(
        SCHEMA,
        args.output.json(),
        applied.map(|fields| (OUTCOME, fields)),
        |_, fields| format!("Closed Yard Log at revision {}", fields["revision"]),
    )
}
