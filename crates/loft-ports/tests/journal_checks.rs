//! What a journal may name: a plan is a file on disk, which anything may have written, so a
//! journal is checked whole before a file moves. A path that is not plain, that lies outside
//! what the journal saves, in git's own directory or behind a link that leads out of the root,
//! and a staged file that is not the journal's own, are refused, and nothing changes.

use std::fs;

use crate::common::Scratch;
use loft_ports::PortError;
use loft_ports::journal::{Changes, Journal, RelPath};
use serde_json::json;

/// A root with a tree under `spec/`, a file outside the root, and a journal in `.loft/save`
/// that saves under `spec/` and the lock beside it.
fn layout() -> (Scratch, Journal) {
    let scratch = Scratch::new();
    scratch.put("root/spec/a.yaml", "a 1\n");
    scratch.put("root/loft.lock.json", "lock 1\n");
    scratch.put("root/README.md", "readme\n");
    scratch.put("root/.git/config", "config\n");
    scratch.put("outside/kept.txt", "kept\n");
    let root = scratch.0.join("root");
    let journal = Journal::new(&root, root.join(".loft/save"))
        .keeping("spec")
        .beside(&["loft.lock.json"]);
    (scratch, journal)
}

/// Puts a committed plan in the journal that writes each of `write` from a staged file named
/// by its place, and removes each of `remove`.
fn planned(scratch: &Scratch, write: &[&str], remove: &[&str]) {
    let entries: Vec<_> = (write.iter().enumerate())
        .map(|(index, path)| {
            scratch.put(&format!("root/.loft/save/files/{index}"), "staged\n");
            json!({ "path": path, "staged": index.to_string() })
        })
        .collect();
    let plan = json!({ "format": "loftspec-save/1", "write": entries, "remove": remove });
    scratch.put("root/.loft/save/plan.json", &plan.to_string());
}

/// Asserts that recovery refuses the journal, naming its directory and `named`, and that every
/// file is as it was, the journal's own included.
fn refused(scratch: &Scratch, journal: &Journal, named: &str) {
    let before = scratch.snapshot();
    let error = journal.recover().unwrap_err();
    let PortError::Journal(dir, message) = &error else {
        panic!("{error}");
    };
    assert_eq!(dir, journal.dir());
    assert!(message.contains(named), "{message}");
    assert_eq!(scratch.snapshot(), before);
}

#[test]
fn a_plan_of_files_the_journal_saves_is_finished() {
    let (scratch, journal) = layout();
    planned(
        &scratch,
        &["spec/a.yaml", "spec/products/b.yaml", "loft.lock.json"],
        &[],
    );
    journal.recover().unwrap();
    let files = scratch.snapshot();
    for path in [
        "root/spec/a.yaml",
        "root/spec/products/b.yaml",
        "root/loft.lock.json",
    ] {
        assert_eq!(files[path], b"staged\n", "{path}");
    }
    assert!(!files.contains_key("root/.loft/"));
}

#[test]
fn a_plan_that_writes_above_the_root_is_refused_and_changes_nothing() {
    let (scratch, journal) = layout();
    planned(&scratch, &["spec/../../outside/kept.txt"], &[]);
    refused(&scratch, &journal, "spec/../../outside/kept.txt");
}

#[test]
fn a_plan_that_removes_above_the_root_is_refused_and_changes_nothing() {
    let (scratch, journal) = layout();
    planned(&scratch, &[], &["../outside/kept.txt"]);
    refused(&scratch, &journal, "../outside/kept.txt");
}

#[test]
fn a_plan_with_an_absolute_path_is_refused() {
    let (scratch, journal) = layout();
    let absolute = scratch.0.join("outside/kept.txt");
    let absolute = absolute.to_str().unwrap().replace('\\', "/");
    planned(&scratch, &[&absolute], &[]);
    refused(&scratch, &journal, &absolute);
}

#[test]
fn a_plan_with_an_empty_part_or_a_dot_part_is_refused() {
    for path in ["spec//a.yaml", "spec/./a.yaml", "spec/", ""] {
        let (scratch, journal) = layout();
        planned(&scratch, &[path], &[]);
        refused(&scratch, &journal, &format!("`{path}`"));
    }
}

#[test]
fn a_plan_that_names_a_file_the_journal_does_not_save_is_refused() {
    for path in [
        "README.md",
        "specs/a.yaml",
        ".loft/other",
        "loft.lock.json/x",
    ] {
        let (scratch, journal) = layout();
        planned(&scratch, &[path], &[]);
        refused(&scratch, &journal, path);
        let (scratch, journal) = layout();
        planned(&scratch, &[], &[path]);
        refused(&scratch, &journal, path);
    }
}

#[test]
fn a_plan_that_names_a_part_called_git_s_directory_in_any_case_is_refused() {
    for path in ["spec/.git/config", "spec/.GIT/hooks/pre-commit"] {
        let (scratch, journal) = layout();
        planned(&scratch, &[path], &[]);
        refused(&scratch, &journal, path);
    }
}

