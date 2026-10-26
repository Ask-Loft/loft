//! Who may wait on a submission (the Wait for the answers Feature): the agent it counts as sent
//! by, after following continuations; or the main agent of another session, which takes it up,
//! where the session that sent it has stopped, no session continues it and no wait holds it. A
//! subagent waits only on its own submissions, and a main agent never on its own subagent's.

use serde_json::Value;
use yardlog::log::Submission;
use yardlog::{Actor, Agent, Time};
use yardlog_store::home::Home;

use super::caller::Continuations;
use super::failure;
use super::sessions;

/// How the calling agent stands to a submission.
pub enum Standing {
    /// It counts as sent by the calling agent.
    Own,
    /// The calling main agent may take it up from a session that stopped.
    TakeUp,
    /// The calling agent may not wait on it, and why.
    Refused(Value),
}

/// How `caller` stands to `submission` at `now`.
pub fn standing(
    submission: &Submission,
    caller: &Agent,
    continuations: &Continuations,
    home: Option<&Home>,
    now: Time,
) -> Result<Standing, failure::Failure> {
    let sender = submission.sender();
    let Some(sending) = sender.agent() else {
        return Ok(Standing::Refused(not_sender(submission, sender)));
    };
    let mine = continuations.identity(caller);
    let theirs = continuations.identity(sending);
    if mine == theirs {
        return Ok(Standing::Own);
    }
    if mine.is_subagent() || mine.same_session(&theirs) {
        return Ok(Standing::Refused(not_sender(submission, sender)));
    }
    if let Some(next) = continuations.continued_by(&sending.host, &sending.session) {
        let message = format!(
            "submission {} was sent by {sender}, which session {next} continues",
            submission.key
        );
        return Ok(Standing::Refused(failure::error("continued", &message)));
    }
    let last = match home {
        Some(home) => sessions::last_heard(
            home,
            &yardlog::identity::session_key(&sending.host, &sending.session),
        )?,
        None => None,
    };
    if !sessions::stopped(last, now) {
        let message = format!(
            "submission {} was sent by {sender}, which was heard within the last {} seconds",
            submission.key,
            sessions::STOPPED_AFTER.as_secs()
        );
        return Ok(Standing::Refused(failure::error("heard", &message)));
    }
    Ok(Standing::TakeUp)
}

fn not_sender(submission: &Submission, sender: &Actor) -> Value {
    let message = format!(
        "submission {} was sent by {sender}; an agent waits only on the submissions it sent",
        submission.key
    );
    failure::error("not-sender", &message)
}
