//! `loft yard withdraw` (the Withdraw a submission Feature): marks an open submission
//! `withdrawn` and leaves its blocks unanswered; a submission that is not open is refused with
//! its state.

use std::path::Path;

use serde_json::json;
use yardlog::{Key, SubmissionKey};

use super::args::WithdrawArgs;
use super::caller::Caller;
use super::change;
use super::failure::{self, Failure};
use super::working_tree::{Follows, sent_to};
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
    let mut writer = args.writer;
    if writer.tree.is_none() {
        let agent = Caller::new(&writer.clone().into()).agent().clone();
        match sent_to(root, &agent, |log| log.submission(submission).is_some()) {
            Ok(tree) => writer.tree = Some(tree),
            Err(failure) => return failure.finish(SCHEMA, as_json),
        }
    }
    let applied = change::apply(
        root,
        writer,
        args.beyond,
        Follows::Work,
        |log, agent, at| log.withdraw(submission, agent, at),
    )
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