#[test]
fn a_journal_with_no_floor_still_refuses_git_s_directory() {
    let scratch = Scratch::new();
    scratch.put("root/.git/config", "config\n");
    scratch.put("elsewhere/files/0", "staged\n");
    scratch.put(
        "elsewhere/plan.json",
        r#"{"format":"loftspec-save/1","write":[{"path":".git/config","staged":"0"}],"remove":[]}"#,
    );
    let journal = Journal::new(scratch.0.join("root"), scratch.0.join("elsewhere"));
    refused(&scratch, &journal, ".git/config");
}

#[test]
fn a_plan_is_checked_whole_before_its_first_file_is_written() {
    let (scratch, journal) = layout();
    planned(&scratch, &["spec/a.yaml", "../outside/kept.txt"], &[]);
    refused(&scratch, &journal, "../outside/kept.txt");
    assert_eq!(scratch.snapshot()["root/spec/a.yaml"], b"a 1\n");
}

#[test]
fn a_staged_file_not_named_by_a_number_is_refused() {
    for staged in ["../../../outside/kept.txt", "0/../1", "", "+1", "a"] {
        let (scratch, journal) = layout();
        scratch.put(
            "root/.loft/save/plan.json",
            &format!(
                r#"{{"format":"loftspec-save/1","write":[{{"path":"spec/a.yaml","staged":{staged:?}}}],"remove":[]}}"#
            ),
        );
        refused(&scratch, &journal, &format!("`{staged}`"));
    }
}

#[cfg(unix)]
#[test]
fn a_staged_file_that_is_a_link_is_refused() {
    let (scratch, journal) = layout();
    planned(&scratch, &["spec/a.yaml"], &[]);
    let staged = scratch.0.join("root/.loft/save/files/0");
    fs::remove_file(&staged).unwrap();
    std::os::unix::fs::symlink(scratch.0.join("outside/kept.txt"), &staged).unwrap();
    let error = journal.recover().unwrap_err();
    assert!(matches!(&error, PortError::Journal(..)), "{error}");
    assert_eq!(
        fs::read(scratch.0.join("root/spec/a.yaml")).unwrap(),
        b"a 1\n"
    );
    assert!(fs::symlink_metadata(&staged).unwrap().is_symlink());
}

#[cfg(unix)]
#[test]
fn a_path_behind_a_link_that_leads_out_of_the_root_is_refused() {
    let (scratch, journal) = layout();
    std::os::unix::fs::symlink(
        scratch.0.join("outside"),
        scratch.0.join("root/spec/products"),
    )
    .unwrap();
    planned(&scratch, &["spec/products/kept.txt"], &[]);
    let error = journal.recover().unwrap_err();
    assert!(
        matches!(&error, PortError::Journal(_, message) if message.contains("spec/products/kept.txt")),
        "{error}"
    );
    assert_eq!(
        fs::read(scratch.0.join("outside/kept.txt")).unwrap(),
        b"kept\n"
    );
    planned(&scratch, &[], &["spec/products/kept.txt"]);
    journal.recover().unwrap_err();
    assert!(scratch.0.join("outside/kept.txt").exists());
}

#[cfg(unix)]
#[test]
fn a_path_behind_a_link_into_git_s_directory_is_refused() {
    let (scratch, journal) = layout();
    std::os::unix::fs::symlink(
        scratch.0.join("root/.git"),
        scratch.0.join("root/spec/products"),
    )
    .unwrap();
    planned(&scratch, &["spec/products/config"], &[]);
    journal.recover().unwrap_err();
    assert_eq!(
        fs::read(scratch.0.join("root/.git/config")).unwrap(),
        b"config\n"
    );
}

#[cfg(unix)]
#[test]
fn a_path_behind_a_link_that_stays_under_the_floor_is_saved() {
    let (scratch, journal) = layout();
    scratch.put("root/spec/shared/x.yaml", "x 1\n");
    std::os::unix::fs::symlink("shared", scratch.0.join("root/spec/products")).unwrap();
    planned(&scratch, &["spec/products/x.yaml"], &[]);
    journal.recover().unwrap();
    assert_eq!(
        fs::read(scratch.0.join("root/spec/shared/x.yaml")).unwrap(),
        b"staged\n"
    );
}

#[test]
fn a_save_of_a_path_the_journal_refuses_stages_no_file() {
    for path in ["../outside/kept.txt", "README.md", "spec/.git/config"] {
        let (scratch, journal) = layout();
        let before = scratch.snapshot();
        let mut changes = Changes::default();
        changes.write.insert(RelPath::new(path), b"new\n".to_vec());
        let error = journal.save(&changes).unwrap_err();
        assert!(
            matches!(&error, PortError::Journal(_, message) if message.contains(path)),
            "{error}"
        );
        assert_eq!(scratch.snapshot(), before, "{path}");
        let mut changes = Changes::default();
        changes.remove.insert(RelPath::new(path));
        journal.save(&changes).unwrap_err();
        assert_eq!(scratch.snapshot(), before, "{path}");
    }
}

