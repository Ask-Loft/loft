//! The format a Yard Log is written in, `yardlog/<n>`, which its root names. A `loft` reads its
//! own format and every older one, writes only its own, and never writes one newer than it.

use std::fmt;

/// A version of the `yardlog` format.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Format(u32);

impl Format {
    /// The format this `loft` writes, and the newest it reads.
    pub const CURRENT: Self = Self(1);

    #[must_use]
    pub const fn new(version: u32) -> Self {
        Self(version)
    }

    #[must_use]
    pub const fn version(self) -> u32 {
        self.0
    }

    /// Reads `yardlog/<n>`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let digits = text.strip_prefix("yardlog/")?;
        let canonical = !digits.is_empty()
            && digits.bytes().all(|byte| byte.is_ascii_digit())
            && !digits.starts_with('0');
        if canonical {
            digits.parse().ok().map(Self)
        } else {
            None
        }
    }

    /// Whether this `loft` may write Yard Log of this format.
    #[must_use]
    pub fn is_known(self) -> bool {
        self <= Self::CURRENT
    }
}

impl fmt::Display for Format {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "yardlog/{}", self.0)
    }
}
