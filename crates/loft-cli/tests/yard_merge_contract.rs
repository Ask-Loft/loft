//! Yard Log written apart on two clones of one branch, merged by a real `git merge` with
//! `loft merge-driver` registered for `.yard/` as `--install` registers it, then settled by the
//! next `loft yard` command. git calls the driver and Loft never calls git's merge machinery, so
//! no simulator stands in for it: this suite holds the driver to git's contract directly.

use std::fs;
use std::path::Path;
use std::process::Output;

use loftspec_store::merge_driver::{ATTRIBUTE, ATTRIBUTES};
use serde_json::Value;
use software_plant::Clock;
use software_plant::real_git::setup_git;

use crate::common::{EPOCH, Scratch, stderr, stdout};
use crate::repo::Repo;
use crate::yard::{ASK, block};

/// The branch both clones work on.
const BRANCH: &str = "feat";

/// Two clones of one repository on [`BRANCH`], `.gitattributes` routing both `spec/` and
/// `.yard/` to the driver, each with the person named.
struct Clones {
    ours: Repo,
    theirs: Repo,
}

impl Clones {
    fn new() -> Self {
        let ours = Repo::new();
        let attributes = format!("{ATTRIBUTE}\n{}\n", yardlog_store::ATTRIBUTE);
        fs::write(ours.0.0.join(ATTRIBUTES), attributes).unwrap();
        ours.git_ok(&["checkout", "--quiet", "-b", BRANCH]);
        ours.commit("route .yard/ to the driver");
        let theirs = Repo(Scratch::empty());
        let cloned = setup_git()
            .args(["clone", "--quiet"])
            .arg(&ours.0.0)
            .arg(&theirs.0.0)
            .output()
            .unwrap();
        assert!(cloned.status.success(), "{}", stderr(&cloned));
        for repo in [&ours, &theirs] {
            repo.git_ok(&["config", "user.name", "Ada Reader"]);
            repo.git_ok(&["config", "user.email", "ada@example.com"]);
        }
        Self { ours, theirs }
    }

    /// Commits what each clone wrote, brings this clone's commits to the other.
    fn share(&self) {
        self.ours.commit("ours");
        self.theirs
            .git_ok(&["pull", "--quiet", "--no-rebase", "origin", BRANCH]);
    }

    /// Commits what each clone wrote and merges the other clone's branch into this one.
    fn merge(&self) -> Output {
        self.ours.commit("ours");
        self.theirs.commit("theirs");
        let theirs = self.theirs.0.0.to_str().unwrap();
        self.ours.git_ok(&["fetch", "--quiet", theirs, BRANCH]);
        self.ours.git(&["merge", "--no-edit", "FETCH_HEAD"])
    }
}

/// Runs `loft yard <args>` in `repo` under real git, `seconds` after [`EPOCH`], with `input`
/// given through `--input` where there is one, and gives its JSON.
fn yard(repo: &Repo, seconds: u64, args: &[&str], input: Option<&str>) -> Value {
    let path = repo.0.home().join("input.json");
    let mut all = vec!["yard"];
    all.extend_from_slice(args);
    if let Some(input) = input {
        fs::write(&path, input).unwrap();
        all.extend_from_slice(&["--input", path.to_str().unwrap()]);
    }
    all.extend_from_slice(&["--format", "json"]);
    let output = repo
        .0
        .loft_under_real_git_at(&all, &Clock::fixed(EPOCH + seconds));
    assert!(
        output.status.success(),
        "loft yard {args:?}: {}{}",
        stdout(&output),
        stderr(&output)
    );
    serde_json::from_slice(&output.stdout).unwrap()
}

fn write(repo: &Repo, seconds: u64, session: &str, batch: &str) -> Value {
    yard(repo, seconds, &["write", "--session", session], Some(batch))
}

fn read(repo: &Repo, seconds: u64, extra: &[&str]) -> Value {
    let mut args = vec!["read", "--session", "s1"];
    args.extend_from_slice(extra);
    yard(repo, seconds, &args, None)
}