#[cfg(unix)]
#[test]
fn a_path_under_the_floor_behind_a_link_that_leaves_the_floor_is_refused() {
    // The link stays under the root, where a file the repository does not track may lie.
    for to in ["../local", "../.loft"] {
        let link = "root/spec/up";
        let (scratch, journal) = layout();
        scratch.put("root/local/.env", "secret\n");
        scratch.put("root/.loft/.env", "secret\n");
        std::os::unix::fs::symlink(to, scratch.0.join(link)).unwrap();
        planned(&scratch, &["spec/up/.env"], &[]);
        refused(&scratch, &journal, "spec/up/.env");
        planned(&scratch, &[], &["spec/up/.env"]);
        refused(&scratch, &journal, "spec/up/.env");
    }
}

#[cfg(unix)]
#[test]
fn a_floor_that_is_a_link_out_of_the_root_is_refused() {
    let scratch = Scratch::new();
    scratch.put("outside/a.yaml", "a 1\n");
    fs::create_dir_all(scratch.0.join("root")).unwrap();
    std::os::unix::fs::symlink("../outside", scratch.0.join("root/spec")).unwrap();
    let root = scratch.0.join("root");
    let journal = Journal::new(&root, root.join(".loft/save")).keeping("spec");
    planned(&scratch, &["spec/a.yaml"], &[]);
    refused(&scratch, &journal, "spec/a.yaml");
}

#[cfg(unix)]
#[test]
fn a_path_beside_the_floor_behind_a_link_that_leaves_the_root_is_refused() {
    let (scratch, journal) = layout();
    let journal = journal.beside(&["shared/one.yaml"]);
    std::os::unix::fs::symlink("../outside", scratch.0.join("root/shared")).unwrap();
    planned(&scratch, &["shared/one.yaml"], &[]);
    refused(&scratch, &journal, "shared/one.yaml");
}

#[test]
fn a_part_that_a_file_system_may_read_as_git_s_directory_is_refused() {
    for path in [
        "spec/.git./config",
        "spec/.git /config",
        "spec/.GIT. ./config",
        "spec/git~1/config",
        "spec/GIT~1/config",
        "spec/.g\u{200c}it/config",
        "spec/.git\u{feff}",
    ] {
        let (scratch, journal) = layout();
        planned(&scratch, &[path], &[]);
        refused(&scratch, &journal, "git's own directory");
    }
}

#[test]
fn a_part_that_holds_a_colon_or_a_backslash_is_refused() {
    for path in ["spec/a.yaml:stream", "spec/a\\b.yaml", "spec/c:/a.yaml"] {
        let (scratch, journal) = layout();
        planned(&scratch, &[path], &[]);
        refused(&scratch, &journal, "not a plain path");
    }
}

#[test]
fn a_staged_file_that_is_a_directory_is_refused() {
    let (scratch, journal) = layout();
    planned(&scratch, &["spec/a.yaml"], &[]);
    let staged = scratch.0.join("root/.loft/save/files/0");
    fs::remove_file(&staged).unwrap();
    fs::create_dir(&staged).unwrap();
    refused(&scratch, &journal, "`0`");
}

/// A journal elsewhere, committed where `committed`, that a link under the root leads to.
#[cfg(unix)]
fn linked(link: &str, to: &str, committed: bool) -> (Scratch, Journal) {
    let (scratch, journal) = layout();
    scratch.put("elsewhere/save/files/0", "staged\n");
    if committed {
        scratch.put(
            "elsewhere/save/plan.json",
            r#"{"format":"loftspec-save/1","write":[{"path":"spec/a.yaml","staged":"0"}],"remove":[]}"#,
        );
    }
    if let Some(parent) = scratch.0.join(link).parent() {
        fs::create_dir_all(parent).unwrap();
    }
    std::os::unix::fs::symlink(scratch.0.join(to), scratch.0.join(link)).unwrap();
    (scratch, journal)
}

#[cfg(unix)]
#[test]
fn a_journal_whose_directory_lies_behind_a_link_is_refused_and_nothing_is_deleted() {
    let places = [
        ("root/.loft", "elsewhere"),
        ("root/.loft/save", "elsewhere/save"),
    ];
    for (link, to) in places {
        for committed in [true, false] {
            let (scratch, journal) = linked(link, to, committed);
            refused(&scratch, &journal, "behind a link");
            assert!(scratch.0.join("elsewhere/save/files/0").exists(), "{link}");
        }
    }
}

#[cfg(unix)]
#[test]
fn a_journal_whose_staged_files_lie_behind_a_link_is_refused() {
    let (scratch, journal) = layout();
    scratch.put("elsewhere/0", "not staged\n");
    scratch.put(
        "root/.loft/save/plan.json",
        r#"{"format":"loftspec-save/1","write":[{"path":"spec/a.yaml","staged":"0"}],"remove":[]}"#,
    );
    std::os::unix::fs::symlink(
        scratch.0.join("elsewhere"),
        scratch.0.join("root/.loft/save/files"),
    )
    .unwrap();
    refused(&scratch, &journal, "behind a link");
}
