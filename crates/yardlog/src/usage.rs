//! Usage counters preserve the difference between measured zero and unavailable data.

use loft_common::yaml::Out;
use serde_json::Value;

/// The counters of a normalized model request, in display order.
pub const COUNTERS: [&str; 4] = ["input", "cacheRead", "cacheWrite", "output"];

/// Recorded subtotals and which of their contributing requests left a counter unavailable.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Counts {
    values: [Option<u64>; 4],
    missing: u8,
}

impl Counts {
    /// Reads one request. A counter that is absent, or anything but a whole number, is
    /// unreported; usage that is not a mapping leaves every counter unreported.
    #[must_use]
    pub fn read(value: &Value) -> Self {
        let Some(values) = value.as_object() else {
            return Self::unreported();
        };
        let mut counts = Self::default();
        for (n, name) in COUNTERS.iter().enumerate() {
            counts.values[n] = values.get(*name).and_then(Value::as_u64);
            if counts.values[n].is_none() {
                counts.missing |= 1 << n;
            }
        }
        counts
    }

    /// One request for which no usage was available.
    #[must_use]
    pub const fn unreported() -> Self {
        Self {
            values: [None; 4],
            missing: 15,
        }
    }

    /// Whether any counter was actually measured, including a measured zero.
    #[must_use]
    pub fn measured(self) -> bool {
        self.values.iter().any(Option::is_some)
    }

    /// Adds a contribution, keeping recorded subtotals when coverage is partial.
    pub fn add(&mut self, other: Self) {
        for (n, right) in other.values.iter().enumerate() {
            self.values[n] = match (self.values[n], right) {
                (Some(left), Some(right)) => Some(left.saturating_add(*right)),
                (left, right) => left.or(*right),
            };
        }
        self.missing |= other.missing;
    }

    /// Canonical fields: no invented zero, and an explicit list of partial counters.
    #[must_use]
    pub fn fields(self) -> Vec<(&'static str, Out)> {
        let mut fields = Vec::new();
        let mut partial = Vec::new();
        for (n, (name, value)) in COUNTERS.iter().zip(self.values).enumerate() {
            let Some(value) = value else {
                continue;
            };
            fields.push((*name, Out::Integer(value)));
            if self.missing & (1 << n) != 0 {
                partial.push(Out::Token((*name).to_owned()));
            }
        }
        if !partial.is_empty() {
            fields.push(("partial", Out::Sequence(partial)));
        }
        fields
    }
}
