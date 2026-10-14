//! The addresses of the services `loft` talks to, each replaced by its environment variable
//! when that is set and not empty.

use crate::environment;

/// Where the command events go.
pub const COLLECTOR: &str = "https://askloft.ai/api/telemetry";

/// Where the latest release is read: GitHub's REST API.
pub const RELEASES: &str = "https://api.github.com";

/// The telemetry collector's address: `LOFT_TELEMETRY_ENDPOINT`, or [`COLLECTOR`].
pub fn collector() -> String {
    replaced("LOFT_TELEMETRY_ENDPOINT", COLLECTOR)
}

/// The address of the latest release of `Ask-Loft/loft`, under `LOFT_UPDATE_ENDPOINT` or
/// [`RELEASES`].
pub fn latest_release() -> String {
    format!(
        "{}/repos/Ask-Loft/loft/releases/latest",
        replaced("LOFT_UPDATE_ENDPOINT", RELEASES).trim_end_matches('/')
    )
}

fn replaced(variable: &str, default: &str) -> String {
    environment::text(variable).unwrap_or_else(|| default.to_owned())
}
