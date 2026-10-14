//! Pseudonymous tokens: an installation token for the user on the machine, a project token for
//! one checkout, and a branch token keyed with a secret that never leaves the checkout.

use std::fs;

use crate::common::telemetry::{Sent, sent, the_event};
use crate::common::{Scratch, stderr};
use serde_json::{Value, json};
use software_plant::GitScenario;

fn check(scratch: &Scratch, git: Option<&GitScenario>) -> Value {
    the_event(&sent(scratch, &["check"], git)).clone()
}

#[test]
fn the_installation_token_is_kept_and_carried_by_every_later_event() {
    let scratch = Scratch::with_reading_list();
    let first = check(&scratch, None)["installation"].clone();
    let second = check(&scratch, None)["installation"].clone();
    assert_eq!(first, second);
    let kept: Value =
        serde_json::from_slice(&fs::read(scratch.config().join("telemetry.json")).unwrap())
            .unwrap();
    assert_eq!(kept["installation"], first);
}

#[test]
fn the_project_token_is_kept_beside_spec_where_git_never_sees_it() {
    let scratch = Scratch::with_reading_list();
    let first = check(&scratch, None)["project"].clone();
    assert_eq!(check(&scratch, None)["project"], first);
    let kept: Value =
        serde_json::from_slice(&fs::read(scratch.0.join(".loft/telemetry.json")).unwrap()).unwrap();
    assert_eq!(kept["project"], first);
    assert_eq!(
        fs::read_to_string(scratch.0.join(".loft/.gitignore")).unwrap(),
        "*\n"
    );
}

#[test]
fn two_checkouts_carry_different_tokens_for_the_same_branch() {
    let (one, two) = (Scratch::with_reading_list(), Scratch::with_reading_list());
    let git = GitScenario::clean_on("main");
    let (first, second) = (check(&one, Some(&git)), check(&two, Some(&git)));
    assert_ne!(first["project"], second["project"]);
    assert_ne!(first["branch"], second["branch"]);
}

#[test]
fn a_deleted_local_directory_gives_a_new_project_token() {
    let scratch = Scratch::with_reading_list();
    let first = check(&scratch, None)["project"].clone();
    fs::remove_dir_all(scratch.0.join(".loft")).unwrap();
    assert_ne!(check(&scratch, None)["project"], first);
}

#[test]
fn a_directory_without_spec_carries_no_project_and_no_branch() {
    let scratch = Scratch::empty();
    let git = GitScenario::clean_on("main");
    let event = check(&scratch, Some(&git));
    assert!(event.get("project").is_none(), "{event}");
    assert!(event.get("branch").is_none(), "{event}");
    assert!(!scratch.0.join(".loft").exists());
}

#[test]
fn one_branch_keeps_its_token_and_two_branches_differ() {
    let scratch = Scratch::with_reading_list();
    let main = check(&scratch, Some(&GitScenario::clean_on("main")))["branch"].clone();
    let branch = main.as_str().unwrap();
    assert!(
        branch.len() == 16 && branch.bytes().all(|b| b.is_ascii_hexdigit()),
        "{branch}"
    );
    assert_eq!(
        check(&scratch, Some(&GitScenario::clean_on("main")))["branch"],
        main
    );
    assert_ne!(
        check(&scratch, Some(&GitScenario::clean_on("develop")))["branch"],
        main
    );
}

#[test]
fn neither_the_branch_name_nor_its_secret_appears_in_the_event() {
    let scratch = Scratch::with_reading_list();
    let event = check(&scratch, Some(&GitScenario::clean_on("main"))).to_string();
    let kept: Value =
        serde_json::from_slice(&fs::read(scratch.0.join(".loft/telemetry.json")).unwrap()).unwrap();
    let secret = kept["branchSecret"].as_str().unwrap();
    assert!(!event.contains(secret), "{event}");
    assert!(!event.contains("\"main\""), "{event}");
}

#[test]
fn no_branch_is_sent_on_a_detached_head_outside_a_repository_or_without_git() {
    for git in [
        Some(GitScenario::clean_on("main").detached()),
        Some(GitScenario::outside_repository()),
        None,
    ] {
        let scratch = Scratch::with_reading_list();
        let sent: Sent = sent(&scratch, &["check"], git.as_ref());
        let event = the_event(&sent);
        assert!(event.get("branch").is_none(), "{event}");
        assert!(event.get("project").is_some(), "{event}");
        assert_eq!(stderr(&sent.output), "", "{git:?}");
    }
}

#[test]
fn a_local_directory_that_cannot_be_written_leaves_out_project_and_branch() {
    let scratch = Scratch::with_reading_list();
    fs::write(scratch.0.join(".loft"), "").unwrap();
    let sent = sent(&scratch, &["check"], Some(&GitScenario::clean_on("main")));
    assert_eq!(sent.output.status.code(), Some(0));
    let event = the_event(&sent);
    assert!(event.get("project").is_none(), "{event}");
    assert!(event.get("branch").is_none(), "{event}");
    assert_eq!(json!(event["installation"].is_string()), json!(true));
}
