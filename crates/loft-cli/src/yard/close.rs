//! `loft yard close` (the Close Yard Log Feature): marks the branch's Yard Log closed with the
//! Act of the close, withdrawing every open submission; closed Yard Log takes no more writes or
//! answers.

use std::path::Path;

use yardlog::YardLog;

use super::args::CloseArgs;
use super::working_tree::Follows;
use super::{change, failure};
use crate::finish::Finished;

const SCHEMA: &str = "loft-yard-close/1";
const OUTCOME: &str = "closed";

pub fn run(root: &Path, args: CloseArgs) -> Finished {
    // A close stays in its directory: ending the last stage of another worktree never turns it
    // toward the agent's own branch.
    let applied = change::apply(
        root,
        args.writer,
        args.beyond,
        Follows::Directory,
        YardLog::close,
    );
    failure::end(
        SCHEMA,
        args.output.json(),
        applied.map(|fields| (OUTCOME, fields)),
        |_, fields| format!("Closed Yard Log at revision {}", fields["revision"]),
    )
}
