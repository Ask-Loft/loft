//! Every test file is a module of its crate's one test binary. Each crate declares its
//! integration tests as one binary, `tests/main.rs`, so Cargo no longer finds a file of `tests/`
//! by itself: a file that `tests/main.rs` does not declare would never be compiled or run. A guard
//! fails on such a file.

use crate::common::workspace;
use std::fs;

/// The files directly in `tests/` that `main` does not declare as `mod <file>;`.
fn undeclared(tests: &[String], main: &str) -> Vec<String> {
    tests
        .iter()
        .filter(|stem| !main.lines().any(|line| line == format!("mod {stem};")))
        .cloned()
        .collect()
}

#[test]
fn every_test_file_is_declared_in_its_crate_s_test_binary() {
    let mut missing = Vec::new();
    let mut crates = 0;
    for entry in fs::read_dir(workspace().join("crates")).unwrap() {
        let tests = entry.unwrap().path().join("tests");
        if !tests.is_dir() {
            continue;
        }
        crates += 1;
        let main = fs::read_to_string(tests.join("main.rs")).unwrap();
        let stems: Vec<String> = fs::read_dir(&tests)
            .unwrap()
            .map(|entry| entry.unwrap().path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "rs"))
            .filter_map(|path| Some(path.file_stem()?.to_str()?.to_owned()))
            .filter(|stem| stem != "main")
            .collect();
        let shown = tests
            .strip_prefix(workspace())
            .unwrap()
            .display()
            .to_string();
        missing.extend(
            undeclared(&stems, &main)
                .into_iter()
                .map(|stem| format!("{shown}/{stem}.rs")),
        );
    }
    assert_eq!(crates, 4, "the scan found {crates} crates with tests");
    assert_eq!(missing, Vec::<String>::new());
}

#[test]
fn a_test_file_the_binary_does_not_declare_is_reported() {
    let tests = ["check".to_owned(), "fmt".to_owned()];
    assert_eq!(undeclared(&tests, "mod check;\n"), ["fmt"]);
    assert_eq!(
        undeclared(&tests, "mod check;\nmod fmt;\n"),
        Vec::<String>::new()
    );
}
