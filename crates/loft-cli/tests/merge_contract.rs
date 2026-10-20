//! The six parallel-work scenarios (F-35) under a real `git merge`, with `loft merge-driver`
//! registered for `spec/`. git calls the driver and Loft never calls git's merge machinery, so
//! no simulator stands in for it: this suite holds the driver to git's contract directly.

use std::collections::BTreeSet;
use std::fs;
use std::process::Output;

use crate::common::{Scratch, stderr, stdout};
use crate::repo::Repo;

const SHARE: &str = "products/reading-list/sharing/share-a-list.yaml";
const ADD: &str = "products/reading-list/shelf/add-a-book.yaml";
const REMOVE: &str = "products/reading-list/shelf/remove-a-book.yaml";

const PUBLISH_LINK: &str = "shall: \"publish a read-only link.\"";

/// The second Requirement of "Share a list", which ends the file.
const REFUSE_EDITS: &str = "  - key: 00000000-0000-4000-8000-000000000035
    statement:
      form: state_driven
      while: \"a list is shared\"
      system: \"the shelf\"
      shall: \"refuse edits made through the link.\"
    tags:
      - portable
    criteria:
      - key: 00000000-0000-4000-8000-000000000036
        name: \"An edit made through the link is refused.\"
";

/// Commits `theirs` on a branch from base and `ours` on `main`, then merges the branch.
fn merge(repo: &Repo, ours: impl FnOnce(&Scratch), theirs: impl FnOnce(&Scratch)) -> Output {
    repo.git_ok(&["checkout", "--quiet", "-b", "theirs"]);
    theirs(&repo.0);
    repo.commit("theirs");
    repo.git_ok(&["checkout", "--quiet", "main"]);
    ours(&repo.0);
    repo.commit("ours");
    repo.git(&["merge", "--no-edit", "theirs"])
}

fn append(scratch: &Scratch, path: &str, text: &str) {
    let mut current = fs::read_to_string(scratch.file(path)).unwrap();
    current.push_str(text);
    fs::write(scratch.file(path), current).unwrap();
}

/// A Requirement of "Share a list" with one criterion, keyed when `keys` are given.
fn requirement(keys: Option<(&str, &str)>, shall: &str) -> String {
    let (key, criterion) = keys.map_or((String::new(), String::new()), |(key, criterion)| {
        (
            format!("key: {key}\n    "),
            format!("key: {criterion}\n        "),
        )
    });
    format!(
        "  - {key}statement:
      form: ubiquitous
      system: \"The shelf\"
      shall: \"{shall}\"
    criteria:
      - {criterion}name: \"The shelf does so.\"
"
    )
}

/// What a command printed, standard output then standard error.
fn printed(output: &Output) -> String {
    stdout(output) + &stderr(output)
}

/// The keys a file holds.
fn keys(text: &str) -> BTreeSet<String> {
    text.lines()
        .filter_map(|line| line.trim_start().strip_prefix("- key: "))
        .map(str::to_owned)
        .collect()
}

#[test]
fn edits_to_different_features_merge_cleanly_and_validate() {
    let repo = Repo::new();
    let merged = merge(
        &repo,
        |ours| ours.edit(SHARE, PUBLISH_LINK, "shall: \"publish a link.\""),
        |theirs| theirs.edit(ADD, "record its title and author.", "record its title."),
    );
    assert_eq!(merged.status.code(), Some(0), "{}", stdout(&merged));
    assert!(repo.text(SHARE).contains("shall: \"publish a link.\""));
    assert!(repo.text(ADD).contains("record its title."));
    assert_eq!(repo.0.loft(&["check"]).status.code(), Some(0));
}

#[test]
fn both_appends_to_one_feature_follow_its_requirements_ours_first() {
    let repo = Repo::new();
    let ours_key = "01a13099-7453-76e1-9b96-921b114802b8";
    let theirs_key = "01a13099-7453-76e1-9b96-921d18d1d697";
    let merged = merge(
        &repo,
        |ours| {
            let keys = Some((ours_key, "01a13099-7453-76e1-9b96-921c5ab97266"));
            append(ours, SHARE, &requirement(keys, "list the shared links."));
        },
        |theirs| {
            let keys = Some((theirs_key, "01a13099-7453-76e1-9b96-921e1c7d41a7"));
            append(theirs, SHARE, &requirement(keys, "expire a shared link."));
        },
    );
    assert_eq!(merged.status.code(), Some(0), "{}", stdout(&merged));
    let text = repo.text(SHARE);
    let at = |key: &str| text.find(&format!("- key: {key}")).unwrap();
    assert!(
        at("00000000-0000-4000-8000-000000000035") < at(ours_key),
        "{text}"
    );
    assert!(at(ours_key) < at(theirs_key), "{text}");
    assert_eq!(repo.0.loft(&["check"]).status.code(), Some(0));
}