fn note(id: &str, text: &str) -> String {
    format!(r#"[{{"id": "{id}", "kind": "note", "stage": "shape", "markdown": "{text}"}}]"#)
}

/// Every file under `dir`, with its text.
fn texts(dir: &Path) -> Vec<(String, String)> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(texts(&path));
        } else {
            found.push((
                path.display().to_string(),
                fs::read_to_string(path).unwrap(),
            ));
        }
    }
    found
}

/// Asserts that a merge succeeded and left no conflict marker in Yard Log's files.
fn merged_cleanly(clones: &Clones, merged: &Output) {
    assert_eq!(merged.status.code(), Some(0), "{}", stderr(merged));
    for (path, text) in texts(&clones.ours.0.0.join(".yard")) {
        assert!(!text.contains("<<<<<<<"), "{path}: {text}");
    }
}

fn ids(list: &Value) -> Vec<&str> {
    list.as_array()
        .unwrap()
        .iter()
        .map(|item| item["id"].as_str().unwrap())
        .collect()
}

#[test]
fn notes_written_apart_merge_cleanly_and_the_next_read_settles_their_shared_id() {
    let clones = Clones::new();
    write(&clones.ours, 0, "s1", ASK);
    clones.share();
    write(&clones.ours, 60, "s1", &note("n1", "Ours"));
    write(&clones.theirs, 120, "s2", &note("n1", "Theirs"));
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let settled = read(&clones.ours, 180, &[]);
    assert_eq!(block(&settled, "n1")["version"]["body"]["markdown"], "Ours");
    assert_eq!(
        block(&settled, "n1-2")["version"]["body"]["markdown"],
        "Theirs"
    );
    let since = read(&clones.ours, 180, &["--since", "2"]);
    assert_eq!(ids(&since["stages"][0]["blocks"]), ["n1-2"]);
}

