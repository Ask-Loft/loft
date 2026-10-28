//! Durable notification metadata and cancellation generations; time comes from the clock port.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::commands::Caller;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Route {
    pub caller: Caller,
    pub thread: String,
    pub directory: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resolution {
    pub log: String,
    pub submission: String,
    pub revision: u64,
    pub tree: String,
    pub branch: String,
    pub thread: String,
}

impl Resolution {
    pub fn marker(&self) -> String {
        format!("yard-decision-{}-{}", self.log, self.submission)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Availability {
    Pending,
    Available,
    #[default]
    Unavailable,
    Uncertain,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Progress {
    Queued,
    Uncertain,
    Accepted,
    Read,
    Cancelled,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Watcher {
    pub route: Route,
    pub armed: bool,
    pub generation: u64,
    pub availability: Availability,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Ticket {
    pub resolution: Resolution,
    pub generation: u64,
    pub progress: Progress,
    pub retry_at: u64,
}

#[derive(Clone, Debug)]
pub struct Attempt {
    pub marker: String,
    pub resolution: Resolution,
    pub route: Route,
    pub generation: u64,
    pub needs_receipt: bool,
}

#[derive(Clone, Copy)]
pub enum Lifecycle {
    Start,
    UserPrompt,
    Notification,
    TurnEnd,
    Interrupt,
    EndAgent,
    EndSession,
}

#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Ledger {
    pub callers: BTreeMap<String, Watcher>,
    pub deliveries: BTreeMap<String, Ticket>,
}

impl Ledger {
    pub fn observe(&mut self, route: Route, event: Lifecycle) {
        let thread = route.thread.clone();
        self.callers.entry(thread.clone()).or_insert(Watcher {
            route: route.clone(),
            armed: matches!(event, Lifecycle::Start | Lifecycle::UserPrompt),
            generation: 0,
            availability: Availability::Pending,
        });
        match event {
            Lifecycle::Interrupt | Lifecycle::EndAgent => self.disarm(&thread),
            Lifecycle::EndSession => self.end_session(&route.caller.session),
            Lifecycle::UserPrompt => {
                if self
                    .callers
                    .get(&thread)
                    .is_some_and(|w| !w.armed || w.route != route)
                {
                    self.disarm(&thread);
                }
                let watcher = self.callers.get_mut(&thread).unwrap();
                watcher.route = route;
                watcher.armed = true;
                watcher.availability = Availability::Pending;
            }
            Lifecycle::Start | Lifecycle::Notification | Lifecycle::TurnEnd => {}
        }
    }

    pub fn offer(&mut self, resolution: Resolution, own_read: Option<u64>) -> Option<String> {
        let watcher = self.callers.get(&resolution.thread)?;
        if !watcher.armed {
            return None;
        }
        let marker = resolution.marker();
        if self
            .deliveries
            .get(&marker)
            .is_some_and(|t| t.resolution.thread != resolution.thread)
        {
            return None;
        }
        if own_read.is_some_and(|revision| revision >= resolution.revision) {
            if let Some(ticket) = self.deliveries.get_mut(&marker) {
                ticket.progress = Progress::Read;
            }
            return None;
        }
        let ticket = self.deliveries.entry(marker.clone()).or_insert(Ticket {
            resolution,
            generation: watcher.generation,
            progress: Progress::Queued,
            retry_at: 0,
        });
        if ticket.progress == Progress::Cancelled && ticket.generation != watcher.generation {
            ticket.generation = watcher.generation;
            ticket.progress = Progress::Queued;
            ticket.retry_at = 0;
        }
        matches!(ticket.progress, Progress::Queued | Progress::Uncertain).then_some(marker)
    }

    pub fn claim(&mut self, marker: &str, now: u64) -> Option<Attempt> {
        let ticket = self.deliveries.get_mut(marker)?;
        let watcher = self.callers.get(&ticket.resolution.thread)?;
        if !watcher.armed
            || watcher.generation != ticket.generation
            || now < ticket.retry_at
            || !matches!(ticket.progress, Progress::Queued | Progress::Uncertain)
        {
            return None;
        }
        let needs_receipt = ticket.progress == Progress::Uncertain;
        ticket.progress = Progress::Uncertain;
        ticket.retry_at = now.saturating_add(5);
        Some(Attempt {
            marker: marker.into(),
            resolution: ticket.resolution.clone(),
            route: watcher.route.clone(),
            generation: watcher.generation,
            needs_receipt,
        })
    }

    pub fn current(&self, attempt: &Attempt) -> bool {
        self.callers
            .get(&attempt.route.thread)
            .is_some_and(|w| w.armed && w.generation == attempt.generation)
            && self.deliveries.get(&attempt.marker).is_some_and(|t| {
                t.generation == attempt.generation && t.progress == Progress::Uncertain
            })
    }

    pub fn confirm(&mut self, attempt: &Attempt) {
        if self.current(attempt) {
            self.deliveries.get_mut(&attempt.marker).unwrap().progress = Progress::Accepted;
        }
    }

    pub fn refuse(&mut self, attempt: &Attempt) {
        if self.current(attempt) {
            self.deliveries.get_mut(&attempt.marker).unwrap().progress = Progress::Queued;
        }
    }

    pub fn cancel_log(&mut self, log: &str) {
        for ticket in self
            .deliveries
            .values_mut()
            .filter(|t| t.resolution.log == log)
        {
            cancel(ticket);
        }
    }

    pub fn report(&mut self, thread: &str, availability: Availability) {
        if let Some(watcher) = self.callers.get_mut(thread) {
            watcher.availability = availability;
        }
    }

    pub fn is_notification(&self, thread: &str, prompt: &str) -> bool {
        prompt.lines().next().is_some_and(|marker| {
            self.deliveries
                .get(marker)
                .is_some_and(|ticket| ticket.resolution.thread == thread)
        })
    }

    /// A native prompt hook is a receipt too, even if the RPC acknowledgment is still in flight.
    pub fn receive_prompt(&mut self, thread: &str, prompt: &str) -> bool {
        let Some(marker) = prompt.lines().next() else {
            return false;
        };
        let Some(watcher) = self.callers.get_mut(thread).filter(|w| w.armed) else {
            return false;
        };
        let Some(ticket) = self.deliveries.get_mut(marker).filter(|t| {
            t.resolution.thread == thread
                && t.generation == watcher.generation
                && t.progress != Progress::Cancelled
        }) else {
            return false;
        };
        if ticket.progress != Progress::Read {
            ticket.progress = Progress::Accepted;
        }
        watcher.availability = Availability::Available;
        true
    }

    pub fn cancel_marker(&mut self, marker: &str) {
        if let Some(ticket) = self.deliveries.get_mut(marker) {
            cancel(ticket);
        }
    }

    pub fn availability(&self, thread: &str) -> Availability {
        let Some(watcher) = self.callers.get(thread).filter(|w| w.armed) else {
            return Availability::Unavailable;
        };
        if self
            .deliveries
            .values()
            .any(|t| t.resolution.thread == thread && t.progress == Progress::Uncertain)
        {
            Availability::Uncertain
        } else {
            watcher.availability
        }
    }

    fn disarm(&mut self, thread: &str) {
        if let Some(watcher) = self.callers.get_mut(thread) {
            watcher.armed = false;
            watcher.generation += 1;
            watcher.availability = Availability::Unavailable;
        }
        for ticket in self
            .deliveries
            .values_mut()
            .filter(|t| t.resolution.thread == thread)
        {
            cancel(ticket);
        }
    }

    fn end_session(&mut self, session: &str) {
        let threads: Vec<_> = self
            .callers
            .iter()
            .filter(|(_, w)| w.route.caller.session == session)
            .map(|(id, _)| id.clone())
            .collect();
        for thread in threads {
            self.disarm(&thread);
        }
    }
}

fn cancel(ticket: &mut Ticket) {
    if !matches!(ticket.progress, Progress::Accepted | Progress::Read) {
        ticket.progress = Progress::Cancelled;
    }
}

#[cfg(test)]
#[path = "notification_state_tests.rs"]
mod tests;
