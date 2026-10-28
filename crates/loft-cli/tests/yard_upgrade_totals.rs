//! `loft yard upgrade` over the totals files of open Yard Logs: a Codex file written before the
//! adapter classified cache writes has its zero cache-write counts made unavailable, every file
//! gains the marker, a file that has it, and every file of a closed Yard Log, is left as it was,
//! and nothing is touched while a page server runs.

use std::fs;

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
const CODEX_CLASSIFIED: &str = "session: s1
host: codex
cacheWriteClassified: true
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

/// A Claude Code session's totals with a measured zero cache write, before the marker.
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
    let output = yard.run(&["upgrade"]);
    let doc = document("upgrade", &output);
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
fn a_claude_code_totals_file_keeps_its_counts_and_gains_the_marker() {
    let yard = with_totals(&[("s2", CLAUDE)]);
    let (code, result) = upgrade(&yard);
    assert_eq!(code, Some(0));
    let marked = CLAUDE.replacen(
        "host: claude-code\n",
        "host: claude-code\ncacheWriteClassified: true\n",
        1,
    );
    assert_eq!(text(&yard, "s2"), marked);
    assert_eq!(
        result["classified"],
        json!([{ "branch": BRANCH, "host": "claude-code", "session": "s2" }])
    );
}

#[test]
fn a_totals_file_that_carries_the_marker_is_left_and_a_second_run_changes_nothing() {
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
