//! What one command event says: closed values and random tokens only, never text taken from
//! the tree, the code, the file system, the repository or the environment.

use std::io::{self, IsTerminal};
use std::time::Duration;

use loftspec::publish::PublishedAt;
use loftspec_store::clock::{Clock, SystemClock};
use serde_json::{Value, json};

use crate::environment;
use crate::finish::Finished;
use crate::install;
use crate::version::VERSION;

/// The schema an event follows, by name and version.
pub const SCHEMA: &str = "loft-telemetry-event/2";

/// What the event says of the checkout it ran in.
pub struct Checkout {
    pub project: String,
    pub branch: Option<String>,
}

/// The event of a run of `command` that finished as `finished` after `duration`.
pub fn build(
    command: &str,
    finished: &Finished,
    duration: Duration,
    installation: &str,
    checkout: Option<&Checkout>,
    internal: bool,
) -> Option<Value> {
    let now = SystemClock::from_env().now().ok()?;
    let agent = agent(&environment::text);
    let mut event = json!({
        "schema": SCHEMA,
        "eventId": uuid::Uuid::new_v4().to_string(),
        "version": VERSION,
        "os": os(std::env::consts::OS),
        "arch": arch(std::env::consts::ARCH),
        "installedBy": install::way().as_str(),
        "time": PublishedAt::from_unix_seconds(now - now % 3_600).ok()?.to_string(),
        "installation": installation,
        "mode": mode(internal, agent),
        "agent": agent,
        "command": command,
        "outcome": finished.outcome,
        "errorKinds": finished.error_kinds,
        "duration": bucket(duration),
    });
    if let Some(checkout) = checkout {
        event["project"] = json!(checkout.project);
        if let Some(branch) = &checkout.branch {
            event["branch"] = json!(branch);
        }
    }
    Some(event)
}

/// The event's name for the operating system `target_os` names, as Rust names the one a binary
/// is built for in `std::env::consts::OS`.
fn os(target_os: &str) -> &'static str {
    match target_os {
        "linux" => "linux",
        "macos" => "macos",
        "windows" => "windows",
        _ => "other",
    }
}

/// The event's name for the architecture `target_arch` names, as Rust names the one a binary is
/// built for in `std::env::consts::ARCH`.
fn arch(target_arch: &str) -> &'static str {
    match target_arch {
        "x86_64" => "x86_64",
        "aarch64" => "aarch64",
        _ => "other",
    }
}

/// The run's mode, by the first rule that holds: internal use, CI, an agent, a terminal.
fn mode(internal: bool, agent: &str) -> &'static str {
    if internal {
        "internal"
    } else if environment::value("CI").is_some() {
        "ci"
    } else if agent != "unknown" {
        "agent"
    } else if io::stderr().is_terminal() {
        "interactive"
    } else {
        "unknown"
    }
}

/// The agent that started the run, from the variables `variable` reads, by the first rule that
/// holds: the Claude app, Claude Code, Codex, a generic agent variable. Only whether a marker
/// is set decides, and no value is sent; Claude's markers come first because a Codex plugin
/// inside Claude Code sets a variable whose name begins with `CODEX_`.
fn agent(variable: &dyn Fn(&str) -> Option<String>) -> &'static str {
    let entrypoint = variable("CLAUDE_CODE_ENTRYPOINT");
    if variable("CLAUDE_CODE_IS_COWORK").is_some()
        || entrypoint.as_deref() == Some("claude-desktop")
    {
        return "claude";
    }
    if variable("CLAUDECODE").as_deref() == Some("1") || entrypoint.is_some() {
        return "claude-code";
    }
    let codex = [
        "CODEX_THREAD_ID",
        "CODEX_SANDBOX",
        "CODEX_SANDBOX_NETWORK_DISABLED",
        "CODEX_CI",
    ];
    if codex.iter().any(|name| variable(name).is_some()) {
        return "chatgpt-codex";
    }
    let named = variable("AI_AGENT")
        .and_then(|value| named(&value))
        .or_else(|| variable("AGENT").and_then(|value| named(&value)));
    named.unwrap_or("unknown")
}

/// The agent a generic agent variable's value names by its beginning, if it names one.
fn named(value: &str) -> Option<&'static str> {
    [
        ("claude-code", "claude-code"),
        ("claude", "claude"),
        ("codex", "chatgpt-codex"),
        ("chatgpt", "chatgpt"),
    ]
    .into_iter()
    .find(|(prefix, _)| value.starts_with(prefix))
    .map(|(_, agent)| agent)
}

