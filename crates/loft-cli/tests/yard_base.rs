//! The base branch the write that opens Yard Log records: the one `--base` names; else the local
//! branch git's reflog says the branch was cut from; else the one `--default-base` names; else
//! `develop`. A view with no base of its own compares with it.

use crate::base::based_on;
use crate::yard::{ASK, BRANCH, HEAD, Yard};

/// A batch that opens Yard Log with a spec-change view and no base of its own.
const VIEW: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "v1", "kind": "loft", "stage": "shape", "view": "spec-change"}
]"#;

/// The base branch Yard Log records once `yard` wrote [`ASK`] with `extra` arguments.
fn recorded(yard: &Yard, extra: &[&str]) -> String {
    let written = yard.write("s1", ASK, extra);
    assert_eq!(written["outcome"], "written", "{written}");
    yard.read("s1", &[])["yardLog"]["base"]
        .as_str()
        .unwrap()
        .to_owned()
}

/// A repository on [`BRANCH`] whose reflog gives `subjects`, newest first.
fn with_reflog(subjects: &[&str]) -> Yard {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().with_reflog(BRANCH, subjects);
    yard
}

#[test]
fn yard_log_opened_without_base_records_the_local_branch_its_branch_was_cut_from() {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().cut_from("feat/yard");
    assert_eq!(recorded(&yard, &[]), "feat/yard");
    let mut later = Yard::new();
    later.git = later
        .git
        .clone()
        .cut_from("feat/yard")
        .with_reflog(BRANCH, &["commit: later", "branch: Created from feat/yard"]);
    assert_eq!(recorded(&later, &["--default-base", "trunk"]), "feat/yard");
}

#[test]
fn a_branch_cut_from_no_other_local_branch_records_the_default_base_or_develop() {
    let commit = format!("branch: Created from {HEAD}");
    let itself = format!("branch: Created from {BRANCH}");
    let cases: [&[&str]; 6] = [
        &["branch: Created from HEAD"],
        &[commit.as_str()],
        &["branch: Created from origin/feat/yard"],
        &[itself.as_str()],
        &["commit (initial): start"],
        &[],
    ];
    for subjects in cases {
        assert_eq!(
            recorded(&with_reflog(subjects), &["--default-base", "trunk"]),
            "trunk",
            "{subjects:?}"
        );
        assert_eq!(
            recorded(&with_reflog(subjects), &[]),
            "develop",
            "{subjects:?}"
        );
    }
    assert_eq!(
        recorded(&Yard::new(), &["--default-base", "trunk"]),
        "trunk"
    );
}

#[test]
fn yard_log_opened_with_base_records_it_whichever_branch_git_names() {
    let mut yard = Yard::new();
    yard.git = yard.git.clone().cut_from("feat/yard");
    assert_eq!(
        recorded(&yard, &["--base", "main", "--default-base", "trunk"]),
        "main"
    );
}

#[test]
fn a_view_written_as_yard_log_opens_compares_with_the_branch_it_was_cut_from() {
    let mut yard = Yard::with_spec();
    yard.git = based_on(&yard.scratch, "feat/yard")
        .base("HEAD", HEAD)
        .with_config("user.name", "Ada Reader")
        .with_config("user.email", "ada@example.com")
        .cut_from("feat/yard");
    let written = yard.write("s1", VIEW, &[]);
    assert_eq!(written["outcome"], "written", "{written}");
    let read = yard.read("s1", &["--reports"]);
    assert_eq!(read["yardLog"]["base"], "feat/yard");
    let view = &crate::yard::block(&read, "v1")["version"]["body"]["report"];
    assert_eq!(view["status"]["base"]["revision"], "feat/yard", "{view}");
}
