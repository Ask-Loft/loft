use std::collections::HashSet;
use std::path::{Path, PathBuf};

use serde_json::json;

use super::{Entries, Entry};

#[test]
fn same_second_waiting_submissions_follow_local_arrival_within_each_log() {
    let wait = |tree, key, original, arrived| {
        json!({
            "tree": tree, "branch":"feat/shelf", "submission":key,
            "sent":{"at":"2026-10-22T09:30:00Z","revision":original}, "sentReached":arrived,
        })
    };
    let mut waits = vec![
        wait("/a", "later", 1, 9),
        wait("/b", "other", 1, 1),
        wait("/a", "earlier", 80, 7),
    ];
    super::order_waiting(&mut waits);
    assert_eq!(
        waits
            .iter()
            .map(|w| w["submission"].as_str().unwrap())
            .collect::<Vec<_>>(),
        ["earlier", "later", "other"]
    );
}

#[test]
fn the_page_server_forgets_what_it_read_of_a_yard_log_no_tree_holds_any_more() {
    let entries = Entries::default();
    let key = |tree: &str, branch: &str| (PathBuf::from(tree), branch.to_owned());
    for (tree, branch) in [("/work/a", "main"), ("/work/a", "old"), ("/work/b", "main")] {
        let entry = Entry {
            stamp: (1, None),
            inherited: false,
            summary: json!({}),
            waits: Vec::new(),
        };
        entries.0.lock().unwrap().insert(key(tree, branch), entry);
    }
    entries.keep_only(&HashSet::from([key("/work/a", "main")]));
    let kept: Vec<_> = entries.0.lock().unwrap().keys().cloned().collect();
    assert_eq!(kept, [key("/work/a", "main")]);
}

#[test]
fn the_top_of_a_worktree_is_spelled_as_the_directory_registered_spells_it() {
    let spelled = |dir: &str, real: &str, tree: &str| {
        super::spelled_top(Path::new(dir), Path::new(real), Path::new(tree))
    };
    assert_eq!(
        spelled(
            "/tmp/w/crates/a",
            "/private/tmp/w/crates/a",
            "/private/tmp/w"
        ),
        Some(PathBuf::from("/tmp/w"))
    );
    assert_eq!(
        spelled("/tmp/w", "/private/tmp/w", "/private/tmp/w"),
        Some(PathBuf::from("/tmp/w"))
    );
    // A link inside the worktree leaves no spelling of its top to tell.
    assert_eq!(spelled("/w/link", "/w/crates/a", "/w"), None);
    assert_eq!(spelled("crates", "/w/crates", "/w"), None);
    assert_eq!(spelled("/elsewhere/x", "/w/x", "/other"), None);
}
