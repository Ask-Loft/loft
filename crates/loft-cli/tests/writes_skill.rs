//! Writing the skill: every command tagged `writes-skill` asks in its text output for the skill
//! directories it wrote to be committed, run against each command whose Feature in this
//! repository's own `spec/` lists the tag; and the converse, that every command that writes a
//! skill directory lists it.

use std::fs;
use std::process::{Command, Output};

use crate::commands::{COMMANDS, Fixture};
use crate::common::{Scratch, stderr, stdout};
use crate::conformance::{tagged, tagged_commands};
use software_plant::GitScenario;
use software_plant::real_git::{isolate, setup_git};

const TAG: &str = "writes-skill";

/// The skill directories a command writes into a project.
const SKILL_DIRECTORIES: [&str; 2] = [".claude/skills/loft-spec", ".agents/skills/loft-spec"];

/// A scratch directory for a command: an empty one, or the Reading List.
fn fixture(fixture: Fixture) -> Scratch {
    let scratch = Scratch::with_reading_list();
    if fixture == Fixture::Empty {
        fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    }
    scratch
}

/// The line of a text output that asks for the skill to be committed.
fn request(output: &Output) -> Option<String> {
    stdout(output)
        .lines()
        .find(|line| line.starts_with("commit "))
        .map(str::to_owned)
}

#[test]
fn each_tagged_command_asks_for_the_skill_directories_it_wrote_to_be_committed() {
    let commands = tagged_commands(TAG);
    assert!(!commands.is_empty(), "no Feature lists `{TAG}`");
    for (slug, args, kind) in commands {
        let scratch = fixture(kind);
        let (output, after) = scratch.loft_in(args, &GitScenario::clean_on("main"));
        assert_eq!(output.status.code(), Some(0), "{slug}: {}", stderr(&output));
        let line = request(&output).unwrap_or_else(|| panic!("{slug}: {}", stdout(&output)));
        for directory in SKILL_DIRECTORIES {
            assert!(line.contains(directory), "{slug}: {line}");
        }
        assert!(line.contains("every clone"), "{slug}: {line}");
        assert_eq!(after, GitScenario::clean_on("main"), "{slug}: git changed");
    }
}

#[test]
fn a_tagged_command_s_json_document_carries_no_request() {
    for (slug, args, kind) in tagged_commands(TAG) {
        let scratch = fixture(kind);
        let output = scratch.loft(&[args, &["--format", "json"][..]].concat());
        assert_eq!(output.status.code(), Some(0), "{slug}: {}", stderr(&output));
        assert!(
            !stdout(&output).contains("commit "),
            "{slug}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn init_with_no_skill_asks_for_no_commit() {
    let output = fixture(Fixture::Empty).loft(&["init", "--no-skill"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(request(&output), None, "{}", stdout(&output));
}

#[test]
fn each_command_that_writes_a_skill_directory_lists_writes_skill() {
    let tagged = tagged(TAG);
    for (slug, args, kind) in COMMANDS {
        let scratch = fixture(kind);
        scratch.loft(args);
        let writes = SKILL_DIRECTORIES
            .iter()
            .any(|directory| scratch.0.join(directory).exists());
        assert_eq!(
            writes,
            tagged.iter().any(|tag| tag == slug),
            "{slug}: writes a skill directory exactly when its Feature lists `{TAG}`"
        );
    }
}

/// What a commit would change in the repository under real git: `HEAD`, the number of commits
/// and the index.
fn commits(scratch: &Scratch) -> [Vec<u8>; 3] {
    let git = |args: &[&str]| {
        let output = setup_git()
            .arg("-C")
            .arg(&scratch.0)
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
        output.stdout
    };
    [
        git(&["rev-parse", "HEAD"]),
        git(&["rev-list", "--count", "HEAD"]),
        git(&["ls-files", "--stage"]),
    ]
}

#[test]
fn each_tagged_command_commits_nothing_under_real_git() {
    for (slug, args, kind) in tagged_commands(TAG) {
        let scratch = fixture(kind);
        let init = setup_git()
            .arg("-C")
            .arg(&scratch.0)
            .args(["init", "--quiet", "-b", "main"])
            .output()
            .unwrap();
        assert!(init.status.success(), "{}", stderr(&init));
        let base = setup_git()
            .arg("-C")
            .arg(&scratch.0)
            .args(["commit", "--quiet", "--allow-empty", "-m", "base"])
            .output()
            .unwrap();
        assert!(base.status.success(), "{}", stderr(&base));
        let before = commits(&scratch);
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        isolate(&mut command).current_dir(&scratch.0);
        let output = command.args(args).output().unwrap();
        assert_eq!(output.status.code(), Some(0), "{slug}: {}", stderr(&output));
        assert!(request(&output).is_some(), "{slug}: {}", stdout(&output));
        assert_eq!(commits(&scratch), before, "{slug}: the repository changed");
    }
}
