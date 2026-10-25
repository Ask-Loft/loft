//! `loft yard write` as a process: opening the branch's Yard Log, writing a batch whole or not
//! at all, the Act of each version, sending a submission, and the refusals of a detached HEAD,
//! a batch based on a stale revision, and a batch that sets what only the person gives.

use serde_json::json;

use crate::common::stderr;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, EPOCH_TEXT, Yard, block, document};

#[test]
fn the_first_write_opens_the_branch_s_yard_log_with_a_root_whose_folder_follows_the_branch() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &[]);
    assert_eq!(written["outcome"], "written");
    let root = yard.scratch.0.join(".yard/feat/shelf/yardlog.yaml");
    assert!(root.is_file());
    let read = yard.read("s1", &[]);
    assert_eq!(read["yardLog"]["branch"], BRANCH);
    assert_eq!(read["yardLog"]["title"], BRANCH);
    assert_eq!(read["yardLog"]["opened"]["at"], EPOCH_TEXT);
    assert_eq!(read["yardLog"]["opened"]["by"]["session"], "s1");
}

#[test]
fn yard_log_keeps_the_key_it_was_opened_with_and_later_writes_apply_to_it() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let key = yard.read("s1", &[])["yardLog"]["key"].clone();
    assert!(key.as_str().is_some_and(|key| key.len() == 36));
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "More"}]"#;
    assert_eq!(yard.write("s2", note, &[])["revision"], 2);
    let read = yard.read("s1", &[]);
    assert_eq!(read["yardLog"]["key"], key);
    assert_eq!(read["stages"][0]["blocks"].as_array().unwrap().len(), 2);
}

#[test]
fn a_write_on_another_branch_of_the_repository_opens_that_branch_s_own_yard_log() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let first = yard.read("s1", &[])["yardLog"]["key"].clone();
    yard.git = Yard::on("fix/spine").git;
    yard.write("s1", ASK, &[]);
    let other = yard.read("s1", &[]);
    assert_eq!(other["branch"], "fix/spine");
    assert_ne!(other["yardLog"]["key"], first);
    assert!(yard.folder("fix/spine").join("yardlog.yaml").is_file());
    assert!(yard.folder(BRANCH).join("yardlog.yaml").is_file());
}

#[test]
fn an_applied_batch_raises_the_revision_by_one_and_gives_the_key_of_each_block_it_wrote() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &[]);
    assert_eq!(written["revision"], 1);
    let blocks = written["blocks"].as_array().unwrap();
    assert_eq!(blocks.len(), 2);
    assert_eq!(blocks[1]["id"], "q1");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "q1")["key"], blocks[1]["key"]);
    assert_eq!(yard.write("s1", ASK, &[])["revision"], 2);
}

#[test]
fn a_batch_with_an_invalid_block_changes_no_file_and_names_the_block_and_the_field() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let before = snapshot(&yard.scratch.0);
    let bad = r#"[{"id": "n2", "kind": "note", "stage": "shape", "markdown": "ok"},
                  {"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which?",
                   "header": "Pick", "options": [{"label": "Only"}]}]"#;
    let output = yard.run_with(&["write", "--session", "s1"], bad);
    let refused = document("write", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(refused["revision"], 1);
    let message = refused["errors"][0]["message"].as_str().unwrap();
    assert!(
        message.contains("`q2`") && message.contains("`options`"),
        "{message}"
    );
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn each_version_carries_the_revision_the_session_and_the_time_of_its_write() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.run_with(
        &[
            "write",
            "--session",
            "s2",
            "--agent",
            "a7",
            "--agent-type",
            "explorer",
        ],
        ASK,
    );
    let q1 = block(&yard.read("s1", &["--versions"]), "q1").clone();
    let versions = q1["versions"].as_array().unwrap();
    assert_eq!(versions[0]["act"]["revision"], 1);
    assert_eq!(
        versions[0]["act"]["by"],
        json!({ "host": "claude-code", "session": "s1" })
    );
    assert_eq!(versions[1]["act"]["revision"], 2);
    assert_eq!(
        versions[1]["act"]["by"],
        json!({ "host": "claude-code", "session": "s2", "agent": "a7", "agentType": "explorer" })
    );
    assert_eq!(versions[1]["act"]["at"], EPOCH_TEXT);
}