#[test]
fn a_requirement_edited_on_one_side_and_moved_on_the_other_is_both() {
    let repo = Repo::new();
    let merged = merge(
        &repo,
        |ours| {
            ours.edit(SHARE, REFUSE_EDITS, "");
            ours.edit(
                SHARE,
                "requirements:\n",
                &format!("requirements:\n{REFUSE_EDITS}"),
            );
        },
        |theirs| theirs.edit(SHARE, "refuse edits made", "refuse every edit made"),
    );
    assert_eq!(merged.status.code(), Some(0), "{}", stdout(&merged));
    let text = repo.text(SHARE);
    let at = |needle: &str| text.find(needle).unwrap();
    assert!(at("000000000035") < at("000000000033"), "{text}");
    assert!(text.contains("refuse every edit made"), "{text}");
}

#[test]
fn a_deleted_node_another_side_references_is_absent_and_check_names_it() {
    let repo = Repo::new();
    let deleted = "00000000-0000-4000-8000-000000000030";
    let merged = merge(
        &repo,
        |ours| {
            let text = fs::read_to_string(ours.file(REMOVE)).unwrap();
            let start = text.find(&format!("  - key: {deleted}")).unwrap();
            fs::write(ours.file(REMOVE), &text[..start]).unwrap();
        },
        |theirs| {
            let name = "changes nothing.\"\n";
            let mentions = format!("{name}        mentions:\n          - {deleted}\n");
            theirs.edit(REMOVE, name, &mentions);
        },
    );
    assert_eq!(merged.status.code(), Some(0), "{}", stdout(&merged));
    let text = repo.text(REMOVE);
    assert!(!text.contains(&format!("key: {deleted}")), "{text}");
    assert!(text.contains(&format!("          - {deleted}\n")), "{text}");
    let check = repo.0.loft(&["check"]);
    assert_eq!(check.status.code(), Some(1));
    let report = printed(&check);
    assert!(report.contains(deleted), "{report}");
}

#[test]
fn one_statement_edited_apart_conflicts_until_the_record_is_removed() {
    let repo = Repo::new();
    let merged = merge(
        &repo,
        |ours| ours.edit(SHARE, PUBLISH_LINK, "shall: \"publish a link.\""),
        |theirs| theirs.edit(SHARE, PUBLISH_LINK, "shall: \"publish a private link.\""),
    );
    assert_eq!(merged.status.code(), Some(1));
    assert!(
        stdout(&merged).contains(&format!(
            "CONFLICT (content): Merge conflict in spec/{SHARE}"
        )),
        "{}",
        stdout(&merged)
    );
    let unmerged = repo.git_ok(&["ls-files", "--unmerged"]);
    assert!(stdout(&unmerged).contains(&format!("spec/{SHARE}")));
    let conflict = "    conflicts:
      - field: statement.shall
        base: \"publish a read-only link.\"
        ours: \"publish a link.\"
        theirs: \"publish a private link.\"
";
    assert!(repo.text(SHARE).contains(conflict), "{}", repo.text(SHARE));
    let check = repo.0.loft(&["check"]);
    assert_eq!(check.status.code(), Some(1));
    let report = printed(&check);
    assert!(
        report.contains("unresolved merge conflict ") && report.contains(" over `statement.shall`"),
        "{report}"
    );
    repo.0.edit(SHARE, conflict, "");
    assert_eq!(repo.0.loft(&["check"]).status.code(), Some(0));
    repo.commit("resolved");
}

