//! The command event: what each field says of a run, and what it never says.

use std::fs;
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use crate::common::telemetry::{EVENT_SCHEMA, Sent, events, sending, sent, sent_with, the_event};
use crate::common::{EPOCH, Scratch, isolate, stderr, stdout};
use crate::repo::Repo;
use crate::schema;
use loftspec::publish::PublishedAt;
use serde_json::{Value, json};
use software_plant::{Clock, Collector, GitScenario};

/// The event of `loft <args>` in the Reading List, with `variables` set.
fn event_with(args: &[&str], variables: &[(&str, &str)]) -> Value {
    let scratch = Scratch::with_reading_list();
    the_event(&sent_with(&scratch, args, None, variables)).clone()
}

#[test]
fn every_test_process_starts_with_telemetry_and_the_update_check_off() {
    assert_eq!(std::env::var("LOFT_TELEMETRY").as_deref(), Ok("off"));
    assert_eq!(std::env::var("LOFT_UPDATE_CHECK").as_deref(), Ok("off"));
}

#[test]
fn the_event_follows_its_schema_and_names_the_run() {
    let scratch = Scratch::with_reading_list();
    let sent = sent(&scratch, &["check"], None);
    let event = the_event(&sent);
    schema::conforming(&serde_json::to_vec(event).unwrap(), EVENT_SCHEMA);
    assert_eq!(event["command"], json!("check"));
    assert_eq!(event["outcome"], json!("passed"));
    assert_eq!(event["errorKinds"], json!([]));
    assert_eq!(event["version"], json!(env!("CARGO_PKG_VERSION")));
    assert_eq!(event["installedBy"], json!("unknown"));
    assert_eq!(event["mode"], json!("unknown"));
    assert_eq!(event["agent"], json!("unknown"));
}

#[test]
fn the_platform_is_the_one_the_binary_was_built_for() {
    let event = event_with(&["check"], &[]);
    let os = if cfg!(target_os = "macos") {
        "macos"
    } else if cfg!(target_os = "linux") {
        "linux"
    } else {
        "windows"
    };
    assert_eq!(event["os"], json!(os));
    assert_eq!(event["arch"], json!(std::env::consts::ARCH));
}

/// The `time` of the event of `loft check` run with `clock`, and the hours of the system clock
/// just before and just after the run, as an event writes them.
fn time_of_a_run(clock: &Clock) -> (Value, [Value; 2]) {
    let scratch = Scratch::with_reading_list();
    crate::common::telemetry::seen(&scratch);
    let collector = Collector::accepting();
    let mut command = scratch.command(None, clock);
    sending(&mut command, &collector.url());
    let before = system_seconds();
    command.arg("check").output().unwrap();
    let after = system_seconds();
    (
        events(&collector)[0]["time"].clone(),
        [before, after].map(hour),
    )
}

fn system_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// The hour `seconds` since the Unix epoch falls in, written as an event writes its `time`.
fn hour(seconds: u64) -> Value {
    json!(
        PublishedAt::from_unix_seconds(seconds - seconds % 3_600)
            .unwrap()
            .to_string()
    )
}

#[test]
fn the_time_is_the_hour_in_utc() {
    let (time, hours) = time_of_a_run(&Clock::system());
    assert!(hours.contains(&time), "{time} is not one of {hours:?}");
    assert!(time.as_str().unwrap().ends_with(":00:00Z"), "{time}");
}

#[test]
fn the_time_is_the_system_clocks_whatever_the_publish_time() {
    let (time, hours) = time_of_a_run(&Clock::fixed(EPOCH + 3_725));
    assert!(hours.contains(&time), "{time} is not one of {hours:?}");
}

#[test]
fn the_outcome_and_error_kinds_are_the_same_with_or_without_json() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.file("loftspec.yaml"), "format: [unclosed\n").unwrap();
    let text = sent(&scratch, &["check"], None);
    let json = sent(&scratch, &["check", "--format", "json"], None);
    let printed: Value = serde_json::from_str(&stdout(&json.output)).unwrap();
    for sent in [&text, &json] {
        let event = the_event(sent);
        assert_eq!(event["outcome"], printed["outcome"]);
        let kinds: Vec<Value> = printed["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|error| error["kind"].clone())
            .collect();
        assert_eq!(event["errorKinds"], json!(kinds));
        assert_eq!(event["errorKinds"][0], json!("read"));
    }
}

#[test]
fn a_command_that_reports_no_error_has_no_error_kinds() {
    let scratch = Scratch::with_reading_list();
    let sent = sent(&scratch, &["fmt", "--check"], None);
    assert_eq!(the_event(&sent)["errorKinds"], json!([]));
}

