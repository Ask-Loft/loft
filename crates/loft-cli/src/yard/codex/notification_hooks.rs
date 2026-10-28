//! Native lifecycle is authoritative for arming, cancellation and receipts of our own nudges.

use super::commands::Caller;
use super::notification_state::{Ledger, Lifecycle, Route};
use super::translate::{Hook, HookKind};

#[derive(PartialEq, Eq)]
pub enum Disposition {
    Ordinary,
    Notification,
    Cancelled,
}

pub fn record(hook: &Hook, caller: Option<&Caller>, ledger: &mut Ledger) -> Disposition {
    let Some(caller) = caller else {
        return Disposition::Ordinary;
    };
    let thread = caller.thread();
    let route = Route {
        caller: caller.clone(),
        thread: thread.into(),
        directory: hook.dir().into(),
    };
    let event = match hook.kind() {
        HookKind::SessionStart | HookKind::SubagentStart => Lifecycle::Start,
        HookKind::UserPromptSubmit => {
            let prompt = hook.0["prompt"].as_str().unwrap_or_default();
            match ledger.receive_prompt(thread, prompt) {
                Some(true) => return Disposition::Notification,
                Some(false) => return Disposition::Cancelled,
                None => Lifecycle::UserPrompt,
            }
        }
        HookKind::Interrupt | HookKind::SubagentStop => Lifecycle::Disarm,
        HookKind::SessionEnd => Lifecycle::EndSession,
        _ => Lifecycle::Other,
    };
    ledger.observe(route, event);
    Disposition::Ordinary
}