#[test]
fn stages_added_on_each_clone_are_each_in_a_file_of_its_own_and_kept_base_first() {
    let clones = Clones::new();
    write(&clones.ours, 0, "s1", ASK);
    clones.share();
    let stage = |id: &str| format!(r#"[{{"id": "{id}", "kind": "stage", "title": "{id}"}}]"#);
    write(&clones.ours, 60, "s1", &stage("build"));
    write(&clones.theirs, 120, "s2", &stage("review"));
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let settled = read(&clones.ours, 180, &[]);
    assert_eq!(ids(&settled["stages"]), ["shape", "build", "review"]);
    let folder = clones.ours.0.0.join(".yard").join(BRANCH);
    assert_eq!(fs::read_dir(folder).unwrap().count(), 4);
}

#[test]
fn answers_given_apart_count_the_earlier_and_keep_the_later_as_it_was_written() {
    let clones = Clones::new();
    yard(
        &clones.ours,
        0,
        &["write", "--session", "s1", "--send"],
        Some(ASK),
    );
    clones.share();
    let answer = |label: &str| format!(r#"[{{"block": "q1", "labels": ["{label}"]}}]"#);
    let args = ["answer", "--session", "s1"];
    yard(&clones.ours, 120, &args, Some(&answer("Top")));
    yard(&clones.theirs, 60, &args, Some(&answer("Bottom")));
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let settled = read(&clones.ours, 180, &["--versions"]);
    let question = block(&settled, "q1");
    assert_eq!(question["answer"]["labels"][0], "Bottom", "{question}");
    let labels: Vec<&Value> = question["responses"]
        .as_array()
        .unwrap()
        .iter()
        .map(|response| &response["labels"][0])
        .collect();
    assert!(labels.contains(&&Value::from("Top")), "{question}");
}

#[test]
fn yard_log_closed_on_one_clone_is_closed_after_the_merge() {
    let clones = Clones::new();
    write(&clones.ours, 0, "s1", ASK);
    clones.share();
    yard(&clones.ours, 60, &["close", "--session", "s1"], None);
    write(&clones.theirs, 120, "s2", &note("n1", "Theirs"));
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let settled = read(&clones.ours, 180, &[]);
    assert!(settled["yardLog"]["closed"].is_object(), "{settled}");
}

#[test]
fn yard_logs_opened_apart_on_two_clones_join_under_the_key_of_the_one_opened_earlier() {
    let clones = Clones::new();
    write(&clones.ours, 60, "s1", ASK);
    write(&clones.theirs, 0, "s2", ASK);
    let key = |repo: &Repo| {
        read(repo, 30, &[])["yardLog"]["key"]
            .as_str()
            .unwrap()
            .to_owned()
    };
    let (ours_key, theirs_key) = (key(&clones.ours), key(&clones.theirs));
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let settled = read(&clones.ours, 180, &[]);
    assert_eq!(settled["yardLog"]["key"], theirs_key.as_str());
    assert_eq!(settled["yardLog"]["merged"][0], ours_key.as_str());
    // This copy's stages come first, and its `shape`, written later, takes the suffix.
    assert_eq!(ids(&settled["stages"]), ["shape-2", "shape"]);
}

#[test]
fn a_session_s_tokens_file_changed_on_both_clones_is_taken_from_the_later_without_conflict() {
    let clones = Clones::new();
    write(&clones.ours, 0, "s1", ASK);
    let tokens = |repo: &Repo, last: &str, requests: u32| {
        let dir = repo.0.0.join(".yard").join(BRANCH).join("tokens");
        fs::create_dir_all(&dir).unwrap();
        let text = format!("session: s1\nlastEnded: {last}\nrequests: {requests}\n");
        fs::write(dir.join("s1.yaml"), text).unwrap();
    };
    tokens(&clones.ours, "2026-01-01T00:00:30Z", 1);
    clones.share();
    tokens(&clones.ours, "2026-01-01T00:02:00Z", 3);
    tokens(&clones.theirs, "2026-01-01T00:01:00Z", 2);
    let merged = clones.merge();
    merged_cleanly(&clones, &merged);
    let file = clones
        .ours
        .0
        .0
        .join(".yard")
        .join(BRANCH)
        .join("tokens/s1.yaml");
    assert!(fs::read_to_string(file).unwrap().contains("requests: 3"));
}

/// A third clone of `from`, on [`BRANCH`], with the person named.
fn third_clone(from: &Repo) -> Repo {
    let third = Repo(Scratch::empty());
    let cloned = setup_git()
        .args(["clone", "--quiet", "--branch", BRANCH])
        .arg(&from.0.0)
        .arg(&third.0.0)
        .output()
        .unwrap();
    assert!(cloned.status.success(), "{}", stderr(&cloned));
    third.git_ok(&["config", "user.name", "Ada Reader"]);
    third.git_ok(&["config", "user.email", "ada@example.com"]);
    third
}

#[test]
fn a_stage_another_clone_renamed_by_its_settling_merges_in_and_reads_by_its_new_id() {
    let clones = Clones::new();
    write(&clones.ours, 60, "s1", ASK);
    clones.ours.commit("ours");
    let before = third_clone(&clones.ours);
    write(&clones.theirs, 0, "s2", ASK);
    clones.theirs.commit("theirs");
    let theirs = clones.theirs.0.0.to_str().unwrap();
    clones.ours.git_ok(&["fetch", "--quiet", theirs, BRANCH]);
    merged_cleanly(
        &clones,
        &clones.ours.git(&["merge", "--no-edit", "FETCH_HEAD"]),
    );
    let settled = read(&clones.ours, 180, &[]);
    assert_eq!(ids(&settled["stages"]), ["shape-2", "shape"]);
    clones.ours.commit("settled");
    let stage = r#"[{"id": "build", "kind": "stage", "title": "Build"}]"#;
    write(&before, 200, "s3", stage);
    before.commit("before");
    let ours = clones.ours.0.0.to_str().unwrap();
    before.git_ok(&["fetch", "--quiet", ours, BRANCH]);
    let merged = before.git(&["merge", "--no-edit", "FETCH_HEAD"]);
    assert_eq!(merged.status.code(), Some(0), "{}", stderr(&merged));
    let read_back = read(&before, 240, &[]);
    assert_eq!(ids(&read_back["stages"]), ["shape-2", "build", "shape"]);
}
