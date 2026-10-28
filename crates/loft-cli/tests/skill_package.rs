//! `loft skill package` (the `loft skill package` Feature): the maintainer's command that writes
//! every plugin package under `plugins/` from the skill sources `loft` embeds, checks them with
//! `--check`, and refuses where the repository's Codex discovery entry is broken.

use std::fs;
use std::os::unix::fs::symlink;
use std::path::Path;
use std::process::Output;

use crate::common::telemetry::{asking, sent};
use crate::common::{EPOCH, Scratch, skill_source, stderr, stdout};
use crate::help;
use crate::schema;
use crate::skill_install::{CLAUDE, CODEX, installed};
use crate::snapshot::snapshot;
use serde_json::{Value, json};
use software_plant::releases::Release;
use software_plant::{Clock, ReleaseFeed};

/// The skill's source and its Codex discovery entry, from the repository's root.
const SOURCE: &str = "crates/loft-cli/skill/loft-spec";
const YARD: &str = "crates/loft-cli/skill/yard";
const DISCOVERY: &str = ".agents/skills/loft-spec";
const LINK: &str = "../../crates/loft-cli/skill/loft-spec";

/// Each plugin's root, with the host directory `loft skill install` writes its package to and
/// its Yard adaptation.
const PLUGINS: [(&str, &str, &str); 2] = [
    ("plugins/claude/loft", CLAUDE, "agents/claude.md"),
    ("plugins/codex/loft", CODEX, "agents/codex.md"),
];

/// A directory standing for the repository of `loft`: the sources of both skills, and the
/// discovery entry linked to the `loft-spec` skill's.
fn repository() -> Scratch {
    let scratch = Scratch::empty();
    copy_tree(&skill_source(), &scratch.0.join(SOURCE));
    copy_tree(&skill_source().join("../yard"), &scratch.0.join(YARD));
    fs::create_dir_all(scratch.0.join(".agents/skills")).unwrap();
    symlink(LINK, scratch.0.join(DISCOVERY)).unwrap();
    scratch
}

/// Copies every file under `from` to the same path under `to`.
fn copy_tree(from: &Path, to: &Path) {
    fs::create_dir_all(to).unwrap();
    for entry in fs::read_dir(from).unwrap() {
        let path = entry.unwrap().path();
        let target = to.join(path.file_name().unwrap());
        if path.is_dir() {
            copy_tree(&path, &target);
        } else {
            fs::copy(&path, &target).unwrap();
        }
    }
}

