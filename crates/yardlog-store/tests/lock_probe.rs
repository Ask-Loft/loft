//! Probing a lock only reads it: a lock file the prober may not write, as in a sandbox, still
//! tells whether another holds it, and a lock file that is missing is held by no one.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::Path;

use yardlog_store::lock::{self, is_held};

use crate::common::Scratch;

fn set_mode(path: &Path, mode: u32) {
    fs::set_permissions(path, fs::Permissions::from_mode(mode)).unwrap();
}

#[test]
fn a_lock_file_the_prober_may_not_write_still_reads_as_held_while_held_and_free_once_let_go() {
    let scratch = Scratch::new();
    let dir = scratch.0.join("home");
    fs::create_dir_all(&dir).unwrap();
    let path = dir.join("server.lock");
    let held = lock::lock(&path).unwrap();
    set_mode(&path, 0o444);
    set_mode(&dir, 0o555);
    let while_held = is_held(&path);
    drop(held);
    let once_let_go = is_held(&path);
    set_mode(&dir, 0o755);
    set_mode(&path, 0o644);
    assert!(while_held.unwrap());
    assert!(!once_let_go.unwrap());
}

#[test]
fn a_missing_lock_file_is_held_by_no_one_and_probing_it_creates_nothing() {
    let scratch = Scratch::new();
    let path = scratch.0.join("home/server.lock");
    assert!(!is_held(&path).unwrap());
    assert!(!path.exists());
    assert!(!path.parent().unwrap().exists());
}
