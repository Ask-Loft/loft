//! `loft yard read` as a process: Yard Log in order at its revision, or what changed since one;
//! every version with `--versions`; whether git ignores `.yard/`; and the revision each agent
//! last read, kept in `~/.yard/` and never under `.yard/`.

use serde_json::json;

use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, EPOCH_TEXT, Yard, block, document};

const NOTES: &str = r#"[
  {"id": "build", "kind": "stage", "title": "Build it", "status": "planned"},
  {"id": "n1", "kind": "note", "stage": "build", "markdown": "First"},
  {"id": "n2", "kind": "note", "stage": "build", "markdown": "Second"}
]"#;

#[test]
fn the_result_gives_yard_log_s_key_and_its_current_revision() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.write("s1", NOTES, &[]);
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], 2);
    assert!(read["yardLog"]["key"].as_str().is_some());
    assert_eq!(read["yardLog"]["format"], "yardlog/1");
}

#[test]
fn the_result_lists_the_stages_in_order_each_with_its_blocks_in_order() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.write("s1", NOTES, &[]);
    let read = yard.read("s1", &[]);
    let stages: Vec<&str> = read["stages"]
        .as_array()
        .unwrap()
        .iter()
        .map(|stage| stage["id"].as_str().unwrap())
        .collect();
    assert_eq!(stages, ["shape", "build"]);
    let blocks: Vec<&str> = read["stages"][1]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|block| block["id"].as_str().unwrap())
        .collect();
    assert_eq!(blocks, ["n1", "n2"]);
}

#[test]
fn each_block_carries_its_key_its_id_its_kind_and_its_current_version_alone() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let rewrite = r#"[{"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which one?",
        "header": "Shelf", "options": [{"label": "Top"}, {"label": "Bottom"}]}]"#;
    yard.write("s1", rewrite, &[]);
    let read = yard.read("s1", &[]);
    let q1 = block(&read, "q1");
    assert_eq!(q1["kind"], "question");
    assert!(q1["key"].as_str().is_some());
    assert_eq!(q1["stage"], read["stages"][0]["key"]);
    assert_eq!(q1["version"]["number"], 2);
    assert_eq!(q1["version"]["body"]["prompt"], "Which one?");
    assert_eq!(q1["versionCount"], 2);
    assert!(q1.get("versions").is_none());
}

#[test]
fn with_versions_each_block_carries_every_version_oldest_first_with_key_act_and_payload() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let rewrite = r#"[{"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which one?",
        "header": "Shelf", "options": [{"label": "Top"}, {"label": "Bottom"}]}]"#;
    yard.write("s1", rewrite, &[]);
    let read = yard.read("s1", &["--versions"]);
    let versions = block(&read, "q1")["versions"].as_array().unwrap().clone();
    assert_eq!(versions.len(), 2);
    assert_eq!(versions[0]["body"]["prompt"], "Which shelf?");
    assert_eq!(versions[1]["body"]["prompt"], "Which one?");
    for version in &versions {
        assert!(version["key"].as_str().is_some());
        assert_eq!(version["act"]["at"], EPOCH_TEXT);
    }
    assert_ne!(versions[0]["key"], versions[1]["key"]);
}

#[test]
fn each_answered_question_carries_its_answer_who_gave_it_when_and_on_which_channel() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    let answer = block(&yard.read("s1", &[]), "q1")["answer"].clone();
    assert_eq!(answer["labels"], json!(["Top"]));
    assert_eq!(answer["channel"], "terminal");
    assert_eq!(answer["act"]["at"], EPOCH_TEXT);
    assert_eq!(
        answer["act"]["by"],
        json!({ "name": "Ada Reader", "email": "ada@example.com" })
    );
}

#[test]
fn the_result_lists_the_open_submissions_each_with_the_agent_that_sent_it_apart() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let other = r#"[{"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which?",
        "header": "Spine", "options": [{"label": "Red"}, {"label": "Blue"}]}]"#;
    yard.run_with(
        &[
            "write",
            "--session",
            "s2",
            "--send",
            "--agent",
            "a1",
            "--agent-type",
            "plan",
        ],
        other,
    );
    let read = yard.read("s1", &[]);
    let agents: Vec<_> = read["submissions"]
        .as_array()
        .unwrap()
        .iter()
        .map(|group| {
            (
                group["agent"].clone(),
                group["open"].as_array().unwrap().len(),
            )
        })
        .collect();
    assert_eq!(agents.len(), 2);
    assert!(agents.contains(&(json!({ "host": "claude-code", "session": "s1" }), 1)));
    assert!(agents.contains(&(
        json!({ "host": "claude-code", "session": "s2", "agent": "a1", "agentType": "plan" }),
        1
    )));
}

