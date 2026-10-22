//! A change an agent makes to Yard Log other than writing blocks: withdrawing a submission or
//! closing Yard Log. Each is applied under Yard Log's lock, settled first, and refused, with
//! nothing written, where Yard Log's rules do not allow it.

use std::path::Path;

use serde_json::Value;
use yardlog::respond::Refused;
use yardlog::{Agent, Time, YardLog};
use yardlog_store::YardStoreError;
use yardlog_store::home::Home;

use super::args::{Beyond, Writer};
use super::branch::{Branch, revision_of};
use super::caller::Caller;
use super::failure::{self, Failure};
use super::write::closed_by_merge;

/// Why a change did not apply.
enum Unapplied {
    Store(YardStoreError),
    Absent,
    Refused(Refused),
}

impl From<YardStoreError> for Unapplied {
    fn from(error: YardStoreError) -> Self {
        Self::Store(error)
    }
}

/// Applies `change` to the branch's Yard Log for the calling agent at the time the clock gives,
/// and gives the result's fields at the revision it returns.
pub fn apply(
    dir: &Path,
    writer: Writer,
    beyond: Beyond,
    change: impl FnOnce(&mut YardLog, Agent, Time) -> Result<u64, Refused>,
) -> Result<Value, Failure> {
    let caller = Caller::new(writer.into());
    let branch = Branch::locate(dir, beyond)?;
    caller.record_continuation(Home::from_env().as_ref())?;
    let at = super::now()?;
    let agent = caller.agent().clone();
    let settler = agent.clone();
    let held = branch.open(at, || Ok(yardlog::Actor::Agent(settler)))?;
    if super::branch::log_of(&held).is_some_and(|log| !log.is_closed())
        && branch.closed_by_merge()?
    {
        return Err(Failure::Refused {
            fields: branch.fields(revision_of(&held)),
            errors: vec![closed_by_merge()],
        });
    }
    let applied = branch.store.update(|current| {
        let mut log = current.ok_or(Unapplied::Absent)?;
        let revision = change(&mut log, agent, at).map_err(Unapplied::Refused)?;
        Ok::<_, Unapplied>((log, revision))
    });
    match applied {
        Ok(updated) => Ok(branch.fields(updated.value)),
        Err(Unapplied::Store(error)) => Err(error.into()),
        Err(unapplied) => {
            let held = branch.store.read()?;
            let error = match unapplied {
                Unapplied::Refused(refused) => refusal(&refused),
                _ => failure::error("absent", "the branch has no Yard Log"),
            };
            Err(Failure::Refused {
                fields: branch.fields(revision_of(&held)),
                errors: vec![error],
            })
        }
    }
}

/// The error a refused change reports.
pub fn refusal(refused: &Refused) -> Value {
    let kind = match refused {
        Refused::Closed => "closed",
        Refused::NoBlock(_) => "no-block",
        Refused::Invalid { .. } => "invalid",
        Refused::NoSubmission(_) => "no-submission",
        Refused::NotOpen { .. } => "not-open",
        Refused::NotMain => "not-main",
    };
    failure::error(kind, &refused.to_string())
}
