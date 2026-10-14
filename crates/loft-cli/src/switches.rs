//! The switches of telemetry and of the update check: an environment variable for one shell, CI
//! job or container, and a choice recorded by `loft <command> on|off` for the user on the
//! machine. Each is read before anything else either does.

use crate::{environment, settings};

/// One switched concern: its variable, the settings file that records its choice, and the
/// command that records it.
pub struct Switch {
    pub variable: &'static str,
    pub file: &'static str,
    pub command: &'static str,
    /// Whether the variable's value `internal` keeps it on, as internal use.
    pub internal: bool,
}

/// Telemetry: `LOFT_TELEMETRY` and `loft telemetry`.
pub const TELEMETRY: Switch = Switch {
    variable: "LOFT_TELEMETRY",
    file: "telemetry.json",
    command: "loft telemetry",
    internal: true,
};

/// The update check: `LOFT_UPDATE_CHECK` and `loft update-check`.
pub const UPDATE_CHECK: Switch = Switch {
    variable: "LOFT_UPDATE_CHECK",
    file: "update-check.json",
    command: "loft update-check",
    internal: false,
};

/// Whether a concern is on, what decided it, and whether it runs as internal use.
#[derive(Debug, PartialEq, Eq)]
pub struct State {
    pub on: bool,
    pub decided_by: String,
    pub internal: bool,
}

impl Switch {
    /// The state the switches give, the variable first. A value of the variable switches the
    /// concern off, so a mistyped one sends nothing, except that `internal` keeps telemetry on
    /// unless its recorded choice is off.
    pub fn state(&self) -> State {
        let variable = environment::value(self.variable);
        let internal = self.internal && variable.as_deref() == Some("internal".as_ref());
        if variable.is_some() && !internal {
            return self.by_variable(false);
        }
        match (self.recorded(), internal) {
            (Some(on), false) => self.by_choice(on),
            (Some(false), true) => self.by_choice(false),
            (_, true) => self.by_variable(true),
            (None, false) => State {
                on: true,
                decided_by: "default".to_owned(),
                internal: false,
            },
        }
    }

    /// The state the variable decides: on only as internal use.
    fn by_variable(&self, on: bool) -> State {
        State {
            on,
            decided_by: self.variable.to_owned(),
            internal: on,
        }
    }

    /// The choice `loft <command> on|off` recorded, if any.
    fn recorded(&self) -> Option<bool> {
        match settings::read(self.file)["choice"].as_str() {
            Some("on") => Some(true),
            Some("off") => Some(false),
            _ => None,
        }
    }

    fn by_choice(&self, on: bool) -> State {
        State {
            on,
            decided_by: format!("{} {}", self.command, if on { "on" } else { "off" }),
            internal: false,
        }
    }
}
