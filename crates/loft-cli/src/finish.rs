//! How a command finished: its exit code, the outcome its JSON document names, and the kind of
//! each error it reported, whether or not `--format json` was given.

use std::process::ExitCode;

use serde_json::Value;

/// How a command finished.
#[derive(Clone)]
pub struct Finished {
    pub code: ExitCode,
    pub outcome: &'static str,
    pub error_kinds: Vec<String>,
}

impl Finished {
    /// A run that ended in `outcome` with no error, exiting 0.
    pub fn success(outcome: &'static str) -> Self {
        Self::with(ExitCode::SUCCESS, outcome, &[])
    }

    /// A run that ended in `outcome` with no error, exiting 1.
    pub fn failure(outcome: &'static str) -> Self {
        Self::with(ExitCode::FAILURE, outcome, &[])
    }

    /// A run that ended in `outcome` with `errors`, each a JSON error with a `kind`.
    pub fn with(code: ExitCode, outcome: &'static str, errors: &[Value]) -> Self {
        let error_kinds = errors
            .iter()
            .filter_map(|error| error["kind"].as_str())
            .map(ToOwned::to_owned)
            .collect();
        Self {
            code,
            outcome,
            error_kinds,
        }
    }

    /// A usage error, exiting 2.
    pub fn usage() -> Self {
        Self::with(ExitCode::from(2), "error", &[])
    }

    /// A run that stopped on `errors`: the outcome `error`, exiting 1.
    pub fn stopped(errors: &[Value]) -> Self {
        Self::with(ExitCode::FAILURE, "error", errors)
    }
}
