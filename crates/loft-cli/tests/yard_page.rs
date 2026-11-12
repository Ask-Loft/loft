//! `loft yard page` (the Give the Yard page's address Feature): the address of the branch's Yard
//! Log on the page, with a line on what the page shows, or that no page server serves it.

#[cfg(unix)]
use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
#[cfg(unix)]
use std::path::Path;
use std::process::Output;

use serde_json::json;
use yardlog_store::lock;

use crate::snapshot::snapshot;
use crate::yard::{Yard, document, name_server};

/// Runs `loft yard page` as a person would, printing words, not JSON.
fn words(yard: &Yard) -> Output {
    yard.scratch
        .loft_with(&["yard", "page"], Some(&yard.git), &yard.clock)
}

/// Runs `serve` while a page server on 7500 is named and holds its lock.
fn served<T>(yard: &Yard, serve: impl FnOnce() -> T) -> T {
    name_server(yard, 7500);
    let _held = lock::lock(&yard.scratch.home().join(".yard/server.lock")).unwrap();
    serve()
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
    let before = snapshot(&yard.scratch.0);
    served(&yard, || yard.run(&["page"]));
    assert_eq!(snapshot(&yard.scratch.0), before);
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

/// Sets the modes of the home's files, then of the home itself.
#[cfg(unix)]
fn set_modes(home: &Path, files: u32, dir: u32) {
    let set = |path: &Path, mode| fs::set_permissions(path, fs::Permissions::from_mode(mode));
    for file in ["server.json", "server.lock", "trees.json"] {
        set(&home.join(file), files).unwrap();
    }
    set(home, dir).unwrap();
}

#[cfg(unix)]
#[test]
fn where_the_page_server_already_serves_the_tree_a_run_that_may_not_write_the_home_gives_the_address()
 {
    let yard = Yard::new();
    let home = yard.scratch.home().join(".yard");
    name_server(&yard, 7500);
    let tree = yard.scratch.0.canonicalize().unwrap();
    let trees = json!({ "trees": [tree.display().to_string()] });
    fs::write(home.join("trees.json"), format!("{trees:#}\n")).unwrap();
    let held = lock::lock(&home.join("server.lock")).unwrap();
    set_modes(&home, 0o444, 0o555);
    let refused = fs::write(home.join("probe"), "").is_err();
    let output = yard.run(&["page"]);
    set_modes(&home, 0o644, 0o755);
    drop(held);
    if !refused {
        eprintln!("skipped: the home stays writable for this user, as for root");
        return;
    }
    let page = document("page", &output);
    assert_eq!(page["outcome"], "page", "{page}");
    assert_eq!(page["page"]["port"], 7500);
}

#[cfg(unix)]
#[test]
fn where_a_running_page_server_does_not_serve_the_tree_the_command_names_its_port_and_says_so() {
    let yard = Yard::new();
    let home = yard.scratch.home().join(".yard");
    name_server(&yard, 7500);
    fs::write(home.join("trees.json"), "{\n  \"trees\": []\n}\n").unwrap();
    let held = lock::lock(&home.join("server.lock")).unwrap();
    set_modes(&home, 0o444, 0o555);
    let refused = fs::write(home.join("probe"), "").is_err();
    let output = yard.run(&["page"]);
    let printed = words(&yard);
    set_modes(&home, 0o644, 0o755);
    drop(held);
    if !refused {
        eprintln!("skipped: the home stays writable for this user, as for root");
        return;
    }
    assert_eq!(output.status.code(), Some(1));
    let page = document("page", &output);
    assert_eq!(page["outcome"], "unavailable");
    assert_eq!(page["page"], json!({ "available": false, "port": 7500 }));
    assert_eq!(printed.status.code(), Some(1));
    let printed = String::from_utf8(printed.stdout).unwrap();
    assert!(printed.contains("port 7500"), "{printed}");
    assert!(
        printed.contains("does not serve this tree yet"),
        "{printed}"
    );
    assert!(!printed.contains("http"), "{printed}");
    assert!(!printed.contains("loft yard serve"), "{printed}");
}
