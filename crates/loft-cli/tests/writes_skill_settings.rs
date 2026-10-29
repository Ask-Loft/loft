//! Writing the skill enables the Claude Code plugin `loft@ask-loft` in the project's
//! `.claude/settings.json`, and leaves the file alone with `--no-plugin`, run against each command
//! whose Feature in this repository's own `spec/` lists the tag `writes-skill`.

use std::fs;
use std::process::Output;

use crate::commands::{Fixture, fixture};
use crate::common::{Scratch, stderr, stdout};
use crate::conformance::tagged_commands;
use crate::skill_install::{CLAUDE, CODEX};
use crate::snapshot::snapshot;

const TAG: &str = "writes-skill";
const SETTINGS: &str = ".claude/settings.json";

/// The release tag the marketplace is pinned to: the running `loft`'s version.
const REF: &str = concat!("v", env!("CARGO_PKG_VERSION"));

/// What the command writes into a project that has no settings file.
fn created() -> String {
    format!(
        r#"{{
  "extraKnownMarketplaces": {{
    "ask-loft": {{
      "source": {{
        "source": "github",
        "repo": "Ask-Loft/loft",
        "ref": "{REF}"
      }}
    }}
  }},
  "enabledPlugins": {{
    "loft@ask-loft": true
  }}
}}
"#
    )
}

/// Runs each tagged command over a fresh fixture whose settings file holds `settings`, or none,
/// and gives each command's slug, its output and the settings file it left.
fn each_with(settings: Option<&str>) -> Vec<(&'static str, Output, Option<String>)> {
    each_run(settings, &[])
        .into_iter()
        .map(|(slug, output, scratch)| {
            let left = fs::read_to_string(scratch.0.join(SETTINGS)).ok();
            (slug, output, left)
        })
        .collect()
}

/// Runs each tagged command, with `extra` after its arguments, over a fresh fixture whose
/// settings file holds `settings`, or none, and gives each command's slug, its output and the
/// directory it ran in.
fn each_run(settings: Option<&str>, extra: &[&str]) -> Vec<(&'static str, Output, Scratch)> {
    let commands = tagged_commands(TAG);
    assert!(!commands.is_empty(), "no Feature lists `{TAG}`");
    commands
        .into_iter()
        .map(|(slug, args, kind, _)| {
            let scratch = fixture(kind);
            if let Some(text) = settings {
                put(&scratch, text);
            }
            let args: Vec<&str> = args.iter().chain(extra).copied().collect();
            let output = scratch.loft(&args);
            assert_eq!(output.status.code(), Some(0), "{slug}: {}", stderr(&output));
            (slug, output, scratch)
        })
        .collect()
}

fn put(scratch: &Scratch, text: &str) {
    fs::create_dir_all(scratch.0.join(".claude")).unwrap();
    fs::write(scratch.0.join(SETTINGS), text).unwrap();
}

fn request(output: &Output) -> String {
    stdout(output)
        .lines()
        .find(|line| line.starts_with("commit "))
        .unwrap_or_default()
        .to_owned()
}

#[test]
fn a_project_without_settings_gets_the_marketplace_and_the_plugin_alone() {
    for (slug, _, left) in each_with(None) {
        assert_eq!(left, Some(created()), "{slug}");
    }
}

#[test]
fn every_other_entry_of_the_settings_is_kept_in_its_order() {
    let settings = r#"{
  "model": "sonnet",
  "permissions": {
    "deny": ["Bash(rm:*)"],
    "allow": ["Bash(ls:*)"]
  },
  "enabledPlugins": {
    "zeta@other": true,
    "alpha@other": false
  }
}
"#;
    let expected = format!(
        r#"{{
  "model": "sonnet",
  "permissions": {{
    "deny": [
      "Bash(rm:*)"
    ],
    "allow": [
      "Bash(ls:*)"
    ]
  }},
  "enabledPlugins": {{
    "zeta@other": true,
    "alpha@other": false,
    "loft@ask-loft": true
  }},
  "extraKnownMarketplaces": {{
    "ask-loft": {{
      "source": {{
        "source": "github",
        "repo": "Ask-Loft/loft",
        "ref": "{REF}"
      }}
    }}
  }}
}}
"#
    );
    for (slug, _, left) in each_with(Some(settings)) {
        assert_eq!(left, Some(expected.clone()), "{slug}");
    }
}

