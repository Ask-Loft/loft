//! Writing the skill enables the Claude Code plugin `loft@ask-loft` in the project's
//! `.claude/settings.json`, run against each command whose Feature in this repository's own
//! `spec/` lists the tag `writes-skill`.

use std::fs;
use std::process::Output;

use crate::commands::{Fixture, fixture};
use crate::common::{Scratch, stderr, stdout};
use crate::conformance::tagged_commands;

const TAG: &str = "writes-skill";
const SETTINGS: &str = ".claude/settings.json";

/// What the command writes into a project that has no settings file.
const CREATED: &str = r#"{
  "extraKnownMarketplaces": {
    "ask-loft": {
      "source": {
        "source": "github",
        "repo": "Ask-Loft/loft"
      }
    }
  },
  "enabledPlugins": {
    "loft@ask-loft": true
  }
}
"#;

/// Runs each tagged command over a fresh fixture whose settings file holds `settings`, or none,
/// and gives each command's slug, its output and the settings file it left.
fn each_with(settings: Option<&str>) -> Vec<(&'static str, Output, Option<String>)> {
    let commands = tagged_commands(TAG);
    assert!(!commands.is_empty(), "no Feature lists `{TAG}`");
    commands
        .into_iter()
        .map(|(slug, args, kind, _)| {
            let scratch = fixture(kind);
            if let Some(text) = settings {
                put(&scratch, text);
            }
            let output = scratch.loft(args);
            assert_eq!(output.status.code(), Some(0), "{slug}: {}", stderr(&output));
            let left = fs::read_to_string(scratch.0.join(SETTINGS)).ok();
            (slug, output, left)
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
        assert_eq!(left.as_deref(), Some(CREATED), "{slug}");
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
    let expected = r#"{
  "model": "sonnet",
  "permissions": {
    "deny": [
      "Bash(rm:*)"
    ],
    "allow": [
      "Bash(ls:*)"
    ]
  },
  "enabledPlugins": {
    "zeta@other": true,
    "alpha@other": false,
    "loft@ask-loft": true
  },
  "extraKnownMarketplaces": {
    "ask-loft": {
      "source": {
        "source": "github",
        "repo": "Ask-Loft/loft"
      }
    }
  }
}
"#;
    for (slug, _, left) in each_with(Some(settings)) {
        assert_eq!(left.as_deref(), Some(expected), "{slug}");
    }
}

#[test]
fn a_marketplace_ask_loft_the_settings_already_name_is_kept_as_it_is() {
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
    let settings = CREATED.replace("  ", "    ");
    for (slug, output, left) in each_with(Some(&settings)) {
        assert_eq!(left.as_deref(), Some(settings.as_str()), "{slug}");
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
