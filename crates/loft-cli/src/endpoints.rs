//! The addresses of the services `loft` talks to, each replaced by its environment variable
//! when that is set and not empty.

use std::env;

/// Where the command events go.
pub const COLLECTOR: &str = "https://askloft.ai/api/telemetry";

/// The telemetry collector's address: `LOFT_TELEMETRY_ENDPOINT`, or [`COLLECTOR`].
pub fn collector() -> String {
    replaced("LOFT_TELEMETRY_ENDPOINT", COLLECTOR)
}

fn replaced(variable: &str, default: &str) -> String {
    env::var(variable)
        .ok()
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| default.to_owned())
}