#[test]
fn a_marketplace_ask_loft_from_another_source_is_kept_as_it_is() {
    let settings = r#"{
  "extraKnownMarketplaces": {
    "ask-loft": {
      "source": {
        "source": "directory",
        "path": "../loft"
      }
    }
  }
}
"#;
    for (slug, _, left) in each_with(Some(settings)) {
        let left = left.unwrap();
        assert!(left.contains(r#""path": "../loft""#), "{slug}: {left}");
        assert!(!left.contains("Ask-Loft/loft"), "{slug}: {left}");
        assert!(left.contains(r#""loft@ask-loft": true"#), "{slug}: {left}");
        assert!(!left.contains(r#""ref""#), "{slug}: {left}");
    }
}

#[test]
fn a_marketplace_ask_loft_from_the_repository_is_moved_to_the_running_release() {
    let settings = r#"{
  "extraKnownMarketplaces": {
    "ask-loft": {
      "source": {
        "source": "github",
        "repo": "Ask-Loft/loft",
        "ref": "v0.0.1"
      },
      "autoUpdate": true
    }
  },
  "enabledPlugins": {
    "loft@ask-loft": true
  }
}
"#;
    let expected = settings.replace("v0.0.1", REF);
    for (slug, output, left) in each_with(Some(settings)) {
        assert_eq!(left.as_deref(), Some(expected.as_str()), "{slug}");
        assert!(
            request(&output).contains(SETTINGS),
            "{slug}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn a_plugin_entry_the_settings_already_hold_is_kept_false_included() {
    let settings = "{\n  \"enabledPlugins\": {\n    \"loft@ask-loft\": false\n  }\n}\n";
    for (slug, _, left) in each_with(Some(settings)) {
        let left = left.unwrap();
        assert!(left.contains(r#""loft@ask-loft": false"#), "{slug}: {left}");
        assert!(left.contains(r#""ask-loft": {"#), "{slug}: {left}");
    }
}

#[test]
fn settings_that_hold_both_entries_are_not_written_and_not_asked_to_be_committed() {
    let elsewhere = r#"{"extraKnownMarketplaces": {"ask-loft": {"source": {"source": "directory", "path": "../loft"}}}, "enabledPlugins": {"loft@ask-loft": true}}"#;
    for settings in [created().replace("  ", "    "), elsewhere.to_owned()] {
        unchanged(&settings);
    }
}

/// Runs each tagged command over `settings` and checks that it neither writes them nor asks for
/// them to be committed.
fn unchanged(settings: &str) {
    for (slug, output, left) in each_with(Some(settings)) {
        assert_eq!(left.as_deref(), Some(settings), "{slug}");
        assert!(
            !request(&output).contains(SETTINGS),
            "{slug}: {}",
            stdout(&output)
        );
        assert!(
            !stdout(&output).contains("enabled the"),
            "{slug}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn the_request_to_commit_names_the_settings_where_the_command_wrote_them() {
    for (slug, output, _) in each_with(None) {
        let line = request(&output);
        assert!(line.contains(SETTINGS), "{slug}: {line}");
        assert!(
            stdout(&output).contains(
                "enabled the Claude Code plugin loft@ask-loft in .claude/settings.json\n"
            ),
            "{slug}: {}",
            stdout(&output)
        );
    }
}

#[test]
fn settings_that_are_not_a_json_object_are_left_alone_with_the_entries_to_add_by_hand() {
    for settings in ["[1, 2]\n", "{ not json\n", "{\"enabledPlugins\": []}\n"] {
        for (slug, output, left) in each_with(Some(settings)) {
            assert_eq!(left.as_deref(), Some(settings), "{slug}");
            let text = stdout(&output);
            let line = text
                .lines()
                .find(|line| line.starts_with(SETTINGS))
                .unwrap_or_else(|| panic!("{slug}: {text}"));
            assert!(line.contains("extraKnownMarketplaces"), "{slug}: {line}");
            assert!(
                line.contains(&format!(r#""ref": "{REF}""#)),
                "{slug}: {line}"
            );
            assert!(
                line.contains(r#""enabledPlugins": {"loft@ask-loft": true}"#),
                "{slug}: {line}"
            );
            assert!(!request(&output).contains(SETTINGS), "{slug}: {text}");
        }
    }
}

#[test]
fn init_with_no_skill_writes_no_settings() {
    let scratch = fixture(Fixture::Empty);
    let output = scratch.loft(&["init", "--no-skill"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(!scratch.0.join(SETTINGS).exists());
}

const NO_PLUGIN: &[&str] = &["--no-plugin"];

#[test]
fn with_no_plugin_settings_are_left_byte_for_byte_whether_or_not_they_are_an_object() {
    for settings in [
        "{\n  \"model\": \"sonnet\"\n}\n",
        "[1, 2]\n",
        "{ not json\n",
    ] {
        for (slug, _, scratch) in each_run(Some(settings), NO_PLUGIN) {
            let left = fs::read_to_string(scratch.0.join(SETTINGS)).unwrap();
            assert_eq!(left, settings, "{slug}");
        }
    }
}

#[test]
fn with_no_plugin_absent_settings_are_not_created() {
    for (slug, _, scratch) in each_run(None, NO_PLUGIN) {
        assert!(!scratch.0.join(SETTINGS).exists(), "{slug}");
    }
}

#[test]
fn with_no_plugin_the_text_names_no_plugin_and_gives_nothing_to_add_by_hand() {
    for settings in [None, Some("[1, 2]\n")] {
        for (slug, output, _) in each_run(settings, NO_PLUGIN) {
            let text = stdout(&output);
            assert!(!text.contains("loft@ask-loft"), "{slug}: {text}");
            assert!(!text.contains(SETTINGS), "{slug}: {text}");
        }
    }
}

#[test]
fn with_no_plugin_the_request_to_commit_names_the_skill_directories_alone() {
    for (slug, output, _) in each_run(None, NO_PLUGIN) {
        assert_eq!(
            request(&output),
            format!(
                "commit {CLAUDE} and {CODEX}, so that every clone's coding agents have the skill"
            ),
            "{slug}"
        );
    }
}

#[test]
fn with_no_plugin_the_skill_directories_are_written_as_without_it() {
    let with = each_run(None, &[]);
    let without = each_run(None, NO_PLUGIN);
    for ((slug, _, plugin), (_, _, alone)) in with.iter().zip(&without) {
        for directory in [CLAUDE, CODEX] {
            assert_eq!(
                snapshot(&alone.0.join(directory)),
                snapshot(&plugin.0.join(directory)),
                "{slug}: {directory}"
            );
        }
    }
}
