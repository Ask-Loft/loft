//! `loft yard close` (the Close Yard Log Feature): marks the branch's Yard Log closed with the
//! Act of the close, withdrawing every open submission; closed Yard Log takes no more writes or
//! answers.

use std::path::Path;

use super::args::CloseArgs;
use super::change;
use crate::finish::Finished;
use crate::json;
use crate::out::outln;

const SCHEMA: &str = "loft-yard-close/1";
const OUTCOME: &str = "closed";

pub fn run(root: &Path, args: CloseArgs) -> Finished {
    let as_json = args.output.json();
    match change::apply(root, args.writer, args.beyond, |log, agent, at| {
        log.close(agent, at)
    }) {
        Ok(fields) => {
            if as_json {
                json::print(&json::result(SCHEMA, OUTCOME, fields, Vec::new()));
            } else {
                outln!("Closed Yard Log at revision {}", fields["revision"]);
            }
            Finished::success(OUTCOME)
        }
        Err(failure) => failure.finish(SCHEMA, as_json),
    }
}
