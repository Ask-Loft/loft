//! `loft skill install` (the `loft skill install` Feature): the `loft-spec` skill composed for
//! each host and written into the current directory, each package replacing its directory whole.

use std::fs;
use std::os::unix::fs::{MetadataExt, symlink};

use crate::common::{Scratch, skill_source, stderr};
use crate::schema;
use crate::skill::split;
use crate::snapshot::snapshot;
use serde_json::{Value, json};

pub const CLAUDE: &str = ".claude/skills/loft-spec";
pub const CODEX: &str = ".agents/skills/loft-spec";

/// `scratch` after `loft skill install` ran there and succeeded.
fn install_into(scratch: Scratch) -> Scratch {
    let output = scratch.loft(&["skill", "install"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch
}

/// A directory where `loft skill install` ran and succeeded.
pub fn installed() -> Scratch {
    install_into(Scratch::empty())
}

fn json_of(output: &std::process::Output) -> Value {
    schema::conforming(&output.stdout, include_str!("../../../schemas/skill.json"))
}

#[test]
fn skill_install_writes_a_package_for_each_host_where_there_is_no_spec() {
    let scratch = installed();
    assert!(scratch.0.join(CLAUDE).join("SKILL.md").is_file());
    assert!(scratch.0.join(CODEX).join("SKILL.md").is_file());
    assert!(!scratch.0.join("spec").exists());
}

#[test]
fn the_codex_package_is_the_source_without_the_claude_code_adaptation() {
    let mut expected = snapshot(&skill_source());
    expected.retain(|path, _| !path.starts_with("agents/claude/"));
    assert_eq!(snapshot(&installed().0.join(CODEX)), expected);
}

#[test]
fn the_claude_code_package_is_the_source_with_its_adaptation_applied() {
    let package = snapshot(&installed().0.join(CLAUDE));
    let source = snapshot(&skill_source());
    let mut expected: Vec<String> = source
        .keys()
        .filter(|path| !path.starts_with("agents/"))
        .cloned()
        .collect();
    expected.push("references/claude-host.md".to_owned());
    expected.sort();
    assert_eq!(package.keys().cloned().collect::<Vec<_>>(), expected);
    assert_eq!(
        package["references/claude-host.md"],
        source["agents/claude/claude-host.md"]
    );
    for path in expected.iter().filter(|path| path.as_str() != "SKILL.md") {
        if let Some(bytes) = source.get(path) {
            assert_eq!(&package[path], bytes, "{path}");
        }
    }
    let skill_md = String::from_utf8(package["SKILL.md"].clone()).unwrap();
    let fields: Vec<&str> = split(&skill_md)
        .0
        .iter()
        .map(|line| line.split_once(':').unwrap().0)
        .collect();
    assert_eq!(
        fields,
        ["name", "description", "when_to_use", "allowed-tools"]
    );
    let lines: Vec<&str> = skill_md.lines().collect();
    let anchor = lines
        .iter()
        .position(|line| line.contains("references/the-loop.md"))
        .unwrap();
    let route = lines[anchor..]
        .iter()
        .position(|line| line.contains("references/claude-host.md"))
        .unwrap();
    let next = lines[anchor + 1..]
        .iter()
        .position(|line| line.starts_with("- "))
        .unwrap();
    assert_eq!(route, next + 1, "{skill_md}");
}

#[test]
fn skill_install_replaces_each_directory_whole_and_leaves_no_temporary_directory() {
    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.0.join(CLAUDE)).unwrap();
    fs::write(scratch.0.join(CLAUDE).join("stale.md"), "old").unwrap();
    let scratch = install_into(scratch);
    assert!(!scratch.0.join(CLAUDE).join("stale.md").exists());
    for parent in [".claude/skills", ".agents/skills"] {
        let names: Vec<String> = fs::read_dir(scratch.0.join(parent))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().into_string().unwrap())
            .collect();
        assert_eq!(names, ["loft-spec"], "{parent}");
    }
}

#[test]
fn skill_install_writes_nothing_outside_the_two_skill_directories_and_the_claude_settings() {
    let scratch = Scratch::empty();
    fs::write(scratch.0.join("README.md"), "readme").unwrap();
    let scratch = install_into(scratch);
    let outside: Vec<String> = snapshot(&scratch.0)
        .into_keys()
        .filter(|path| !path.starts_with(CLAUDE) && !path.starts_with(CODEX))
        .collect();
    assert_eq!(outside, [".claude/settings.json", "README.md"]);
}

/// A directory where `.claude` is a file, so the Claude Code package cannot be written, and the
/// Codex directory holds a file of its own.
fn unwritable() -> Scratch {
    let scratch = Scratch::empty();
    fs::write(
        scratch.0.join(".claude"),
        "a file where a directory belongs",
    )
    .unwrap();
    fs::create_dir_all(scratch.0.join(CODEX)).unwrap();
    fs::write(scratch.0.join(CODEX).join("mine.md"), "kept").unwrap();
    scratch
}

#[test]
fn a_skill_directory_that_cannot_be_written_is_an_error_naming_it() {
    let scratch = unwritable();
    let before = snapshot(&scratch.0);
    let output = scratch.loft(&["skill", "install"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(CLAUDE), "{}", stderr(&output));
    assert_eq!(snapshot(&scratch.0), before);
}

#[test]
fn the_json_of_skill_install_names_the_directories_it_wrote() {
    let scratch = Scratch::empty();
    let document = json_of(&scratch.loft(&["skill", "install", "--format", "json"]));
    assert_eq!(document["outcome"], "installed");
    assert_eq!(document["skills"], json!([CLAUDE, CODEX]));
}

#[test]
fn the_json_of_a_failed_skill_install_lists_the_error() {
    let output = unwritable().loft(&["skill", "install", "--format", "json"]);
    let document = json_of(&output);
    assert_eq!(document["outcome"], "error");
    let message = document["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains(CLAUDE), "{message}");
}

#[test]
fn skill_install_renames_a_new_directory_into_each_skill_directory_s_place() {
    let scratch = Scratch::empty();
    let mut before = Vec::new();
    for directory in [CLAUDE, CODEX] {
        fs::create_dir_all(scratch.0.join(directory)).unwrap();
        before.push(fs::metadata(scratch.0.join(directory)).unwrap().ino());
    }
    let scratch = install_into(scratch);
    let composed = installed();
    for (directory, inode) in [CLAUDE, CODEX].into_iter().zip(before) {
        let written = scratch.0.join(directory);
        assert_ne!(fs::metadata(&written).unwrap().ino(), inode, "{directory}");
        assert_eq!(
            snapshot(&written),
            snapshot(&composed.0.join(directory)),
            "{directory}"
        );
    }
}

#[test]
fn a_skill_install_that_fails_leaves_the_skill_directory_as_it_was_and_no_temporary_directory() {
    let scratch = Scratch::empty();
    let skills = scratch.0.join(".claude/skills");
    fs::create_dir_all(&skills).unwrap();
    symlink("nowhere", skills.join("loft-spec")).unwrap();
    let output = scratch.loft(&["skill", "install"]);
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains(CLAUDE), "{}", stderr(&output));
    let names: Vec<String> = fs::read_dir(&skills)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().into_string().unwrap())
        .collect();
    assert_eq!(names, ["loft-spec"]);
    assert_eq!(
        fs::read_link(skills.join("loft-spec")).unwrap(),
        std::path::Path::new("nowhere")
    );
}