#[test]
fn the_result_says_whether_git_ignores_yard() {
    let mut yard = Yard::new();
    assert_eq!(yard.read("s1", &[])["ignored"], false);
    yard.git = yard.git.clone().ignoring(":/.yard/");
    assert_eq!(yard.read("s1", &[])["ignored"], true);
}

#[test]
fn a_branch_with_no_yard_log_reads_as_yard_log_with_no_stage_at_revision_0() {
    let yard = Yard::new();
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], 0);
    assert_eq!(read["stages"], json!([]));
    assert_eq!(read["yardLog"]["branch"], BRANCH);
    assert!(!yard.scratch.0.join(".yard").exists());
}

#[test]
fn a_read_on_a_detached_head_returns_no_stage_at_revision_0() {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().detached();
    let output = yard.run(&["read", "--session", "s1"]);
    let read = document("read", &output);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(read["revision"], 0);
    assert_eq!(read["detached"], true);
    assert_eq!(read["stages"], json!([]));
}

#[test]
fn a_read_since_a_revision_returns_only_the_blocks_written_or_answered_after_it() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    yard.write("s1", NOTES, &[]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Bottom"]}]"#);
    let read = yard.read("s1", &["--since", "2"]);
    assert_eq!(read["since"], 2);
    let ids: Vec<&str> = read["stages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stage| stage["blocks"].as_array().unwrap())
        .map(|block| block["id"].as_str().unwrap())
        .collect();
    assert_eq!(ids, ["q1"]);
}

#[test]
fn a_read_since_the_current_revision_returns_no_block() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.write("s1", NOTES, &[]);
    assert_eq!(yard.read("s1", &["--since", "2"])["stages"], json!([]));
}

#[test]
fn the_revision_last_read_is_kept_for_each_agent_apart_in_the_machine_s_home() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &["--mark-read"]);
    yard.write("s1", NOTES, &[]);
    yard.run(&[
        "read",
        "--session",
        "s1",
        "--mark-read",
        "--agent",
        "a1",
        "--agent-type",
        "plan",
    ]);
    let key = yard.read("s2", &["--mark-read"])["yardLog"]["key"]
        .as_str()
        .unwrap()
        .to_owned();
    let last = yard.home_file("last-read.json");
    assert_eq!(last[&key], json!({ "s1": 1, "s1/a1": 2, "s2": 2 }));
}

#[test]
fn a_read_by_one_agent_leaves_the_revision_another_last_read_as_it_was() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &["--mark-read"]);
    yard.write("s1", NOTES, &[]);
    yard.read("s2", &["--mark-read"]);
    assert_eq!(yard.read("s1", &[])["lastRead"], 1);
}

#[test]
fn an_agent_with_no_revision_of_its_own_starts_from_the_latest_any_agent_last_read() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &["--mark-read"]);
    yard.write("s1", NOTES, &[]);
    yard.read("s2", &["--mark-read"]);
    assert_eq!(yard.read("s3", &[])["lastRead"], 2);
}

#[test]
fn recording_the_revision_read_changes_no_file_under_yard_and_does_not_raise_the_revision() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let before = snapshot(&yard.scratch.0.join(".yard"));
    let first = yard.read("s1", &[]);
    let second = yard.read("s2", &[]);
    assert!(
        snapshot(&yard.scratch.0.join(".yard")) == before,
        "a file changed"
    );
    assert_eq!(first["revision"], 1);
    assert_eq!(second["revision"], 1);
}

#[test]
fn a_read_without_mark_read_leaves_the_revision_the_agent_last_read_as_it_was() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &["--mark-read"]);
    yard.write("s1", NOTES, &[]);
    let unmarked = yard.read("s1", &[]);
    assert_eq!(unmarked["revision"], 2);
    assert_eq!(unmarked["lastRead"], 1);
    assert_eq!(yard.read("s1", &[])["lastRead"], 1);
}

#[test]
fn a_read_without_mark_read_writes_no_file_of_the_machine_s_home() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let home = yard.scratch.home().join(".yard");
    std::fs::create_dir_all(&home).unwrap();
    let before = snapshot(&home);
    yard.read("s1", &[]);
    yard.read("s2", &["--agent", "a1", "--agent-type", "Explore"]);
    assert!(snapshot(&home) == before, "a file of ~/.yard/ changed");
}

#[test]
fn mark_read_without_session_exits_2_and_names_session() {
    let yard = Yard::new();
    let output = yard.run(&["read", "--mark-read"]);
    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("--session"), "{stderr}");
}
