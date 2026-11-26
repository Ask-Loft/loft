//! The Claude Code plugin's command hooks: `hooks/hooks.json` runs `loft yard claude hook` for
//! each event it serves through the plugin's `hooks/run-loft.sh`, which runs the `loftCommand`
//! setting as written, or, while it is `loft`, the `loft` on the search path, else the one in
//! `~/.cargo/bin`. Where no `loft` can be run, the `SessionStart` hook tells the agent that Yard
//! Client is off and how to turn it on, and every other hook prints nothing and exits 0.

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::path::{Path, PathBuf};
#[cfg(unix)]
use std::process::{Command, Output, Stdio};

use crate::common::workspace;
#[cfg(unix)]
use crate::common::{Scratch, isolate, stderr};
use crate::plugins::{CLAUDE_ROOT, read};
#[cfg(unix)]
use serde_json::Value;
use serde_json::json;

/// The command each of the Claude Code plugin's command hooks runs.
const CLAUDE_HOOK: &str = "LOFT_TELEMETRY=off LOFT_UPDATE_CHECK=off exec sh \"${CLAUDE_PLUGIN_ROOT}/hooks/run-loft.sh\" yard claude hook --host-pid \"$PPID\"";

/// The events the plugin's command hooks serve.
const EVENTS: [&str; 8] = [
    "PermissionRequest",
    "PostCompact",
    "PostToolUse",
    "SessionEnd",
    "SessionStart",
    "Stop",
    "SubagentStart",
    "UserPromptSubmit",
];

#[test]
fn the_claude_code_plugins_hooks_record_holds_only_the_command_hooks_that_run_loft_yard_claude_hook()
 {
    let manifest = read(&format!("{CLAUDE_ROOT}/hooks/hooks.json"));
    let hooks = manifest["hooks"].as_object().unwrap();
    let mut names: Vec<_> = hooks.keys().map(String::as_str).collect();
    names.sort_unstable();
    assert_eq!(names, EVENTS);
    for (name, groups) in hooks {
        let groups = groups.as_array().unwrap();
        assert_eq!(groups.len(), 1, "{name}");
        let asking = name == "PermissionRequest";
        assert_eq!(
            groups[0].get("matcher"),
            asking.then(|| json!("ExitPlanMode")).as_ref(),
            "{name}"
        );
        assert_eq!(
            groups[0]["hooks"],
            json!([{"type": "command", "command": CLAUDE_HOOK, "timeout": if asking { 86_400 } else { 10 }}]),
            "{name}"
        );
    }
    assert!(
        workspace()
            .join(CLAUDE_ROOT)
            .join("hooks/run-loft.sh")
            .is_file()
    );
}

/// Where a hook runs: its `loftCommand` setting, if any, and the directories of its search path.
#[cfg(unix)]
struct Host<'a> {
    setting: Option<&'a str>,
    path: Vec<PathBuf>,
    cargo_home: Option<PathBuf>,
}

#[cfg(unix)]
impl Host<'_> {
    /// A host whose search path holds the system's own programs alone, and no `loft`.
    fn bare() -> Self {
        Self {
            setting: None,
            path: vec![PathBuf::from("/usr/bin"), PathBuf::from("/bin")],
            cargo_home: None,
        }
    }
}

/// Runs the Claude Code plugin's hook command through the shell as Claude Code does, on `host`,
/// with the hook object `hook` on its standard input.
#[cfg(unix)]
fn run_claude_hook(scratch: &Scratch, host: &Host, hook: &Value) -> Output {
    use std::io::Write as _;
    let mut command = Command::new("sh");
    isolate(&mut command, &scratch.home());
    command
        .args(["-c", CLAUDE_HOOK])
        .env("CLAUDE_PLUGIN_ROOT", workspace().join(CLAUDE_ROOT))
        .env("PATH", std::env::join_paths(&host.path).unwrap())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    match host.setting {
        Some(setting) => command.env("CLAUDE_PLUGIN_OPTION_LOFTCOMMAND", setting),
        None => command.env_remove("CLAUDE_PLUGIN_OPTION_LOFTCOMMAND"),
    };
    match &host.cargo_home {
        Some(cargo_home) => command.env("CARGO_HOME", cargo_home),
        None => command.env_remove("CARGO_HOME"),
    };
    let mut child = command.spawn().unwrap();
    child
        .stdin
        .take()
        .unwrap()
        .write_all(hook.to_string().as_bytes())
        .unwrap();
    child.wait_with_output().unwrap()
}

