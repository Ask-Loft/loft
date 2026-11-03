//! The page server serves a registered tree whatever spelling of its path names it: a tree is
//! registered as git names its top, which on Windows is `C:/repo`, while a request resolves to the
//! canonical `\\?\C:\repo`.

use std::fs;

use serde_json::json;

use crate::served::Served;
use crate::yard::{ASK, BRANCH, Yard};
use crate::yard_serve_logs::tree;

#[test]
fn a_tree_registered_under_another_spelling_of_its_path_is_served() {
    // A spelling with `..` makes the mismatch Windows makes, on every platform.
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &[]);
    let spelled = format!("{}/.git/..", tree(&yard.scratch));
    fs::write(
        yard.scratch.home().join(".yard").join("trees.json"),
        json!({ "trees": [spelled] }).to_string(),
    )
    .unwrap();
    let read = served.get(&format!("/api/log?tree={}", tree(&yard.scratch)));
    assert_eq!(read.status, 200, "{}", read.json());
    assert_eq!(read.json()["yardLog"]["branch"], BRANCH);
}
