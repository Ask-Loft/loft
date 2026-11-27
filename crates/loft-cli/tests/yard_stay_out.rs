//! Yard Client in a working tree that does not use Loft: `loft yard protocol` says whether a
//! working tree uses Loft and what shows it, and the Claude and Codex hooks answer nothing and
//! write nothing where none does, while a specification, its lock, a Yard Log of any branch or
//! `loft.yard` in git's configuration turn them on.

use std::fs;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::{Clock, GitScenario};
use yardlog_store::lock;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::served::{Served, within};
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, document};
use crate::yard_claude_hook::{context, hook as claude, input};
use crate::yard_codex::{event, hook as codex, watch};
use crate::yard_serve_events::log;

/// What `loft yard protocol` reports of the working tree of `dir`, a directory below the scratch
/// tree, with `git` answering, or with no `git` on the search path at all.
fn working_tree(scratch: &Scratch, dir: &str, git: Option<&GitScenario>) -> Value {
    let output = scratch
        .command(git, &Clock::fixed(EPOCH))
        .current_dir(scratch.0.join(dir))
        .args(["yard", "protocol", "--format", "json"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", stderr(&output));
    document("protocol", &output)["workingTree"].clone()
}

/// Makes the directories of `path`, below the scratch tree.
fn mkdir(scratch: &Scratch, path: &str) {
    fs::create_dir_all(scratch.0.join(path)).unwrap();
}

/// A Claude Code session start of the source `startup` for session `s1`.
fn claude_start(yard: &Yard) -> Value {
    let mut start = input(yard, "SessionStart", "s1");
    start["source"] = json!("startup");
    start
}

/// Each hook of a Claude Code session that the plugin's command hooks run the command for, in
/// the order a session meets them, `/clear` and plan mode included.
fn claude_session(yard: &Yard) -> Vec<Value> {
    let mut subagent = input(yard, "SubagentStart", "s1");
    subagent["agent_id"] = json!("child");
    subagent["agent_type"] = json!("reviewer");
    let mut planning = input(yard, "UserPromptSubmit", "s1");
    planning["permission_mode"] = json!("plan");
    let mut plan = input(yard, "PermissionRequest", "s1");
    plan["tool_name"] = json!("ExitPlanMode");
    plan["tool_input"] = json!({"plan": "Build it."});
    let mut ended = input(yard, "SessionEnd", "s1");
    ended["reason"] = json!("clear");
    let mut cleared = input(yard, "SessionStart", "s2");
    cleared["source"] = json!("clear");
    vec![
        claude_start(yard),
        subagent,
        planning,
        input(yard, "PostToolUse", "s1"),
        input(yard, "Stop", "s1"),
        input(yard, "PostCompact", "s1"),
        plan,
        ended,
        cleared,
    ]
}

/// Each hook of a Codex session, a subagent's included.
fn codex_session(yard: &Yard) -> Vec<Value> {
    let mut subagent = event(yard, "SubagentStart");
    subagent["agent_id"] = json!("child");
    subagent["agent_type"] = json!("reviewer");
    [
        "SessionStart",
        "UserPromptSubmit",
        "PreToolUse",
        "PostToolUse",
    ]
    .into_iter()
    .map(|name| event(yard, name))
    .chain([subagent])
    .chain(
        ["PostCompact", "Stop", "Interrupt", "SessionEnd"]
            .into_iter()
            .map(|name| event(yard, name)),
    )
    .collect()
}

#[test]
fn a_directory_below_one_that_holds_the_specification_uses_loft_with_no_git_asked() {
    let scratch = Scratch::with_reading_list();
    mkdir(&scratch, "src/deep");
    assert_eq!(
        working_tree(&scratch, "src/deep", None),
        json!({"usesLoft": true, "by": "spec"})
    );
}

#[test]
fn a_directory_below_one_that_holds_the_lock_uses_loft() {
    let scratch = Scratch::empty();
    fs::write(scratch.0.join("loft.lock.json"), "{}\n").unwrap();
    mkdir(&scratch, "a/b");
    assert_eq!(
        working_tree(&scratch, "a/b", None),
        json!({"usesLoft": true, "by": "lock"})
    );
}

#[test]
fn a_worktree_that_holds_yard_log_of_another_branch_uses_loft_and_briefs_its_own_branch() {
    let mut yard = Yard::on("develop");
    yard.write("s0", ASK, &[]);
    yard.git = Yard::new().git;
    assert_eq!(
        working_tree(&yard.scratch, ".", Some(&yard.git)),
        json!({"usesLoft": true, "by": "yard-log"})
    );
    let started = claude(&yard, &claude_start(&yard), None);
    assert!(
        context(&started).is_some_and(|text| text.contains("no Yard Log yet")),
        "{started}"
    );
}

#[test]
fn a_spec_directory_without_the_manifest_and_a_yard_directory_without_yard_log_show_nothing() {
    let yard = Yard::new();
    mkdir(&yard.scratch, "spec/models");
    fs::write(yard.scratch.0.join("spec/models/book_spec.rb"), "").unwrap();
    mkdir(&yard.scratch, ".yard/feat/shelf");
    fs::write(yard.scratch.0.join(".yard/notes.txt"), "").unwrap();
    assert_eq!(
        working_tree(&yard.scratch, ".", Some(&yard.git)),
        json!({"usesLoft": false})
    );
    assert_eq!(
        claude(&yard, &claude_start(&yard), None)["hookOutput"],
        json!({})
    );
}

#[test]
fn a_specification_above_the_top_of_the_worktree_shows_nothing() {
    let scratch = Scratch::with_reading_list();
    mkdir(&scratch, "vendor/clone/.git");
    mkdir(&scratch, "vendor/clone/src");
    mkdir(&scratch, "vendor/module/src");
    fs::write(
        scratch.0.join("vendor/module/.git"),
        "gitdir: ../../.git/modules/module\n",
    )
    .unwrap();
    for dir in ["vendor/clone/src", "vendor/module/src"] {
        assert_eq!(
            working_tree(&scratch, dir, None),
            json!({"usesLoft": false}),
            "{dir}"
        );
    }
    assert_eq!(
        working_tree(&scratch, "vendor", None),
        json!({"usesLoft": true, "by": "spec"})
    );
}

#[test]
fn loft_yard_set_to_true_turns_yard_client_on_in_each_worktree_of_the_repository() {
    let yard = Yard::opted_in();
    assert_eq!(
        working_tree(&yard.scratch, ".", Some(&yard.git)),
        json!({"usesLoft": true, "by": "config"})
    );
    let another = Yard {
        scratch: Scratch::empty(),
        git: yard.git.clone(),
        clock: yard.clock.clone(),
    };
    for each in [&yard, &another] {
        let started = claude(each, &claude_start(each), None);
        assert!(context(&started).is_some(), "{started}");
        let started = codex(each, &event(each, "SessionStart"));
        assert!(
            started["hookOutput"]["hookSpecificOutput"]["additionalContext"].is_string(),
            "{started}"
        );
    }
}

#[test]
fn loft_yard_set_to_false_to_no_boolean_or_unset_turns_nothing_on() {
    for value in [
        Some("false"),
        Some("no"),
        Some("off"),
        Some("0"),
        Some(""),
        Some("maybe"),
        None,
    ] {
        let mut yard = Yard::new();
        if let Some(value) = value {
            yard.git = yard.git.clone().with_config("loft.yard", value);
        }
        assert_eq!(
            working_tree(&yard.scratch, ".", Some(&yard.git)),
            json!({"usesLoft": false}),
            "{value:?}"
        );
        assert_eq!(
            claude(&yard, &claude_start(&yard), None)["hookOutput"],
            json!({}),
            "{value:?}"
        );
    }
}

#[test]
fn what_the_working_tree_holds_shows_it_before_loft_yard_is_read() {
    let scratch = Scratch::with_reading_list();
    let git = GitScenario::clean_on(BRANCH).with_config("loft.yard", "true");
    assert_eq!(
        working_tree(&scratch, ".", Some(&git)),
        json!({"usesLoft": true, "by": "spec"})
    );
}

#[test]
fn in_an_empty_repository_the_claude_hooks_answer_nothing_and_write_nothing() {
    let yard = Yard::new();
    let before = snapshot(&yard.scratch.0);
    for hook in claude_session(&yard) {
        let answered = claude(&yard, &hook, Some("4242"));
        assert_eq!(answered["hookOutput"], json!({}), "{hook}");
    }
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
    assert!(!yard.scratch.0.join(".git").exists(), "git's directory");
    assert!(!yard.scratch.home().join(".yard").exists(), "~/.yard/");
}

#[test]
fn in_an_empty_repository_the_codex_hooks_answer_nothing_and_write_nothing() {
    let yard = Yard::new();
    let before = snapshot(&yard.scratch.0);
    for hook in codex_session(&yard) {
        assert_eq!(codex(&yard, &hook)["hookOutput"], json!({}), "{hook}");
    }
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
    assert!(!yard.scratch.0.join(".git").exists(), "git's directory");
    assert!(!yard.scratch.home().join(".yard").exists(), "~/.yard/");
}

#[test]
fn outside_a_git_repository_the_codex_hooks_give_no_context() {
    let mut yard = Yard::new();
    yard.git = GitScenario::outside_repository();
    for name in ["SessionStart", "UserPromptSubmit"] {
        assert_eq!(
            codex(&yard, &event(&yard, name))["hookOutput"],
            json!({}),
            "{name}"
        );
    }
    assert!(!yard.scratch.home().join(".yard").exists());
}

#[test]
fn in_an_empty_repository_the_codex_watch_starts_no_worker_and_no_page_server() {
    let yard = Yard::new();
    let output = yard.run_with(
        &["codex", "watch"],
        &event(&yard, "SessionStart").to_string(),
    );
    assert!(output.status.success(), "{}", stderr(&output));
    assert_eq!(document("codex-watch", &output)["hookOutput"], json!({}));
    assert!(!yard.scratch.home().join(".yard").exists());
}

#[test]
fn a_running_page_server_hears_nothing_of_a_session_in_an_empty_repository() {
    // A page server that serves a tree with Yard Log, in the machine home the hooks share.
    let served_tree = Yard::new();
    served_tree.write("s1", ASK, &["--send"]);
    let _served = Served::start(
        &served_tree,
        &["--tree", &served_tree.scratch.0.display().to_string()],
    );
    let elsewhere = Scratch::empty();
    let mut hooks = Vec::new();
    for (event, mode) in [
        ("UserPromptSubmit", "plan"),
        ("Stop", "default"),
        ("PostCompact", "default"),
    ] {
        let mut each = input(&served_tree, event, "s9");
        each["cwd"] = json!(elsewhere.0.canonicalize().unwrap());
        each["permission_mode"] = json!(mode);
        hooks.push(each);
    }
    for each in &hooks {
        assert_eq!(
            claude(&served_tree, each, None)["hookOutput"],
            json!({}),
            "{each}"
        );
    }
    assert_eq!(log(&served_tree, "s9"), Vec::<Value>::new());
    let heard = fs::read_to_string(served_tree.scratch.home().join(".yard/sessions.json"))
        .unwrap_or_default();
    assert!(!heard.contains("\"s9\""), "{heard}");
}

#[test]
fn without_json_the_report_says_whether_the_working_tree_uses_loft_and_how_to_turn_it_on() {
    let empty = Scratch::empty();
    let said = stdout(&empty.loft(&["yard", "protocol"]));
    assert!(
        said.contains("This working tree does not use Loft")
            && said.contains("`git config loft.yard true` turns it on"),
        "{said}"
    );
    let specified = Scratch::with_reading_list();
    let said = stdout(&specified.loft(&["yard", "protocol"]));
    assert!(
        said.contains("This working tree uses Loft: `spec/loftspec.yaml` shows it."),
        "{said}"
    );
}

#[cfg(unix)]
#[test]
fn a_directory_reached_through_a_link_is_looked_at_by_its_real_path() {
    use std::os::unix::fs::symlink;
    let specified = Yard::with_spec();
    mkdir(&specified.scratch, "src/deep");
    let links = Scratch::empty();
    symlink(specified.scratch.0.join("src/deep"), links.0.join("into")).unwrap();
    let mut start = claude_start(&specified);
    start["cwd"] = json!(links.0.join("into"));
    assert!(context(&claude(&specified, &start, None)).is_some());
    let plain = Scratch::empty();
    mkdir(&plain, "sub");
    symlink(plain.0.join("sub"), specified.scratch.0.join("linked")).unwrap();
    start["cwd"] = json!(specified.scratch.0.join("linked"));
    assert_eq!(claude(&specified, &start, None)["hookOutput"], json!({}));
    assert!(!plain.0.join(".git").exists());
}

#[test]
fn a_session_that_continues_one_served_through_another_worktree_is_briefed_after_clear() {
    let yard = Yard::new();
    let there = Scratch::empty();
    yard.write(
        "s1",
        r#"[{"id":"there","kind":"stage","title":"There","status":"active"}]"#,
        &["--tree", &there.0.display().to_string()],
    );
    let mut ended = input(&yard, "SessionEnd", "s1");
    ended["reason"] = json!("clear");
    claude(&yard, &ended, Some("4242"));
    let mut cleared = input(&yard, "SessionStart", "s2");
    cleared["source"] = json!("clear");
    let briefed = claude(&yard, &cleared, Some("4242"));
    assert!(
        context(&briefed).is_some_and(|text| text.contains("There")),
        "{briefed}"
    );
    assert_eq!(yard.home_file("continuations.json"), json!({"s2": "s1"}));
}

#[test]
fn a_worktree_a_session_once_wrote_yard_log_in_gets_no_lock_once_its_yard_log_is_gone() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    fs::remove_dir_all(yard.scratch.0.join(".yard")).unwrap();
    fs::remove_dir_all(yard.scratch.0.join(".git")).unwrap();
    yard.git = Yard::on("other").git;
    let answered = claude(&yard, &claude_start(&yard), None);
    assert_eq!(answered["hookOutput"], json!({}));
    assert!(!yard.scratch.0.join(".git").exists());
}

#[test]
fn a_codex_worker_stops_at_its_session_s_end_after_the_tree_stops_using_loft() {
    let yard = Yard::opted_in();
    let served = Served::start(&yard, &[]);
    let input = event(&yard, "UserPromptSubmit");
    codex(&yard, &input);
    let mut worker = watch(
        &yard,
        &input,
        &yard.clock,
        &["--port", &served.port.to_string()],
    );
    let held = yard
        .scratch
        .home()
        .join(".yard/codex/codex-session/worker.lock");
    assert!(within(Duration::from_secs(3), || held.exists()
        && lock::is_held(&held).unwrap()));
    let mut end = event(&yard, "SessionEnd");
    end["cwd"] = json!(yard.scratch.0.canonicalize().unwrap());
    let path = yard.scratch.home().join("end.json");
    fs::write(&path, end.to_string()).unwrap();
    let ended = yard
        .scratch
        .command(Some(&Yard::new().git), &yard.clock)
        .args(["yard", "codex", "hook", "--input", path.to_str().unwrap()])
        .args(["--format", "json"])
        .output()
        .unwrap();
    assert!(ended.status.success(), "{}", stderr(&ended));
    assert_eq!(document("codex-hook", &ended)["hookOutput"], json!({}));
    assert!(within(Duration::from_secs(3), || worker
        .0
        .try_wait()
        .unwrap()
        .is_some()));
}
