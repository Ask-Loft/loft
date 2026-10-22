//! Agents as callers of the `loft yard` commands: the session and subagent each command names,
//! the worktree its directory is in, and a session that continues another, as after `/clear`,
//! counted as the first session of its chain for the submissions it sent and the revision it
//! last read, while every Act keeps the session id that made it.

use serde_json::json;

use crate::common::stderr;
use crate::yard::{ASK, BRANCH, Yard, block, document};

#[test]
fn a_command_run_without_agent_acts_for_the_session_s_main_agent() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let read = yard.read("s1", &[]);
    let by = &block(&read, "q1")["version"]["act"]["by"];
    assert_eq!(by, &json!({ "host": "claude-code", "session": "s1" }));
}

#[test]
fn a_changing_command_run_without_session_exits_2_and_names_session() {
    let yard = Yard::new();
    let runs: [&[&str]; 4] = [
        &["write", "--input", "-"],
        &[
            "withdraw",
            "--submission",
            "01a16502-f6d2-7666-adad-3403c0a92870",
        ],
        &["close"],
        &["answer", "--input", "-"],
    ];
    for args in runs {
        let output = yard.run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        assert!(stderr(&output).contains("--session"), "{args:?}");
    }
}

#[test]
fn agent_without_agent_type_or_agent_type_without_agent_exits_2_naming_the_one_missing() {
    let yard = Yard::new();
    let output = yard.run(&["read", "--session", "s1", "--agent", "a1"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--agent-type"),
        "{}",
        stderr(&output)
    );
    let output = yard.run(&["read", "--session", "s1", "--agent-type", "plan"]);
    assert_eq!(output.status.code(), Some(2));
    assert!(
        stderr(&output).contains("--agent <ID>"),
        "{}",
        stderr(&output)
    );
}

#[test]
fn every_block_withdrawal_and_close_records_the_session_and_agent_that_made_it() {
    let yard = Yard::new();
    let sub = ["--agent", "a9", "--agent-type", "explorer"];
    let mut args = vec!["write", "--session", "s3", "--send"];
    args.extend(sub);
    let written = document("write", &yard.run_with(&args, ASK));
    let key = written["submission"].as_str().unwrap();
    let read = yard.read("s1", &[]);
    let by = &block(&read, "q1")["version"]["act"]["by"];
    assert_eq!(by["agent"], "a9");
    let mut withdraw = vec!["withdraw", "--session", "s3", "--submission", key];
    withdraw.extend(sub);
    yard.run(&withdraw);
    let mut close = vec!["close", "--session", "s3"];
    close.extend(sub);
    yard.run(&close);
    let read = yard.read("s1", &[]);
    assert_eq!(
        read["yardLog"]["closed"]["by"],
        json!({ "host": "claude-code", "session": "s3", "agent": "a9", "agentType": "explorer" })
    );
    let root = std::fs::read_to_string(yard.folder(BRANCH).join("yardlog.yaml")).unwrap();
    // The opening, the send, the withdrawal and the close, then the Act of each of the three
    // changes in the root's Acts.
    assert_eq!(root.matches("agent: \"a9\"").count(), 7, "{root}");
}

#[test]
fn a_command_run_below_the_top_of_a_worktree_acts_on_yard_log_at_its_top() {
    let mut yard = Yard::new();
    let below = yard.scratch.0.join("shelves/top");
    std::fs::create_dir_all(&below).unwrap();
    yard.git = yard.git.clone().below("shelves/top/");
    let mut command = yard.scratch.command(Some(&yard.git), &yard.clock);
    let input = yard.scratch.home().join("batch.json");
    std::fs::write(&input, ASK).unwrap();
    let output = command
        .current_dir(&below)
        .args([
            "yard",
            "write",
            "--session",
            "s1",
            "--format",
            "json",
            "--input",
        ])
        .arg(&input)
        .output()
        .unwrap();
    let written = document("write", &output);
    assert_eq!(written["outcome"], "written", "{}", stderr(&output));
    assert!(yard.folder(BRANCH).join("yardlog.yaml").is_file());
    assert!(!below.join(".yard").exists());
}

#[test]
fn each_result_names_the_tree_the_branch_and_the_repository_s_common_directory() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &[]);
    let tree = yard.scratch.0.canonicalize().unwrap();
    assert_eq!(written["tree"], tree.display().to_string().as_str());
    assert_eq!(written["branch"], BRANCH);
    assert_eq!(
        written["commonDir"],
        tree.join(".git").display().to_string().as_str()
    );
}

#[test]
fn a_continuation_is_kept_in_the_machine_s_home_and_never_under_yard() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &[]);
    let output = yard.run(&["read", "--session", "s2", "--continues", "s1"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(yard.home_file("continuations.json"), json!({ "s2": "s1" }));
    let yard_files = std::fs::read_dir(yard.folder(BRANCH)).unwrap().count();
    assert_eq!(yard_files, 2);
}

#[test]
fn a_submission_the_ended_session_sent_counts_as_sent_by_the_session_that_continues_it() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let other = r#"[{"id": "q2", "kind": "question", "stage": "shape", "prompt": "Why?",
        "header": "Why", "options": [{"label": "A"}, {"label": "B"}]}]"#;
    yard.write("s2", other, &["--send", "--continues", "s1"]);
    let read = yard.read("s2", &[]);
    let groups = read["submissions"].as_array().unwrap();
    assert_eq!(groups.len(), 1, "{groups:?}");
    assert_eq!(groups[0]["open"].as_array().unwrap().len(), 2);
}

#[test]
fn the_continuing_session_starts_from_the_revision_the_ended_session_last_read() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &[]);
    yard.write("s1", ASK, &[]);
    yard.read("s9", &[]);
    let read = document(
        "read",
        &yard.run(&["read", "--session", "s2", "--continues", "s1"]),
    );
    assert_eq!(read["lastRead"], 1);
}

#[test]
fn a_chain_of_continuations_reaches_back_to_the_first_session_of_the_chain() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.read("s1", &[]);
    yard.write("s1", ASK, &[]);
    yard.read("s9", &[]);
    yard.run(&["read", "--session", "s2", "--continues", "s1"]);
    yard.write("s1", ASK, &[]);
    let read = document(
        "read",
        &yard.run(&["read", "--session", "s3", "--continues", "s2"]),
    );
    assert_eq!(read["lastRead"], 2);
    assert_eq!(
        yard.home_file("continuations.json"),
        json!({ "s2": "s1", "s3": "s2" })
    );
}

#[test]
fn each_block_and_act_keeps_the_session_id_that_wrote_it() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.write("s2", ASK, &["--continues", "s1"]);
    let versions = block(&yard.read("s1", &["--versions"]), "q1")["versions"].clone();
    assert_eq!(versions[0]["act"]["by"]["session"], "s1");
    assert_eq!(versions[1]["act"]["by"]["session"], "s2");
}