#[test]
fn internal_ci_and_an_agent_set_the_mode_in_that_order() {
    let mode = |variables: &[(&str, &str)]| event_with(&["check"], variables)["mode"].clone();
    assert_eq!(mode(&[("LOFT_TELEMETRY", "internal")]), json!("internal"));
    assert_eq!(
        mode(&[("LOFT_TELEMETRY", "internal"), ("CI", "true")]),
        json!("internal")
    );
    assert_eq!(mode(&[("CI", "true"), ("CLAUDECODE", "1")]), json!("ci"));
    assert_eq!(mode(&[("CLAUDECODE", "1")]), json!("agent"));
    assert_eq!(mode(&[]), json!("unknown"));
}

#[test]
fn an_agent_inside_ci_is_still_named() {
    let event = event_with(&["check"], &[("CI", "true"), ("CODEX_CI", "1")]);
    assert_eq!(event["mode"], json!("ci"));
    assert_eq!(event["agent"], json!("chatgpt-codex"));
}

#[test]
fn a_codex_plugin_inside_claude_code_is_claude_code() {
    let event = event_with(
        &["check"],
        &[
            ("CLAUDECODE", "1"),
            ("CLAUDE_CODE_ENTRYPOINT", "cli"),
            ("CODEX_COMPANION_SESSION_ID", "x"),
        ],
    );
    assert_eq!(event["agent"], json!("claude-code"));
}

#[test]
fn the_event_carries_no_text_from_the_tree_the_files_the_repository_or_the_environment() {
    let scratch = Scratch::with_reading_list();
    crate::common::telemetry::seen(&scratch);
    let collector = Collector::accepting();
    let git = GitScenario::clean_on("feature/acme-telltale-branch");
    let mut command = crate::common::telemetry::command(&scratch, Some(&git), &collector);
    command
        .env("USER", "telltale-user")
        .env("LOGNAME", "telltale-user")
        .env("HOSTNAME", "telltale-host")
        .env("GIT_AUTHOR_EMAIL", "telltale@example.com")
        .env("AI_AGENT", "claude-code_telltale-marker")
        .args(["find", "shelf"]);
    let output = command.output().unwrap();
    let body = String::from_utf8(collector.events().remove(0)).unwrap();
    let home = scratch.home();
    let real = |mut command: Command| {
        let output = command.output().unwrap();
        String::from_utf8(output.stdout).unwrap().trim().to_owned()
    };
    let mut telltales = vec![
        real(Command::new("whoami")),
        real(Command::new("hostname")),
        "telltale".to_owned(),
        "acme".to_owned(),
        scratch.0.display().to_string(),
        home.display().to_string(),
        "Shelf".to_owned(),
        "shelf".to_owned(),
        "Reading List".to_owned(),
    ];
    telltales.extend(stdout(&output).lines().map(str::to_owned));
    telltales.extend(stderr(&output).lines().map(str::to_owned));
    for telltale in telltales.iter().filter(|telltale| !telltale.is_empty()) {
        assert!(!body.contains(telltale.as_str()), "{telltale} in {body}");
    }
}

#[test]
fn an_install_record_sets_installed_by() {
    let scratch = Scratch::with_reading_list();
    scratch.record_install("cargo");
    let sent: Sent = sent(&scratch, &["check"], None);
    assert_eq!(the_event(&sent)["installedBy"], json!("cargo"));
}

#[test]
fn an_executable_inside_node_modules_at_ask_loft_is_installed_by_npm() {
    let scratch = Scratch::with_reading_list();
    crate::common::telemetry::seen(&scratch);
    let collector = Collector::accepting();
    let mut command = scratch.npm_installed(&Clock::fixed(EPOCH));
    sending(&mut command, &collector.url());
    command.arg("check").output().unwrap();
    assert_eq!(events(&collector)[0]["installedBy"], json!("npm"));
}

#[test]
fn every_event_has_its_own_random_id() {
    let scratch = Scratch::with_reading_list();
    let first = sent(&scratch, &["check"], None);
    let second = sent(&scratch, &["check"], None);
    assert_ne!(the_event(&first)["eventId"], the_event(&second)["eventId"]);
}

#[test]
fn the_readme_s_sample_event_follows_the_schema() {
    let readme = include_str!("../../../README.md");
    let section = readme
        .split_once("## Telemetry and the update check")
        .unwrap()
        .1;
    let sample = section
        .split_once("```json\n")
        .unwrap()
        .1
        .split_once("```")
        .unwrap()
        .0;
    schema::conforming(sample.as_bytes(), EVENT_SCHEMA);
}

#[test]
fn an_install_record_that_does_not_parse_leaves_the_way_inferred_and_prints_nothing() {
    let scratch = Scratch::with_reading_list();
    fs::create_dir_all(scratch.config()).unwrap();
    fs::write(scratch.config().join("install.json"), "{ not json").unwrap();
    let sent = sent(&scratch, &["check"], None);
    assert_eq!(the_event(&sent)["installedBy"], json!("unknown"));
    assert_eq!(stderr(&sent.output), "");
}

