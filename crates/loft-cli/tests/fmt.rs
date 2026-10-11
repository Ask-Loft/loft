//! `loft fmt` as a process (D-11 F-39, F-44, F-47): exit codes, the paths it prints, and what it
//! writes.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;

const READING_LIST: &str =
    include_str!("../../../testdata/v1/reading-list/loft-authored-state.json");

/// A directory removed when the test ends.
struct Scratch(PathBuf);

impl Scratch {
    /// A fresh directory holding the Reading List's `spec/` tree.
    fn with_reading_list() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("loft-cli-fmt-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        let document = convert(&Export::from_json(READING_LIST).unwrap())
            .unwrap()
            .document;
        for (path, bytes) in codec::write(&document) {
            let target = root.join("spec").join(path.as_str());
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        Self(root)
    }

    fn file(&self, path: &str) -> PathBuf {
        self.0.join("spec").join(path)
    }

    fn edit(&self, path: &str, from: &str, to: &str) {
        let text = fs::read_to_string(self.file(path)).unwrap();
        assert!(text.contains(from), "{from:?} in {path}");
        fs::write(self.file(path), text.replacen(from, to, 1)).unwrap();
    }

    fn loft(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_loft"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}

/// The first file under `spec/decisions/`, as its path under `spec/`.
fn a_decision(root: &Path) -> String {
    let mut names: Vec<_> = fs::read_dir(root.join("spec/decisions"))
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    names.sort();
    format!("decisions/{}", names[0])
}

#[test]
fn ac_544_a_canonical_tree_checks_clean_and_formats_to_nothing() {
    let scratch = Scratch::with_reading_list();
    let check = scratch.loft(&["fmt", "--check"]);
    assert_eq!(check.status.code(), Some(0), "{}", stderr(&check));
    assert_eq!(stdout(&check), "");
    let write = scratch.loft(&["fmt"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    assert_eq!(stdout(&write), "");
}

#[test]
fn ac_542_check_names_a_drifting_file_and_writes_nothing() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "title: \"", "title:   \"");
    let before = fs::read(scratch.file(&path)).unwrap();
    let check = scratch.loft(&["fmt", "--check"]);
    assert_eq!(check.status.code(), Some(1));
    assert_eq!(stdout(&check), format!("spec/{path}\n"));
    assert_eq!(fs::read(scratch.file(&path)).unwrap(), before);
}

#[test]
fn ac_534_fmt_rewrites_a_drifting_file_and_then_checks_clean() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "title: \"", "title:   \"");
    let write = scratch.loft(&["fmt"]);
    assert_eq!(write.status.code(), Some(0), "{}", stderr(&write));
    assert_eq!(stdout(&write), format!("spec/{path}\n"));
    assert_eq!(scratch.loft(&["fmt", "--check"]).status.code(), Some(0));
    let leftovers = fs::read_dir(&scratch.0).unwrap().count();
    assert_eq!(leftovers, 1, "only spec/ remains beside the tree");
}

#[test]
fn ac_539_ac_540_a_comment_is_an_error_at_its_position_and_nothing_is_written() {
    let scratch = Scratch::with_reading_list();
    let path = a_decision(&scratch.0);
    scratch.edit(&path, "\nslug:", " # a note\nslug:");
    let before = fs::read(scratch.file(&path)).unwrap();
    let output = scratch.loft(&["fmt"]);
    assert_eq!(output.status.code(), Some(1));
    let message = stderr(&output);
    assert!(message.starts_with(&format!("spec/{path}:1:")), "{message}");
    assert_eq!(fs::read(scratch.file(&path)).unwrap(), before);
}

#[test]
fn a_directory_without_a_tree_is_an_error() {
    let scratch = Scratch::with_reading_list();
    fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    let output = scratch.loft(&["fmt", "--check"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("there is no `spec/` directory"));
}

#[test]
fn a_usage_error_exits_2() {
    let scratch = Scratch::with_reading_list();
    assert_eq!(scratch.loft(&["fmt", "--chekc"]).status.code(), Some(2));
    assert_eq!(scratch.loft(&["frobnicate"]).status.code(), Some(2));
}
