//! `loft yard page` (the Give the Yard page's address Feature): the address of the branch's Yard
//! Log on the page, with a line on what the page shows, or that no page server serves it.

use std::collections::BTreeMap;
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Output;

use serde_json::json;
use yardlog_store::lock;

use crate::yard::{Yard, document, name_server};

/// Runs `loft yard page` as a person would, printing words, not JSON.
fn words(yard: &Yard) -> Output {
    yard.scratch
        .command(Some(&yard.git), &yard.clock)
        .args(["yard", "page"])
        .output()
        .unwrap()
}

/// Every file under `root`, by its path, with its bytes.
fn files(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut found = BTreeMap::new();
    let mut dirs = vec![root.to_path_buf()];
    while let Some(dir) = dirs.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            if path.is_dir() {
                dirs.push(path);
            } else {
                found.insert(path.display().to_string(), fs::read(&path).unwrap());
            }
        }
    }
    found
}

/// Runs `serve` while a page server on 7500 is named and holds its lock.
fn served<T>(yard: &Yard, serve: impl FnOnce() -> T) -> T {
    name_server(yard, 7500);
    let held = lock::lock(&yard.scratch.home().join(".yard/server.lock")).unwrap();
    let served = serve();
    drop(held);
    served
}

#[test]
fn the_page_command_gives_the_address_that_names_the_worktree_and_its_branch() {
    let yard = Yard::new();
    let output = served(&yard, || yard.run(&["page"]));
    assert!(output.status.success());
    let page = document("page", &output);
    let tree = page["tree"].as_str().unwrap().replace('/', "%2F");
    assert_eq!(page["outcome"], "page");
    assert_eq!(page["branch"], "feat/shelf");
    assert_eq!(
        page["page"],
        json!({
            "available": true,
            "port": 7500,
            "url": format!("http://127.0.0.1:7500/#tree={tree}&branch=feat%2Fshelf"),
        })
    );
}

#[test]
fn the_page_command_says_what_the_page_shows_and_suggests_opening_the_address() {
    let yard = Yard::new();
    let output = served(&yard, || words(&yard));
    let printed = String::from_utf8(output.stdout).unwrap();
    assert!(output.status.success());
    assert!(
        printed.contains(
            "The Yard page for feat/shelf shows its Yard Log in full: http://127.0.0.1:7500/#tree="
        ),
        "{printed}"
    );
    assert!(printed.contains("Open it in your browser."), "{printed}");
}

#[test]
fn the_page_command_writes_no_file_of_the_tree() {
    let yard = Yard::new();
    let before = files(&yard.scratch.0);
    served(&yard, || yard.run(&["page"]));
    assert_eq!(files(&yard.scratch.0), before);
}

#[test]
fn where_no_page_server_serves_the_page_the_command_says_so_names_loft_yard_serve_and_exits_1() {
    let yard = Yard::new();
    let output = yard.run(&["page"]);
    assert_eq!(output.status.code(), Some(1));
    let page = document("page", &output);
    assert_eq!(page["outcome"], "unavailable");
    assert_eq!(page["page"], json!({ "available": false, "port": 7411 }));
    let output = words(&yard);
    let printed = String::from_utf8(output.stdout).unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(
        printed.contains("No page server serves the Yard page"),
        "{printed}"
    );
    assert!(printed.contains("`loft yard serve`"), "{printed}");
    assert!(!printed.contains("http"), "{printed}");
}

#[cfg(unix)]
fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[cfg(unix)]
#[test]
fn where_the_page_server_already_serves_the_tree_a_run_that_may_not_write_the_home_gives_the_address()
 {
    let yard = Yard::new();
    let home = yard.scratch.home().join(".yard");
    name_server(&yard, 7500);
    let tree = yard.scratch.0.canonicalize().unwrap();
    let trees = serde_json::json!({ "trees": [tree.display().to_string()] });
    fs::write(home.join("trees.json"), format!("{trees:#}\n")).unwrap();
    let held = lock::lock(&home.join("server.lock")).unwrap();
    for file in ["server.json", "server.lock", "trees.json"] {
        set_mode(&home.join(file), 0o444);
    }
    set_mode(&home, 0o555);
    let output = yard.run(&["page"]);
    set_mode(&home, 0o755);
    for file in ["server.json", "server.lock", "trees.json"] {
        set_mode(&home.join(file), 0o644);
    }
    drop(held);
    let page = document("page", &output);
    assert_eq!(page["outcome"], "page", "{page}");
    assert_eq!(page["page"]["port"], 7500);
}