#[cfg(unix)]
#[test]
fn a_run_whose_standard_error_is_a_terminal_is_interactive() {
    let scratch = Scratch::with_reading_list();
    crate::common::telemetry::seen(&scratch);
    let collector = Collector::accepting();
    let mut command = scratch.check_on_terminal(&Clock::fixed(EPOCH));
    sending(&mut command, &collector.url()).output().unwrap();
    assert_eq!(events(&collector)[0]["mode"], json!("interactive"));
}

#[test]
fn error_kinds_keep_the_order_the_command_reports_them_in() {
    let scratch = Scratch::with_reading_list();
    scratch.edit(
        "products/reading-list/product.yaml",
        "  - portable\n",
        "  - portable\n  - portable\n",
    );
    fs::write(scratch.file("lint.yaml"), "rules:\n  - rule: XX-001\n").unwrap();
    let sent = sent(&scratch, &["check", "--format", "json"], None);
    let printed: Value = serde_json::from_str(&stdout(&sent.output)).unwrap();
    let kinds: Vec<Value> = printed["errors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|error| error["kind"].clone())
        .collect();
    assert_eq!(kinds, [json!("validation"), json!("lint-config")]);
    assert_eq!(the_event(&sent)["errorKinds"], json!(kinds));
}

#[test]
fn the_git_remote_and_the_configured_names_never_reach_the_event() {
    let repo = Repo::new();
    repo.git_ok(&["config", "user.email", "telltale@acme.example"]);
    repo.git_ok(&["config", "user.name", "Telltale Person"]);
    repo.git_ok(&[
        "remote",
        "add",
        "origin",
        "https://acme-telltale.example/acme/secret-plans.git",
    ]);
    repo.git_ok(&["checkout", "--quiet", "-b", "feature/acme-telltale"]);
    let scratch = &repo.0;
    crate::common::telemetry::seen(scratch);
    let collector = Collector::accepting();
    let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
    isolate(&mut command, &scratch.home());
    software_plant::real_git::isolate(&mut command);
    sending(&mut command, &collector.url());
    command
        .arg("check")
        .current_dir(&scratch.0)
        .output()
        .unwrap();
    let body = String::from_utf8(collector.events().remove(0)).unwrap();
    let event: Value = serde_json::from_str(&body).unwrap();
    assert!(
        event.get("branch").is_some(),
        "real git named no branch: {body}"
    );
    for telltale in ["telltale", "acme", "Telltale Person", "secret-plans"] {
        assert!(!body.contains(telltale), "{telltale} in {body}");
    }
}

/// Every name, title, statement part, body and other prose the tree under `dir` writes on one
/// line.
fn texts(dir: &std::path::Path, found: &mut Vec<String>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            texts(&path, found);
            continue;
        }
        for line in fs::read_to_string(&path).unwrap().lines() {
            let line = line.trim_start().trim_start_matches("- ");
            let Some((field, value)) = line.split_once(": ") else {
                continue;
            };
            let named = [
                "name",
                "title",
                "body",
                "description",
                "statement",
                "system",
                "when",
                "while",
                "where",
                "if",
                "shall",
                "context",
                "rationale",
                "definition",
                "scenario",
                "environment",
            ]
            .contains(&field);
            let text = value
                .strip_prefix('"')
                .and_then(|quoted| quoted.strip_suffix('"'))
                .unwrap_or(value);
            if named && !text.is_empty() && !["|", ">", "|-", ">-"].contains(&text) {
                found.push(text.to_owned());
            }
        }
    }
}

#[test]
fn no_marker_value_path_part_or_text_of_the_tree_appears_in_the_event() {
    let scratch = Scratch::with_reading_list();
    let markers = [
        "CLAUDE_CODE_IS_COWORK",
        "CLAUDE_CODE_ENTRYPOINT",
        "CODEX_THREAD_ID",
        "CODEX_SANDBOX",
        "CODEX_SANDBOX_NETWORK_DISABLED",
        "CODEX_CI",
        "AI_AGENT",
        "AGENT",
    ];
    let values: Vec<String> = markers
        .iter()
        .map(|name| format!("claude-code-{}-quokka", name.to_lowercase()))
        .collect();
    let variables: Vec<(&str, &str)> = markers
        .iter()
        .zip(&values)
        .map(|(name, value)| (*name, value.as_str()))
        .collect();
    let sent = sent_with(&scratch, &["check"], None, &variables);
    let body = serde_json::to_string(the_event(&sent)).unwrap();
    for value in &values {
        assert!(!body.contains(value.as_str()), "{value} in {body}");
    }
    assert!(!body.contains("quokka"), "{body}");
    for dir in [scratch.0.clone(), scratch.home()] {
        for part in dir.components() {
            let part = part.as_os_str().to_string_lossy();
            if part.len() > 3 {
                assert!(!body.contains(part.as_ref()), "{part} in {body}");
            }
        }
    }
    let mut found = Vec::new();
    texts(&scratch.0.join("spec"), &mut found);
    assert!(found.len() > 80, "{}", found.len());
    for text in found {
        assert!(!body.contains(&text), "{text} in {body}");
    }
}
