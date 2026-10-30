//! A change an agent makes to Yard Log other than writing blocks: withdrawing a submission or
//! closing Yard Log. Each is applied under Yard Log's lock, settled first, and refused, with
//! nothing written, where Yard Log's rules do not allow it.

use std::path::Path;

use serde_json::Value;
use yardlog::respond::Refused;
use yardlog::{Agent, Time, YardLog};

use super::args::{Beyond, Writer};
use super::branch::{self, Branch, Unapplied};
use super::caller::Caller;
use super::failure::{self, Failure};
use super::working_tree::{Follows, target, wrote};

/// Applies `change` to the branch's Yard Log for the calling agent at the time the clock gives,
/// and gives the result's fields at the revision it returns.
pub fn apply(
    dir: &Path,
    writer: Writer,
    beyond: Beyond,
    follows: Follows,
    change: impl FnOnce(&mut YardLog, Agent, Time) -> Result<u64, Refused>,
) -> Result<Value, Failure> {
    let named = writer.tree.clone();
    let caller = Caller::new(&writer.into());
    let dir = target(dir, named.as_deref(), caller.agent.as_ref(), follows)?;
    let branch = Branch::locate(&dir, beyond)?;
    let at = super::now()?;
    let agent = caller.agent().clone();
    let settler = agent.clone();
    let held = branch.open(at, || Ok(yardlog::Actor::Agent(settler)))?;
    branch.refuse_if_merged(&held)?;
    let applied = branch.update(|current| {
        let mut log = current.ok_or(Unapplied::Refused(branch::absent()))?;
        let revision =
            change(&mut log, agent, at).map_err(|refused| Unapplied::Refused(refusal(&refused)))?;
        Ok((log, revision))
    })?;
    match applied {
        Ok(revision) => {
            wrote(caller.agent(), &branch.place().tree, at);
            Ok(branch.fields(revision))
        }
        Err(error) => Err(branch.refused(&branch.store.read()?, vec![error])),
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