/// Puts the `loft` under test at `dir/name`.
#[cfg(unix)]
fn place_loft(dir: &Path, name: &str) {
    fs::create_dir_all(dir).unwrap();
    std::os::unix::fs::symlink(env!("CARGO_BIN_EXE_loft"), dir.join(name)).unwrap();
}

/// Whether the hooks ran a `loft` on `host`: a `SessionEnd` after `/clear` and the `SessionStart`
/// that follows it record that the new session continues the old one, which only `loft` does.
#[cfg(unix)]
fn hooks_ran_loft(scratch: &Scratch, host: &Host) -> bool {
    let dir = scratch.0.canonicalize().unwrap();
    for hook in [
        json!({"session_id": "s1", "cwd": dir, "hook_event_name": "SessionEnd", "reason": "clear"}),
        json!({"session_id": "s2", "cwd": dir, "hook_event_name": "SessionStart", "source": "clear"}),
    ] {
        let ran = run_claude_hook(scratch, host, &hook);
        assert!(ran.status.success(), "{}", stderr(&ran));
    }
    let Ok(continued) = fs::read_to_string(scratch.home().join(".yard/continuations.json")) else {
        return false;
    };
    assert_eq!(
        serde_json::from_str::<Value>(&continued).unwrap(),
        json!({"s2": "s1"}),
        "both hooks ran under one parent, the process the shell names in $PPID"
    );
    true
}

/// What the `SessionStart` hook adds to the agent's context on `host`, where it adds anything.
#[cfg(unix)]
fn session_start_context(scratch: &Scratch, host: &Host) -> Option<String> {
    let dir = scratch.0.canonicalize().unwrap();
    let hook = json!({"session_id": "s1", "cwd": dir, "hook_event_name": "SessionStart", "source": "startup"});
    let ran = run_claude_hook(scratch, host, &hook);
    assert!(ran.status.success(), "{}", stderr(&ran));
    assert_eq!(stderr(&ran), "");
    let printed: Value = serde_json::from_slice(&ran.stdout).ok()?;
    assert_eq!(
        printed["hookSpecificOutput"]["hookEventName"],
        "SessionStart"
    );
    printed["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .map(ToOwned::to_owned)
}

#[cfg(unix)]
#[test]
fn the_claude_hook_command_runs_a_loft_command_of_several_words_and_passes_its_claude_code_process()
{
    let scratch = Scratch::empty();
    let setting = format!("/usr/bin/env {}", env!("CARGO_BIN_EXE_loft"));
    let host = Host {
        setting: Some(&setting),
        ..Host::bare()
    };
    assert!(hooks_ran_loft(&scratch, &host));
}

#[cfg(unix)]
#[test]
fn the_claude_hooks_run_the_loft_on_the_search_path() {
    let scratch = Scratch::empty();
    let bin = scratch.0.join(".bin");
    place_loft(&bin, "loft");
    let mut host = Host::bare();
    host.path.insert(0, bin);
    assert!(hooks_ran_loft(&scratch, &host));
}

#[cfg(unix)]
#[test]
fn the_claude_hooks_run_the_loft_in_cargo_s_bin_where_the_search_path_has_none() {
    let scratch = Scratch::empty();
    place_loft(&scratch.home().join(".cargo/bin"), "loft");
    assert!(hooks_ran_loft(&scratch, &Host::bare()));
}

#[cfg(unix)]
#[test]
fn the_claude_hooks_look_in_the_bin_of_the_cargo_home_that_cargo_home_names_and_not_in_cargo() {
    let scratch = Scratch::empty();
    let cargo_home = scratch.0.join(".cargo-elsewhere");
    place_loft(&cargo_home.join("bin"), "loft");
    let host = Host {
        cargo_home: Some(cargo_home.clone()),
        ..Host::bare()
    };
    assert!(hooks_ran_loft(&scratch, &host));
    let other = Scratch::empty();
    place_loft(&other.home().join(".cargo/bin"), "loft");
    let host = Host {
        cargo_home: Some(cargo_home.join("missing")),
        ..Host::bare()
    };
    assert!(!hooks_ran_loft(&other, &host));
    let context = session_start_context(&other, &host).expect("a context");
    assert!(context.contains("missing/bin/loft`"), "{context}");
}

#[cfg(unix)]
#[test]
fn the_claude_hooks_run_a_loft_exe_in_cargo_s_bin_as_windows_names_it() {
    let scratch = Scratch::empty();
    place_loft(&scratch.home().join(".cargo/bin"), "loft.exe");
    assert!(hooks_ran_loft(&scratch, &Host::bare()));
}

#[cfg(unix)]
#[test]
fn a_loft_command_of_blanks_or_of_loft_between_blanks_is_the_bare_loft_and_looks_in_cargo_s_bin() {
    for setting in ["", "  ", " loft\t"] {
        let scratch = Scratch::empty();
        place_loft(&scratch.home().join(".cargo/bin"), "loft");
        let host = Host {
            setting: Some(setting),
            ..Host::bare()
        };
        assert!(hooks_ran_loft(&scratch, &host), "{setting:?}");
    }
}

#[cfg(unix)]
#[test]
fn the_context_stays_one_json_document_where_the_home_directory_holds_a_tab_or_a_quote() {
    let scratch = Scratch::empty();
    let home = scratch.home().join("odd\t\"home");
    fs::create_dir_all(&home).unwrap();
    let dir = scratch.0.canonicalize().unwrap();
    let hook = json!({"session_id": "s1", "cwd": dir, "hook_event_name": "SessionStart", "source": "startup"});
    let mut command = Command::new("sh");
    isolate(&mut command, &home);
    command
        .args(["-c", CLAUDE_HOOK])
        .env("CLAUDE_PLUGIN_ROOT", workspace().join(CLAUDE_ROOT))
        .env("PATH", "/usr/bin:/bin")
        .env_remove("CLAUDE_PLUGIN_OPTION_LOFTCOMMAND")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped());
    let mut child = command.spawn().unwrap();
    std::io::Write::write_all(
        &mut child.stdin.take().unwrap(),
        hook.to_string().as_bytes(),
    )
    .unwrap();
    let ran = child.wait_with_output().unwrap();
    let printed: Value = serde_json::from_slice(&ran.stdout).expect("one JSON document");
    let context = printed["hookSpecificOutput"]["additionalContext"]
        .as_str()
        .unwrap();
    assert!(context.contains("odd \"home/.cargo/bin/loft"), "{context}");
}

