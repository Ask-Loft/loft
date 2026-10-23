//! `loft yard withdraw` (the Withdraw a submission Feature): marks an open submission
//! `withdrawn` and leaves its blocks unanswered; a submission that is not open is refused with
//! its state.

use std::path::Path;

use serde_json::json;
use yardlog::{Key, SubmissionKey};

use super::args::WithdrawArgs;
use super::change;
use super::failure::{self, Failure};
use crate::finish::Finished;

const SCHEMA: &str = "loft-yard-withdraw/1";
const OUTCOME: &str = "withdrawn";

pub fn run(root: &Path, args: WithdrawArgs) -> Finished {
    let as_json = args.output.json();
    let submission = match Key::parse(&args.submission) {
        Ok(key) => SubmissionKey::new(key),
        Err(error) => {
            return Failure::Usage(format!("--submission: {error}")).finish(SCHEMA, as_json);
        }
    };
    let applied = change::apply(root, args.writer, args.beyond, |log, agent, at| {
        log.withdraw(submission, agent, at)
    })
    .map(|mut fields| {
        fields["submission"] = json!(submission.to_string());
        (OUTCOME, fields)
    });
    failure::end(SCHEMA, as_json, applied, |_, fields| {
        format!(
            "Withdrew {submission}; Yard Log at revision {}",
            fields["revision"]
        )
    })
}
