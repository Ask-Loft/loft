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

    /// Reads what [`Counts::fields`] wrote, as JSON: a counter that is absent, or anything but a
    /// whole number, is unavailable, and each counter `partial` names was left unavailable by
    /// some contribution. Anything but a mapping leaves every counter unavailable.
    #[must_use]
    pub fn from_fields(value: &Value) -> Self {
        let mut counts = Self::read(value);
        let partial = value.get("partial").and_then(Value::as_array);
        for name in partial.into_iter().flatten().filter_map(Value::as_str) {
            if let Some(n) = COUNTERS.iter().position(|counter| *counter == name) {
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

    /// Whether some counter is unavailable or was left unavailable by a contribution.
    #[must_use]
    pub fn partial(self) -> bool {
        self.missing != 0 || self.values.iter().any(Option::is_none)
    }

    /// The sum of the counters recorded.
    #[must_use]
    pub fn total(self) -> u64 {
        self.values
            .iter()
            .flatten()
            .fold(0, |sum, value| sum.saturating_add(*value))
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

    /// The fields of [`Counts::fields`] as JSON.
    #[must_use]
    pub fn json(self) -> Value {
        let mut fields = serde_json::Map::new();
        let mut partial = Vec::new();
        for (n, (name, value)) in COUNTERS.iter().zip(self.values).enumerate() {
            let Some(value) = value else {
                continue;
            };
            fields.insert((*name).to_owned(), Value::from(value));
            if self.missing & (1 << n) != 0 {
                partial.push(Value::from(*name));
            }
        }
        if !partial.is_empty() {
            fields.insert("partial".to_owned(), Value::Array(partial));
        }
        Value::Object(fields)
    }
}