/// `scratch` after `loft skill package` ran there and succeeded.
fn packaged(scratch: Scratch) -> Scratch {
    let output = scratch.loft(&["skill", "package"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

fn json_of(output: &Output) -> Value {
    schema::conforming(
        &output.stdout,
        include_str!("../../../schemas/skill-package.json"),
    )
}

fn yard_source(path: &str) -> String {
    let source = skill_source().join("../yard").join(path);
    fs::read_to_string(source).unwrap()
}

#[test]
fn after_skill_package_runs_each_package_equals_its_composition() {
    let scratch = packaged(repository());
    let composed = installed();
    for (root, package, adaptation) in PLUGINS {
        assert_eq!(
            snapshot(&scratch.0.join(root).join("skills/loft-spec")),
            snapshot(&composed.0.join(package)),
            "{root}"
        );
        let yard = fs::read_to_string(scratch.0.join(root).join("skills/yard/SKILL.md")).unwrap();
        let expected = format!("{}\n{}", yard_source("SKILL.md"), yard_source(adaptation));
        assert_eq!(yard, expected, "{root}");
        let skills: Vec<String> = fs::read_dir(scratch.0.join(root).join("skills"))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(skills.len(), 2, "{root}: {skills:?}");
    }
}

#[test]
fn skill_package_reports_the_package_directories_it_wrote() {
    let scratch = repository();
    let output = scratch.loft(&["skill", "package", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "packaged");
    assert_eq!(
        document["packages"],
        json!([
            "plugins/claude/loft/skills/loft-spec",
            "plugins/claude/loft/skills/yard",
            "plugins/codex/loft/skills/loft-spec",
            "plugins/codex/loft/skills/yard",
        ])
    );
}

#[test]
fn skill_package_sends_no_command_event_and_asks_for_no_update() {
    let scratch = repository();
    let run = sent(&scratch, &["skill", "package"], None);
    assert_eq!(run.output.status.code(), Some(0), "{}", stderr(&run.output));
    assert!(run.events.is_empty(), "{:?}", run.events);
    let feed = ReleaseFeed::listing(vec![Release::published("v99.0.0")]);
    let mut command = scratch.command(None, &Clock::fixed(EPOCH));
    let output = asking(&mut command, &feed.url())
        .args(["skill", "package"])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(feed.requests().len(), 0);
}

#[test]
fn neither_loft_help_nor_loft_skill_help_lists_skill_package() {
    assert!(help::subcommands().contains(&"skill".to_owned()));
    assert_eq!(help::skill_commands(), ["install"]);
}

#[test]
fn a_committed_package_that_equals_its_composition_checks_with_exit_0() {
    let scratch = packaged(repository());
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["skill", "package", "--check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "current");
    for list in ["changed", "missing", "extra"] {
        assert_eq!(document[list], json!([]), "{list}");
    }
    assert!(snapshot(&scratch.0) == before, "a file changed");
}

#[test]
fn a_changed_missing_or_extra_package_file_is_named_and_exits_1_writing_nothing() {
    let scratch = packaged(repository());
    let changed = "plugins/claude/loft/skills/yard/SKILL.md";
    let missing = "plugins/codex/loft/skills/loft-spec/references/format.md";
    let extra = "plugins/claude/loft/skills/loft-spec/references/stale.md";
    fs::write(scratch.0.join(changed), "edited by hand\n").unwrap();
    fs::remove_file(scratch.0.join(missing)).unwrap();
    fs::write(scratch.0.join(extra), "left behind\n").unwrap();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["skill", "package", "--check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let document = json_of(&output);
    assert_eq!(document["outcome"], "drift");
    assert_eq!(document["changed"], json!([changed]));
    assert_eq!(document["missing"], json!([missing]));
    assert_eq!(document["extra"], json!([extra]));
    let output = scratch.loft(&["skill", "package", "--check"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    for path in [changed, missing, extra] {
        assert!(
            stdout(&output).contains(path),
            "{path}: {}",
            stdout(&output)
        );
    }
    assert!(snapshot(&scratch.0) == before, "a file changed");
}

/// The repository with its discovery entry broken in each way: missing, copied, and linked
/// elsewhere.
fn broken() -> [(&'static str, Scratch); 3] {
    let missing = repository();
    fs::remove_file(missing.0.join(DISCOVERY)).unwrap();
    let copied = repository();
    fs::remove_file(copied.0.join(DISCOVERY)).unwrap();
    fs::create_dir_all(copied.0.join(DISCOVERY).join("agents")).unwrap();
    for path in ["SKILL.md", "agents/openai.yaml"] {
        fs::copy(
            copied.0.join(SOURCE).join(path),
            copied.0.join(DISCOVERY).join(path),
        )
        .unwrap();
    }
    let elsewhere = repository();
    fs::remove_file(elsewhere.0.join(DISCOVERY)).unwrap();
    symlink("../../crates/loft-cli/skill", elsewhere.0.join(DISCOVERY)).unwrap();
    [
        ("missing", missing),
        ("copied", copied),
        ("elsewhere", elsewhere),
    ]
}

#[test]
fn a_broken_discovery_entry_is_refused_and_named_and_no_package_file_changes() {
    for (case, scratch) in broken() {
        let committed = scratch
            .0
            .join("plugins/claude/loft/skills/loft-spec/SKILL.md");
        fs::create_dir_all(committed.parent().unwrap()).unwrap();
        fs::write(&committed, "committed\n").unwrap();
        let before = snapshot(&scratch.0);
        for check in [&[][..], &["--check"]] {
            let output = scratch.loft(&[&["skill", "package"][..], check].concat());
            assert_eq!(output.status.code(), Some(1), "{case}: {}", stderr(&output));
            assert!(
                stderr(&output).contains(DISCOVERY),
                "{case}: {}",
                stderr(&output)
            );
            let args = [&["skill", "package", "--format", "json"][..], check].concat();
            let document = json_of(&scratch.loft(&args));
            assert_eq!(document["outcome"], "error", "{case}");
            assert_eq!(document["errors"][0]["kind"], "discovery", "{case}");
            assert!(snapshot(&scratch.0) == before, "{case}: a file changed");
        }
    }
}

/// A packaged repository whose sources differ from the ones `loft` embeds: one edited, one
/// removed. Gives the two, from the repository's root.
fn stale() -> (Scratch, [String; 2]) {
    let scratch = packaged(repository());
    let edited = format!("{YARD}/agents/codex.md");
    let removed = format!("{SOURCE}/references/format.md");
    fs::write(scratch.0.join(&edited), "edited after the build\n").unwrap();
    fs::remove_file(scratch.0.join(&removed)).unwrap();
    (scratch, [edited, removed])
}

#[test]
fn a_source_that_differs_from_the_embedded_one_is_refused_and_named_and_no_package_file_changes() {
    let (scratch, sources) = stale();
    let before = snapshot(&scratch.0);
    for check in [&[][..], &["--check"]] {
        let output = scratch.loft(&[&["skill", "package"][..], check].concat());
        assert_eq!(
            output.status.code(),
            Some(1),
            "{check:?}: {}",
            stderr(&output)
        );
        for source in &sources {
            assert!(
                stderr(&output).contains(source.as_str()),
                "{check:?} {source}: {}",
                stderr(&output)
            );
        }
        let args = [&["skill", "package", "--format", "json"][..], check].concat();
        let document = json_of(&scratch.loft(&args));
        assert_eq!(document["outcome"], "error", "{check:?}");
        let kinds: Vec<&Value> = document["errors"]
            .as_array()
            .unwrap()
            .iter()
            .map(|error| &error["kind"])
            .collect();
        assert_eq!(kinds, [&json!("stale"), &json!("stale")], "{check:?}");
        assert!(snapshot(&scratch.0) == before, "{check:?}: a file changed");
    }
}

#[test]
fn skill_package_check_does_not_report_current_while_a_source_differs() {
    let (scratch, _) = stale();
    let output = scratch.loft(&["skill", "package", "--check", "--format", "json"]);
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    assert_ne!(json_of(&output)["outcome"], "current");
    let output = scratch.loft(&["skill", "package", "--check"]);
    assert!(
        !stdout(&output).contains("every package equals its composition"),
        "{}",
        stdout(&output)
    );
}
