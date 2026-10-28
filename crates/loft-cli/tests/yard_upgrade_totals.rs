//! `loft yard upgrade` over the totals files of open Yard Logs: a Codex file of an older totals
//! format has its zero cache-write counts made unavailable, every such file gains the current
//! format, a file already in it, and every file of a closed Yard Log, is left as it was, nothing
//! is touched while a page server runs, and a file it fails to settle is named beside what became
//! of each Yard Log.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::process::Output;

use serde_json::{Value, json};

use crate::common::workspace;
use crate::schema;
use crate::served::Served;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, document};

/// A Codex session's totals as a page server wrote them before cache writes were classified:
/// a zero cache write in every scope but the subagent's, whose positive count reaches the
/// session total and the model counts.
pub(crate) const CODEX: &str = "session: s1
host: codex
firstEnded: 2026-01-01T10:00:01Z
lastEnded: 2026-01-01T10:00:02Z
tokens:
  input: 140
  cacheRead: 60
  cacheWrite: 4
  output: 10
stages:
  - stage: shape
    input: 70
    cacheRead: 30
    cacheWrite: 0
    output: 5
    agents:
      - agent: main
        input: 70
        cacheRead: 30
        cacheWrite: 0
        output: 5
noStage:
  input: 70
  cacheRead: 30
  cacheWrite: 4
  output: 5
agents:
  - agent: main
    model: fixture
    input: 70
    cacheRead: 30
    cacheWrite: 0
    output: 5
  - agent: r1
    type: review
    model: fixture
    input: 70
    cacheRead: 30
    cacheWrite: 4
    output: 5
submissions:
  - submission: sub
    input: 70
    cacheRead: 30
    cacheWrite: 0
    output: 5
    partial:
      - cacheWrite
";

/// [`CODEX`] once upgraded: each zero cache write unavailable, each positive one kept.
const CODEX_CLASSIFIED: &str = "format: yardtokens/2
session: s1
host: codex
firstEnded: 2026-01-01T10:00:01Z
lastEnded: 2026-01-01T10:00:02Z
tokens:
  input: 140
  cacheRead: 60
  cacheWrite: 4
  output: 10
stages:
  - stage: shape
    input: 70
    cacheRead: 30
    output: 5
    agents:
      - agent: main
        input: 70
        cacheRead: 30
        output: 5
noStage:
  input: 70
  cacheRead: 30
  cacheWrite: 4
  output: 5
agents:
  - agent: main
    model: fixture
    input: 70
    cacheRead: 30
    output: 5
  - agent: r1
    type: review
    model: fixture
    input: 70
    cacheRead: 30
    cacheWrite: 4
    output: 5
submissions:
  - submission: sub
    input: 70
    cacheRead: 30
    output: 5
";

/// A Claude Code session's totals with a measured zero cache write, in the first totals format.
const CLAUDE: &str = "session: s2
host: claude-code
firstEnded: 2026-01-01T10:00:01Z
lastEnded: 2026-01-01T10:00:01Z
tokens:
  input: 5
  cacheRead: 0
  cacheWrite: 0
  output: 1
stages:
  - stage: shape
    input: 5
    cacheRead: 0
    cacheWrite: 0
    output: 1
    agents:
      - agent: main
        input: 5
        cacheRead: 0
        cacheWrite: 0
        output: 1
agents:
  - agent: main
    model: opus
    input: 5
    cacheRead: 0
    cacheWrite: 0
    output: 1
";

pub(crate) fn tokens(yard: &Yard, key: &str) -> std::path::PathBuf {
    yard.folder(BRANCH)
        .join("tokens")
        .join(format!("{key}.yaml"))
}

/// An open Yard Log on [`BRANCH`] holding the totals `files`, by session key.
pub(crate) fn with_totals(files: &[(&str, &str)]) -> Yard {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    fs::create_dir_all(yard.folder(BRANCH).join("tokens")).unwrap();
    for (key, text) in files {
        fs::write(tokens(&yard, key), text).unwrap();
    }
    yard
}

pub(crate) fn upgrade(yard: &Yard) -> (Option<i32>, Value) {
    conforming(&yard.run(&["upgrade"]))
}

/// Runs `loft yard upgrade` where no home directory is known.
fn upgrade_without_home(yard: &Yard) -> (Option<i32>, Value) {
    let output = yard
        .scratch
        .command(Some(&yard.git), &yard.clock)
        .args(["yard", "upgrade", "--format", "json"])
        .env("HOME", "no-home")
        .output()
        .unwrap();
    conforming(&output)
}

fn conforming(output: &Output) -> (Option<i32>, Value) {
    let doc = document("upgrade", output);
    let schema = fs::read_to_string(workspace().join("schemas/yard-upgrade.json")).unwrap();
    let text = serde_json::to_vec(&doc).unwrap();
    (output.status.code(), schema::conforming(&text, &schema))
}

