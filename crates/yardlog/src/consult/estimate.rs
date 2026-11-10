//! The estimate of the context tokens each class of read brought in. A model request's prompt
//! holds everything the agent's previous request held, that request's output, and what came back
//! from the tool calls that ended between the two: so the growth of the prompt, less the previous
//! output, is shared among those calls, equally, and each call's share among its classes. A call
//! that ran a `loft` read counts toward the specification; one that holds no class is
//! unclassified. A cache write a request does not report counts as none, as a host that keeps no
//! cache-write count writes none.

use std::collections::HashMap;

use super::{Call, Class};
use crate::totals::Who;
use crate::usage::Counts;

/// The context tokens each class of read is estimated to have brought in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Estimate {
    pub spec: u64,
    pub code: u64,
    pub other: u64,
    pub unclassified: u64,
}

impl Estimate {
    /// The names the classes are written under, in the order of [`Estimate::values`].
    pub(super) const NAMES: [&str; 4] = ["spec", "code", "other", "unclassified"];

    pub(super) const fn values(self) -> [u64; 4] {
        [self.spec, self.code, self.other, self.unclassified]
    }

    pub(super) const fn from_values(values: [u64; 4]) -> Self {
        let [spec, code, other, unclassified] = values;
        Self {
            spec,
            code,
            other,
            unclassified,
        }
    }

    pub fn add(&mut self, other: Self) {
        let mut values = self.values();
        for (value, more) in values.iter_mut().zip(other.values()) {
            *value += more;
        }
        *self = Self::from_values(values);
    }

    /// Adds `tokens` to `class`, or to the unclassified where there is none.
    fn put(&mut self, class: Option<Class>, tokens: u64) {
        let slot = match class {
            Some(Class::Spec) => &mut self.spec,
            Some(Class::Code) => &mut self.code,
            Some(Class::Other) => &mut self.other,
            None => &mut self.unclassified,
        };
        *slot += tokens;
    }
}

/// The classes a call's share of tokens goes to, each once: none where it holds no class.
fn classes(call: &Call<'_>) -> Vec<Option<Class>> {
    let mut classes: Vec<Option<Class>> = call
        .reads
        .iter()
        .chain(call.edits)
        .copied()
        .map(Some)
        .collect();
    if call.loft_read {
        classes.push(Some(Class::Spec));
    }
    if classes.is_empty() {
        classes.push(None);
    }
    classes.sort_unstable();
    classes.dedup();
    classes
}

/// `total` shared among `parts` as equally as whole tokens allow, the first parts taking what is
/// left over.
fn shares(total: u64, parts: usize) -> impl Iterator<Item = u64> {
    let parts = u64::try_from(parts).unwrap_or(u64::MAX).max(1);
    let (base, rest) = (total / parts, total % parts);
    (0..parts).map(move |n| base + u64::from(n < rest))
}

/// What an agent's last request left: its prompt and output, or that it lacked a count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Last {
    Known { prompt: u64, output: u64 },
    Unknown,
}

impl Last {
    fn of(tokens: Option<Counts>) -> Self {
        let Some(tokens) = tokens else {
            return Self::Unknown;
        };
        match (tokens.input(), tokens.cache_read(), tokens.output()) {
            (Some(input), Some(read), Some(output)) => Self::Known {
                prompt: input + read + tokens.cache_write().unwrap_or_default(),
                output,
            },
            _ => Self::Unknown,
        }
    }
}

/// What one agent's estimate waits on: its last request, the calls ended since, and how many
/// requests it made.
#[derive(Debug, Clone, Default)]
struct Agent {
    last: Option<Last>,
    ended: Vec<Vec<Option<Class>>>,
    requests: u64,
}

/// A call started and not yet ended: its agent, how many requests its agent had made when it
/// started, and its classes.
#[derive(Debug, Clone)]
struct Open {
    who: Who,
    requests: u64,
    classes: Vec<Option<Class>>,
}

/// What the estimate needs between events: the calls started and not yet ended, by their id,
/// and each agent's state. No file holds it: replaying a session's events rebuilds it.
#[derive(Debug, Clone, Default)]
pub(super) struct Live {
    calls: HashMap<String, Open>,
    agents: HashMap<Who, Agent>,
}

impl Live {
    pub(super) fn started(&mut self, call: &Call<'_>) {
        let requests = self.agents.get(call.who).map_or(0, |agent| agent.requests);
        let open = Open {
            who: call.who.clone(),
            requests,
            classes: classes(call),
        };
        self.calls.insert(call.id.to_owned(), open);
    }

    pub(super) fn ended(&mut self, who: &Who, id: &str) {
        let Some(open) = self.calls.remove(id) else {
            return;
        };
        // A call ends in the agent that started it; an end that names another is not its own.
        if &open.who == who {
            let agent = self.agents.entry(open.who).or_default();
            agent.ended.push(open.classes);
        }
    }

    /// What the request of `who` with `tokens` attributes, and whether it left out a request
    /// that lacked a count.
    pub(super) fn requested(&mut self, who: &Who, tokens: Option<Counts>) -> (Estimate, bool) {
        let agent = self.agents.entry(who.clone()).or_default();
        // A call of the agent still open across a whole request never ended: one the person
        // refused or interrupted. It is let go, so that what the page server keeps stays bounded.
        let made = agent.requests;
        self.calls
            .retain(|_, open| &open.who != who || open.requests >= made);
        agent.requests += 1;
        let ended = std::mem::take(&mut agent.ended);
        let now = Last::of(tokens);
        let before = agent.last.replace(now);
        let (
            Some(Last::Known {
                prompt: was,
                output,
            }),
            Last::Known { prompt, .. },
        ) = (before, now)
        else {
            let partial = now == Last::Unknown || before == Some(Last::Unknown);
            return (Estimate::default(), partial);
        };
        let mut estimate = Estimate::default();
        let growth = prompt.saturating_sub(was.saturating_add(output));
        if growth == 0 || ended.is_empty() {
            return (estimate, false);
        }
        for (share, classes) in shares(growth, ended.len()).zip(&ended) {
            for (tokens, class) in shares(share, classes.len()).zip(classes) {
                estimate.put(*class, tokens);
            }
        }
        (estimate, false)
    }
}