#[test]
fn a_batch_sent_opens_a_submission_of_its_asked_blocks_with_the_key_loft_mints() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &["--send"]);
    let submission = written["submission"].as_str().unwrap();
    let read = yard.read("s1", &[]);
    let open = &read["submissions"][0]["open"][0];
    assert_eq!(open["key"], submission);
    assert_eq!(open["blocks"], json!([block(&read, "q1")["key"]]));
    assert_eq!(open["sent"]["at"], EPOCH_TEXT);
    assert_eq!(open["sent"]["by"]["session"], "s1");
    assert_eq!(read["submissions"][0]["agent"]["session"], "s1");
}

#[test]
fn a_batch_not_sent_opens_no_submission() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &[]);
    assert!(written.get("submission").is_none());
    assert_eq!(yard.read("s1", &[])["submissions"], json!([]));
}

#[test]
fn a_batch_sent_with_nothing_to_send_is_refused_and_changes_no_file() {
    let yard = Yard::new();
    let note = r#"[{"id": "s", "kind": "stage", "title": "Only a stage"}]"#;
    let output = yard.run_with(&["write", "--session", "s1", "--send"], note);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(
        document("write", &output)["errors"][0]["kind"],
        "nothing-to-send"
    );
    assert!(!yard.scratch.0.join(".yard").exists());
}

#[test]
fn a_write_on_a_detached_head_is_refused_and_writes_nothing_under_yard() {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().detached();
    let output = yard.run_with(&["write", "--session", "s1"], ASK);
    assert_eq!(output.status.code(), Some(1));
    assert!(
        stderr(&output).contains("belongs to a branch"),
        "{}",
        stderr(&output)
    );
    assert_eq!(document("write", &output)["errors"][0]["kind"], "detached");
    assert!(!yard.scratch.0.join(".yard").exists());
}

#[test]
fn a_batch_that_changes_a_block_changed_since_its_base_is_refused_with_its_current_version() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "One"}]"#;
    yard.write("s2", note, &[]);
    let before = snapshot(&yard.scratch.0);
    let rewrite = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Two"}]"#;
    let output = yard.run_with(&["write", "--session", "s1", "--based-on", "1"], rewrite);
    let refused = document("write", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["errors"][0]["kind"], "stale");
    assert_eq!(refused["revision"], 2);
    assert_eq!(refused["stale"][0]["id"], "n1");
    assert_eq!(refused["stale"][0]["version"]["body"]["markdown"], "One");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

#[test]
fn a_batch_whose_blocks_did_not_change_since_its_base_is_applied_though_others_did() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "One"}]"#;
    yard.write("s2", note, &[]);
    let other = r#"[{"id": "n2", "kind": "note", "stage": "shape", "markdown": "Mine"}]"#;
    let output = yard.run_with(&["write", "--session", "s1", "--based-on", "1"], other);
    assert_eq!(document("write", &output)["revision"], 3);
}

#[test]
fn a_batch_that_sets_an_answer_or_an_outcome_is_refused() {
    let yard = Yard::new();
    let answered = r#"[{"id": "s", "kind": "stage", "title": "S"},
        {"id": "q", "kind": "question", "stage": "s", "prompt": "P?", "header": "H",
         "options": [{"label": "A"}, {"label": "B"}], "answer": {"labels": ["A"]}}]"#;
    let output = yard.run_with(&["write", "--session", "s1"], answered);
    assert_eq!(output.status.code(), Some(1));
    let message = document("write", &output)["errors"][0]["message"].clone();
    assert!(message.as_str().unwrap().contains("`answer`"), "{message}");
    let decided = r#"[{"id": "s", "kind": "stage", "title": "S"},
        {"id": "a", "kind": "approval", "stage": "s", "title": "T", "markdown": "M",
         "outcome": "approved"}]"#;
    let output = yard.run_with(&["write", "--session", "s1"], decided);
    assert_eq!(output.status.code(), Some(1));
    let message = document("write", &output)["errors"][0]["message"].clone();
    assert!(
        message.as_str().unwrap().contains("only the person"),
        "{message}"
    );
    assert!(!yard.scratch.0.join(".yard").exists());
}