fn text(yard: &Yard, key: &str) -> String {
    fs::read_to_string(tokens(yard, key)).unwrap()
}

#[test]
fn a_codex_zero_cache_write_becomes_unavailable_in_every_scope_and_positive_counts_are_kept() {
    let yard = with_totals(&[("codex~s1", CODEX)]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(text(&yard, "codex~s1"), CODEX_CLASSIFIED);
    assert_eq!(
        result["classified"],
        json!([{ "branch": BRANCH, "host": "codex", "session": "s1" }])
    );
}

#[test]
fn a_claude_code_totals_file_keeps_its_counts_and_gains_its_format() {
    let yard = with_totals(&[("s2", CLAUDE)]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(text(&yard, "s2"), format!("format: yardtokens/2\n{CLAUDE}"));
    assert_eq!(
        result["classified"],
        json!([{ "branch": BRANCH, "host": "claude-code", "session": "s2" }])
    );
}

#[test]
fn a_totals_file_already_in_the_current_format_is_left_and_a_second_run_changes_nothing() {
    let yard = with_totals(&[("codex~s1", CODEX), ("s2", CLAUDE)]);
    upgrade(&yard);
    let before = snapshot(&yard.scratch.0.join(".yard"));
    let (code, second) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(second["classified"], json!([]));
    assert!(
        snapshot(&yard.scratch.0.join(".yard")) == before,
        "a file changed"
    );
}

#[test]
fn a_totals_file_that_carries_only_an_earlier_build_s_marker_gains_its_format() {
    let marked = CLAUDE.replacen(
        "host: claude-code\n",
        "host: claude-code\ncacheWriteClassified: true\n",
        1,
    );
    let yard = with_totals(&[("s2", &marked)]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(text(&yard, "s2"), format!("format: yardtokens/2\n{CLAUDE}"));
    assert_eq!(result["classified"][0]["session"], "s2");
}

#[test]
fn the_totals_files_of_a_closed_yard_log_are_left_as_they_were_written() {
    let yard = with_totals(&[("codex~s1", CODEX)]);
    yard.run(&["close", "--session", "s1"]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(text(&yard, "codex~s1"), CODEX);
    assert_eq!(result["classified"], json!([]));
}

#[test]
fn while_a_page_server_runs_totals_files_are_left_and_the_page_server_is_named() {
    let yard = with_totals(&[("codex~s1", CODEX)]);
    let served = Served::start(&yard, &[]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(text(&yard, "codex~s1"), CODEX);
    assert_eq!(result["classified"], json!([]));
    assert_eq!(result["totalsLeft"]["port"], served.port);
    let reason = result["totalsLeft"]["reason"].as_str().unwrap();
    assert!(reason.contains(&served.port.to_string()), "{reason}");
    drop(served);
    upgrade(&yard);
    assert_eq!(text(&yard, "codex~s1"), CODEX_CLASSIFIED);
}

#[test]
fn a_totals_file_it_fails_to_write_is_named_beside_what_became_of_each_yard_log() {
    let yard = with_totals(&[("s2", CLAUDE)]);
    let folder = yard.folder(BRANCH).join("tokens");
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o555)).unwrap();
    let (code, result) = upgrade(&yard);
    fs::set_permissions(&folder, fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!(code, Some(1));
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["current"][0]["branch"], BRANCH);
    assert_eq!(result["upgraded"], json!([]));
    assert_eq!(result["left"], json!([]));
    let path = result["totalsFailed"]["path"].as_str().unwrap();
    assert!(path.ends_with("tokens/s2.yaml"), "{path}");
    assert!(
        result["totalsFailed"]["message"]
            .as_str()
            .is_some_and(|message| !message.is_empty())
    );
    assert_eq!(text(&yard, "s2"), CLAUDE);
}

#[test]
fn with_no_home_an_older_totals_file_is_named_as_the_one_it_failed_on() {
    let yard = with_totals(&[("s2", CLAUDE)]);
    let (code, result) = upgrade_without_home(&yard);
    assert_eq!(code, Some(1));
    assert_eq!(result["outcome"], "failed");
    assert_eq!(result["current"][0]["branch"], BRANCH);
    let path = result["totalsFailed"]["path"].as_str().unwrap();
    assert!(path.ends_with("tokens/s2.yaml"), "{path}");
}

#[test]
fn with_no_home_and_no_totals_file_to_settle_the_upgrade_does_not_fail() {
    let current = format!("format: yardtokens/2\n{CLAUDE}");
    let yard = with_totals(&[("s2", &current)]);
    let (code, result) = upgrade_without_home(&yard);
    assert_eq!(code, Some(0));
    assert_eq!(result["outcome"], "upgraded");
    assert!(result.get("totalsFailed").is_none(), "{result}");
    assert_eq!(text(&yard, "s2"), current);
}
