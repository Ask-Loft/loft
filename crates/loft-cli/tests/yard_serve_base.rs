//! The base branch a tree is registered with: the page server checks against it whether a
//! branch's Yard Log was closed by its merge, as the commands the mod runs check against the
//! plugin's `baseBranch`, so the page and the terminal agree.

use serde_json::json;

use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};

#[test]
fn the_page_server_checks_the_merge_against_the_base_branch_its_tree_was_registered_with() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding("main", &root, "2222222");
    let dir = yard.scratch.0.display().to_string();
    let served = Served::start(&yard, &["--tree", &dir, "--base", "main"]);
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let log = served
        .get(&format!("/api/log?tree={tree}&branch={BRANCH}"))
        .json();
    assert_eq!(log["yardLog"]["closedBy"], "merge", "{log}");
    let sent = json!({
        "tree": tree,
        "branch": BRANCH,
        "answers": [{ "block": "q1", "labels": ["Top"] }],
    });
    let answered = served.post_as_page("/api/answer", &sent).json();
    assert_eq!(answered["errors"][0]["kind"], "closed", "{answered}");
}
