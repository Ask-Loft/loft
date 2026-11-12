//! `LOFT_YARD_HOME`: what belongs to the machine kept in the directory it names in place of
//! `~/.yard/`, so that a page server kept there runs beside one kept elsewhere; an empty or
//! relative value is ignored.

use std::fs;
use std::path::PathBuf;

use crate::common::stderr;
use crate::served::{Served, free_port};
use crate::yard::Yard;

/// A directory outside `~/.yard/` for `yard`'s runs to keep the machine's files in.
fn moved(yard: &Yard) -> PathBuf {
    yard.scratch.home().join("elsewhere/yard-dev")
}

/// A page server started with `LOFT_YARD_HOME` set to `home`.
fn serve_under(yard: &Yard, home: &str) -> Served {
    Served::start_with(yard, free_port(), &[("LOFT_YARD_HOME", home)], &[])
}

#[test]
fn a_moved_home_holds_the_files_of_the_machine_and_nothing_is_written_to_the_default_one() {
    let yard = Yard::new();
    let home = moved(&yard);
    let under = [("LOFT_YARD_HOME", home.to_str().unwrap())];
    let _served = serve_under(&yard, under[0].1);
    let batch = yard.scratch.home().join("batch.json");
    fs::write(
        &batch,
        r#"[{"id":"build","kind":"stage","title":"Build","status":"active"}]"#,
    )
    .unwrap();
    let wrote = yard.run_under(
        &[
            "write",
            "--session",
            "s1",
            "--input",
            batch.to_str().unwrap(),
        ],
        &under,
    );
    assert_eq!(wrote.status.code(), Some(0), "{}", stderr(&wrote));
    let read = yard.run_under(&["read", "--session", "s1", "--mark-read"], &under);
    assert_eq!(read.status.code(), Some(0), "{}", stderr(&read));
    for name in [
        "server.json",
        "server.lock",
        "token",
        "trees.json",
        "last-read.json",
        "session-trees.json",
    ] {
        assert!(home.join(name).exists(), "{name}");
    }
    assert!(!yard.scratch.home().join(".yard").exists());
}

#[test]
fn page_servers_kept_in_two_homes_serve_beside_each_other() {
    let yard = Yard::new();
    let first = Served::start(&yard, &[]);
    let home = moved(&yard);
    let second = serve_under(&yard, home.to_str().unwrap());
    assert_eq!(first.get("/api/version").status, 200);
    assert_eq!(second.get("/api/version").status, 200);
    assert!(!yard.scratch.home().join(".yard/stop.json").exists());
    assert!(!home.join("stop.json").exists());
}

#[test]
fn an_empty_or_relative_yard_home_is_ignored_and_the_default_one_is_used() {
    for value in ["", "relative/yard"] {
        let yard = Yard::new();
        let _served = serve_under(&yard, value);
        assert!(
            yard.scratch.home().join(".yard/server.json").exists(),
            "{value:?}"
        );
        assert!(!yard.scratch.0.join("relative").exists(), "{value:?}");
    }
}