/// The bucket of a run's duration.
fn bucket(duration: Duration) -> &'static str {
    match duration.as_millis() {
        0..100 => "under-100ms",
        100..1_000 => "under-1s",
        1_000..10_000 => "under-10s",
        10_000..60_000 => "under-60s",
        _ => "60s-or-more",
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::Duration;

    use super::{agent, arch, bucket, os};

    fn agent_of(set: &[(&str, &str)]) -> &'static str {
        let set: BTreeMap<String, String> = set
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        agent(&|name| set.get(name).cloned())
    }

    #[test]
    fn each_operating_system_and_architecture_has_its_name_and_any_other_is_other() {
        for (target, name) in [
            ("linux", "linux"),
            ("macos", "macos"),
            ("windows", "windows"),
            ("freebsd", "other"),
            ("ios", "other"),
        ] {
            assert_eq!(os(target), name, "{target}");
        }
        for (target, name) in [
            ("x86_64", "x86_64"),
            ("aarch64", "aarch64"),
            ("riscv64", "other"),
            ("x86", "other"),
        ] {
            assert_eq!(arch(target), name, "{target}");
        }
    }

    #[test]
    fn each_duration_falls_in_its_bucket_at_both_edges() {
        let cases = [
            (0, "under-100ms"),
            (99, "under-100ms"),
            (100, "under-1s"),
            (999, "under-1s"),
            (1_000, "under-10s"),
            (9_999, "under-10s"),
            (10_000, "under-60s"),
            (59_999, "under-60s"),
            (60_000, "60s-or-more"),
            (3_600_000, "60s-or-more"),
        ];
        for (millis, expected) in cases {
            assert_eq!(bucket(Duration::from_millis(millis)), expected, "{millis}");
        }
    }

    #[test]
    fn each_agent_is_recognised_by_its_markers_in_order() {
        assert_eq!(agent_of(&[("CLAUDE_CODE_IS_COWORK", "1")]), "claude");
        assert_eq!(
            agent_of(&[
                ("CLAUDE_CODE_ENTRYPOINT", "claude-desktop"),
                ("CLAUDECODE", "1")
            ]),
            "claude"
        );
        assert_eq!(agent_of(&[("CLAUDECODE", "1")]), "claude-code");
        assert_eq!(
            agent_of(&[("CLAUDE_CODE_ENTRYPOINT", "cli")]),
            "claude-code"
        );
        for marker in [
            "CODEX_THREAD_ID",
            "CODEX_SANDBOX",
            "CODEX_SANDBOX_NETWORK_DISABLED",
            "CODEX_CI",
        ] {
            assert_eq!(agent_of(&[(marker, "x")]), "chatgpt-codex", "{marker}");
        }
        assert_eq!(
            agent_of(&[("CLAUDECODE", "1"), ("CODEX_COMPANION_SESSION_ID", "x")]),
            "claude-code"
        );
    }

    #[test]
    fn a_codex_variable_outside_the_four_markers_recognises_no_agent() {
        assert_eq!(agent_of(&[("CODEX_COMPANION_SESSION_ID", "x")]), "unknown");
    }

    #[test]
    fn the_generic_variables_name_an_agent_by_their_beginning() {
        let cases = [
            ("claude-code_2-1-299_agent", "claude-code"),
            ("claude", "claude"),
            ("codex-cli", "chatgpt-codex"),
            ("chatgpt-agent", "chatgpt"),
            ("cursor", "unknown"),
        ];
        for (value, expected) in cases {
            assert_eq!(agent_of(&[("AI_AGENT", value)]), expected, "{value}");
            assert_eq!(agent_of(&[("AGENT", value)]), expected, "{value}");
        }
        assert_eq!(
            agent_of(&[("AI_AGENT", "codex"), ("AGENT", "claude")]),
            "chatgpt-codex"
        );
        assert_eq!(
            agent_of(&[("AI_AGENT", "cursor"), ("AGENT", "claude")]),
            "claude"
        );
    }

    #[test]
    fn no_marker_leaves_the_agent_unknown() {
        assert_eq!(agent_of(&[]), "unknown");
        assert_eq!(agent_of(&[("CLAUDECODE", "0")]), "unknown");
    }
}
