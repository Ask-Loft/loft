//! The estimate of the context tokens each class of read brought in. A model request's prompt
//! holds everything the agent's previous request held, that request's output, and what came back
//! from the tool calls that ended between the two: so the growth of the prompt, less the previous
//! output, is shared among those calls, equally, and each call's share among its classes. A call
//! that ran a `loft` read counts toward the specification; one that holds no class is
//! unclassified. A cache write a request does not report counts as none, as a host that keeps no
//! cache-write count writes none.

use std::collections::HashMap;

use loft_common::yaml::Out;
use serde_json::Value;

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

/// Where a share of a call's tokens goes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Bucket {
    Spec,
    Code,
    Other,
    Unclassified,
}

impl Estimate {
    pub fn add(&mut self, other: Self) {
        self.spec += other.spec;
        self.code += other.code;
        self.other += other.other;
        self.unclassified += other.unclassified;
    }

    fn put(&mut self, bucket: Bucket, tokens: u64) {
        match bucket {
            Bucket::Spec => self.spec += tokens,
            Bucket::Code => self.code += tokens,
            Bucket::Other => self.other += tokens,
            Bucket::Unclassified => self.unclassified += tokens,
        }
    }

    /// The estimate's fields, each class that took any tokens.
    #[must_use]
    pub fn fields(self) -> Vec<(&'static str, Out)> {
        [
            ("spec", self.spec),
            ("code", self.code),
            ("other", self.other),
            ("unclassified", self.unclassified),
        ]
        .into_iter()
        .filter(|(_, tokens)| *tokens > 0)
        .map(|(name, tokens)| (name, Out::Integer(tokens)))
        .collect()
    }

    /// Reads what [`Estimate::fields`] wrote, as JSON.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        let field = |name: &str| value[name].as_u64().unwrap_or_default();
        Self {
            spec: field("spec"),
            code: field("code"),
            other: field("other"),
            unclassified: field("unclassified"),
        }
    }
}

/// The buckets a call's share of tokens goes to, each once.
fn buckets(call: &Call<'_>) -> Vec<Bucket> {
    let mut buckets: Vec<Bucket> = call
        .reads
        .iter()
        .chain(call.edits)
        .map(|class| match class {
            Class::Spec => Bucket::Spec,
            Class::Code => Bucket::Code,
            Class::Other => Bucket::Other,
        })
        .collect();
    if call.loft_read {
        buckets.push(Bucket::Spec);
    }
    if buckets.is_empty() {
        buckets.push(Bucket::Unclassified);
    }
    buckets.sort_unstable();
    buckets.dedup();
    buckets
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
        match (
            tokens.get("input"),
            tokens.get("cacheRead"),
            tokens.get("output"),
        ) {
            (Some(input), Some(read), Some(output)) => Self::Known {
                prompt: input + read + tokens.get("cacheWrite").unwrap_or_default(),
                output,
            },
            _ => Self::Unknown,
        }
    }
}

/// What one agent's estimate waits on: its last request, and the calls ended since.
#[derive(Debug, Clone, Default)]
struct Agent {
    last: Option<Last>,
    ended: Vec<Vec<Bucket>>,
}

/// What the estimate needs between events: the calls started and not yet ended, by their id,
/// and each agent's state. No file holds it: replaying a session's events rebuilds it.
#[derive(Debug, Clone, Default)]
pub(super) struct Live {
    calls: HashMap<String, (Who, Vec<Bucket>)>,
    agents: HashMap<Who, Agent>,
}

impl Live {
    pub(super) fn started(&mut self, call: &Call<'_>) {
        self.calls
            .insert(call.id.to_owned(), (call.who.clone(), buckets(call)));
    }

    pub(super) fn ended(&mut self, who: &Who, id: &str) {
        let Some((started_by, buckets)) = self.calls.remove(id) else {
            return;
        };
        // A call ends in the agent that started it; an end that names another is not its own.
        if &started_by == who {
            self.agents
                .entry(started_by)
                .or_default()
                .ended
                .push(buckets);
        }
    }

    /// What the request of `who` with `tokens` attributes, and whether it left out a request
    /// that lacked a count.
    pub(super) fn requested(&mut self, who: &Who, tokens: Option<Counts>) -> (Estimate, bool) {
        let agent = self.agents.entry(who.clone()).or_default();
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
        for (share, buckets) in shares(growth, ended.len()).zip(&ended) {
            for (tokens, bucket) in shares(share, buckets.len()).zip(buckets) {
                estimate.put(*bucket, tokens);
            }
        }
        (estimate, false)
    }
}