#[cfg(unix)]
#[test]
fn a_loft_command_the_person_set_is_run_as_written_and_nothing_else_is_tried() {
    let scratch = Scratch::empty();
    place_loft(&scratch.home().join(".cargo/bin"), "loft");
    let host = Host {
        setting: Some("/nowhere/loft --flag"),
        ..Host::bare()
    };
    assert!(!hooks_ran_loft(&scratch, &host));
    let context = session_start_context(&scratch, &host).expect("a context");
    assert!(context.contains("`/nowhere/loft --flag`"), "{context}");
    assert!(!context.contains(".cargo"), "{context}");
}

#[cfg(unix)]
#[test]
fn with_no_loft_to_run_the_session_start_hook_tells_the_agent_that_yard_client_is_off_and_how_to_turn_it_on()
 {
    let scratch = Scratch::empty();
    let context = session_start_context(&scratch, &Host::bare()).expect("a context");
    let home = scratch.home();
    for needle in [
        "Yard Client is off".to_owned(),
        "`loft` on the search path".to_owned(),
        format!("`{}/.cargo/bin/loft`", home.display()),
        format!("`{}/.cargo/bin/loft.exe`", home.display()),
        "`loftCommand` setting".to_owned(),
        "put `loft` on the search path".to_owned(),
        "start a new session".to_owned(),
    ] {
        assert!(context.contains(&needle), "{needle} in {context}");
    }
}

#[cfg(unix)]
#[test]
fn with_no_loft_to_run_every_other_hook_prints_nothing_and_exits_0() {
    let scratch = Scratch::empty();
    let dir = scratch.0.canonicalize().unwrap();
    for event in EVENTS.iter().filter(|event| **event != "SessionStart") {
        let hook = json!({"session_id": "s1", "cwd": dir, "hook_event_name": event, "tool_name": "ExitPlanMode", "reason": "other"});
        let ran = run_claude_hook(&scratch, &Host::bare(), &hook);
        assert!(ran.status.success(), "{event}: {}", stderr(&ran));
        assert_eq!(ran.stdout, b"", "{event}");
        assert_eq!(stderr(&ran), "", "{event}");
    }
}