#[test]
fn nodes_both_sides_create_are_both_kept_with_their_own_keys_unlabelled() {
    let repo = Repo::new();
    let base = keys(&repo.text(SHARE));
    let create = |scratch: &Scratch, shall: &str| {
        append(scratch, SHARE, &requirement(None, shall));
        let minted = scratch.loft(&["fmt"]);
        assert_eq!(minted.status.code(), Some(0), "{}", stderr(&minted));
    };
    let merged = merge(
        &repo,
        |ours| create(ours, "list the shared links."),
        |theirs| create(theirs, "expire a shared link."),
    );
    assert_eq!(merged.status.code(), Some(0), "{}", stdout(&merged));
    let created: BTreeSet<String> = keys(&repo.text(SHARE)).difference(&base).cloned().collect();
    assert_eq!(
        created.len(),
        4,
        "two Requirements and their criteria: {created:?}"
    );
    let labels = fs::read_to_string(repo.0.file("labels.yaml")).unwrap();
    assert!(created.iter().all(|key| !labels.contains(key.as_str())));
    assert_eq!(repo.0.loft(&["check"]).status.code(), Some(0));
}

#[test]
fn a_file_both_sides_add_is_merged_over_an_empty_base() {
    let repo = Repo::new();
    let path = "products/reading-list/shelf/lend-a-book.yaml";
    let feature = |name: &str| {
        format!("key: 01a13099-7453-76e1-9b96-921f20fbd080\nslug: lend-a-book\nname: \"{name}\"\n")
    };
    let merged = merge(
        &repo,
        |ours| fs::write(ours.file(path), feature("Lend a book")).unwrap(),
        |theirs| fs::write(theirs.file(path), feature("Lend a book to a friend")).unwrap(),
    );
    assert_eq!(merged.status.code(), Some(1), "{}", stdout(&merged));
    assert_eq!(
        repo.text(path),
        format!(
            "{}conflicts:\n  - field: name\n    ours: \"Lend a book\"\n    theirs: \"Lend a book to a friend\"\n",
            feature("Lend a book")
        )
    );
}

#[test]
fn a_driver_whose_launcher_cannot_run_leaves_conflict_markers_and_names_the_way_out() {
    let repo = Repo::new();
    let missing = format!(
        "merge.loft.driver={}",
        loftspec_store::merge_driver::driver("no-such-loft")
    );
    // Each side rewords a different Requirement of one file, which git's text merge joins
    // without a conflict; the driver still reports one, so the file stays unmerged.
    repo.git_ok(&["checkout", "--quiet", "-b", "theirs"]);
    repo.0
        .edit(SHARE, PUBLISH_LINK, "shall: \"publish a link.\"");
    repo.commit("theirs");
    repo.git_ok(&["checkout", "--quiet", "main"]);
    repo.0.edit(
        SHARE,
        "refuse edits made through the link.",
        "refuse every edit.",
    );
    repo.commit("ours");
    let merged = repo.git(&["-c", &missing, "merge", "--no-edit", "theirs"]);
    assert!(!merged.status.success(), "{}", printed(&merged));
    assert!(
        printed(&merged)
            .contains("git checkout -m 'spec/products/reading-list/sharing/share-a-list.yaml'"),
        "{}",
        printed(&merged)
    );
    let text = repo.text(SHARE);
    assert!(
        text.contains("publish a link.") && text.contains("refuse every edit."),
        "{text}"
    );
    let status = repo.git_ok(&["status", "--porcelain"]);
    assert!(stdout(&status).contains("UU spec/products/reading-list/sharing/share-a-list.yaml"));
}

#[test]
fn a_driver_whose_launcher_cannot_run_writes_markers_where_the_sides_meet() {
    let repo = Repo::new();
    let missing = format!(
        "merge.loft.driver={}",
        loftspec_store::merge_driver::driver("no-such-loft")
    );
    let merged = merge(
        &repo,
        |ours| ours.edit(SHARE, PUBLISH_LINK, "shall: \"publish a private link.\""),
        |theirs| theirs.edit(SHARE, PUBLISH_LINK, "shall: \"publish a public link.\""),
    );
    assert!(!merged.status.success());
    // The merge above ran the working driver; abort it and run the same merge without one.
    repo.git_ok(&["merge", "--abort"]);
    let again = repo.git(&["-c", &missing, "merge", "--no-edit", "theirs"]);
    assert!(!again.status.success());
    let text = repo.text(SHARE);
    assert!(
        text.contains("<<<<<<< ours") && text.contains(">>>>>>> theirs"),
        "{text}"
    );
}
