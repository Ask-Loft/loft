//! The setup report (The setup report Feature): `loft init` and `loft skill install` end with the
//! state of the git repository, Loft's merge driver, the `loft` the hosts' hooks run and the
//! Claude Code plugin, and the step that remains for each, under the git simulator.

use std::fs;
use std::os::unix::fs::PermissionsExt as _;
use std::process::Output;

use serde_json::Value;
use software_plant::GitScenario;
use software_plant::git::Call;

use crate::common::{Scratch, stderr, stdout};
use crate::schema;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Each command that ends with the report, by its arguments, with the schema of its JSON document.
const COMMANDS: [(&[&str], &str); 2] = [
    (&["init"], include_str!("../../../schemas/init.json")),
    (
        &["skill", "install"],
        include_str!("../../../schemas/skill.json"),
    ),
];

/// A repository on `main` with the merge driver registered, whose hooks find this `loft`.
fn ready() -> GitScenario {
    GitScenario::clean_on("main")
        .with_config("merge.loft.driver", "loft merge-driver %O %A %B %P")
        .launcher("loft", &format!("loft {VERSION}"))
}

/// Runs `args` with `extra` in a fresh directory under `scenario`, and gives its output and the
/// scenario as the run left it; the command succeeds.
fn run_in(
    scratch: &Scratch,
    args: &[&str],
    extra: &[&str],
    scenario: &GitScenario,
) -> (Output, GitScenario) {
    let args: Vec<&str> = args.iter().chain(extra).copied().collect();
    let (output, after) = scratch.loft_in(&args, scenario);
    assert_eq!(
        output.status.code(),
        Some(0),
        "{args:?}: {}",
        stderr(&output)
    );
    (output, after)
}

/// The report of each command under `scenario`, with `extra` after its arguments, read from its
/// JSON document, which conforms to the command's schema.
fn reports(scenario: &GitScenario, extra: &[&str]) -> Vec<(String, Vec<Value>)> {
    reports_in(&Scratch::empty, scenario, extra)
}

fn reports_in(
    scratch: &dyn Fn() -> Scratch,
    scenario: &GitScenario,
    extra: &[&str],
) -> Vec<(String, Vec<Value>)> {
    COMMANDS
        .iter()
        .map(|(args, schema)| {
            let extra: Vec<&str> = extra.iter().chain(&["--format", "json"]).copied().collect();
            let (output, _) = run_in(&scratch(), args, &extra, scenario);
            let json = schema::conforming(&output.stdout, schema);
            let setup = json["setup"].as_array().expect("a setup list").clone();
            (args.join(" "), setup)
        })
        .collect()
}

/// The entry of `check` in `setup`, if it is listed.
fn entry<'a>(setup: &'a [Value], check: &str) -> Option<&'a Value> {
    setup.iter().find(|entry| entry["check"] == check)
}

/// The state and the step of `check`, which is listed.
fn state_and_step(setup: &[Value], check: &str) -> (String, Option<String>) {
    let entry = entry(setup, check).unwrap_or_else(|| panic!("no {check}: {setup:?}"));
    (
        entry["state"].as_str().unwrap().to_owned(),
        entry["step"].as_str().map(str::to_owned),
    )
}

/// The step of `check`, whose state is `state`.
fn step(setup: &[Value], check: &str, state: &str) -> String {
    let (found, step) = state_and_step(setup, check);
    assert_eq!(found, state, "{check}: {setup:?}");
    step.unwrap_or_else(|| panic!("{check} holds no step: {setup:?}"))
}

#[test]
fn the_report_gives_each_check_in_order_with_its_state_and_no_step_where_it_passed() {
    for (command, setup) in reports(&ready(), &[]) {
        let checks: Vec<&str> = setup
            .iter()
            .map(|entry| entry["check"].as_str().unwrap())
            .collect();
        assert_eq!(
            checks,
            ["git", "merge-driver", "hooks-loft", "plugin"],
            "{command}"
        );
        for check in ["git", "merge-driver", "hooks-loft"] {
            assert_eq!(
                state_and_step(&setup, check),
                ("ok".to_owned(), None),
                "{command}"
            );
        }
        assert_eq!(state_and_step(&setup, "plugin").0, "unchecked", "{command}");
    }
}

#[test]
fn the_text_output_ends_with_a_next_line_for_each_step() {
    let scenario = GitScenario::clean_on("main");
    for (args, _) in COMMANDS {
        let (output, _) = run_in(&Scratch::empty(), args, &[], &scenario);
        let text = stdout(&output);
        let next: Vec<&str> = text.lines().rev().take(3).collect();
        assert!(
            next.iter().all(|line| line.starts_with("next: ")),
            "{args:?}: {text}"
        );
        assert!(
            next[2].contains("loft merge-driver --install"),
            "{args:?}: {text}"
        );
        assert!(next[1].contains("search path"), "{args:?}: {text}");
        assert!(next[0].contains("claude plugin list"), "{args:?}: {text}");
        assert_eq!(text.matches("next: ").count(), 3, "{args:?}: {text}");
    }
}

#[test]
fn without_the_merge_driver_the_report_steps_to_installing_it() {
    for (command, setup) in reports(&GitScenario::clean_on("main"), &[]) {
        let step = step(&setup, "merge-driver", "action");
        assert!(
            step.contains("loft merge-driver --install"),
            "{command}: {step}"
        );
        assert!(
            step.contains("--command \"npx --no-install loft\""),
            "{command}: {step}"
        );
        assert_eq!(state_and_step(&setup, "git").0, "ok", "{command}");
    }
}

#[test]
fn a_registered_merge_driver_is_ok() {
    let scenario = GitScenario::clean_on("main").with_config("merge.loft.driver", "loft");
    for (command, setup) in reports(&scenario, &[]) {
        assert_eq!(
            state_and_step(&setup, "merge-driver"),
            ("ok".to_owned(), None),
            "{command}"
        );
    }
}

#[test]
fn where_git_fails_the_merge_driver_is_unchecked_and_the_command_succeeds() {
    let scenario = GitScenario::clean_on("main").failing(Call::ConfigGet, 128, "fatal: unreadable");
    for (command, setup) in reports(&scenario, &[]) {
        let (state, step) = state_and_step(&setup, "merge-driver");
        assert_eq!((state.as_str(), step), ("unchecked", None), "{command}");
    }
}

#[test]
fn where_git_cannot_be_run_the_checks_are_unchecked_and_the_command_succeeds() {
    for (args, schema) in COMMANDS {
        let scratch = Scratch::empty();
        let args: Vec<&str> = args.iter().chain(&["--format", "json"]).copied().collect();
        let output = scratch.loft(&args);
        assert_eq!(
            output.status.code(),
            Some(0),
            "{args:?}: {}",
            stderr(&output)
        );
        let json = schema::conforming(&output.stdout, schema);
        let setup = json["setup"].as_array().unwrap();
        for check in ["git", "merge-driver", "hooks-loft"] {
            let (state, step) = state_and_step(setup, check);
            assert_eq!(
                (state.as_str(), step),
                ("unchecked", None),
                "{args:?} {check}"
            );
        }
    }
}

#[test]
fn the_report_writes_nothing_to_the_repository() {
    for (args, _) in COMMANDS {
        let scratch = Scratch::empty();
        let scenario = GitScenario::clean_on("main");
        let (output, after) = run_in(&scratch, args, &[], &scenario);
        assert!(stdout(&output).contains("next: "), "{args:?}");
        assert_eq!(after, scenario, "{args:?}");
        assert!(!scratch.0.join(".gitattributes").exists(), "{args:?}");
    }
}

#[test]
fn outside_a_repository_git_steps_to_a_repository_and_the_rest_is_unchecked() {
    for (command, setup) in reports(&GitScenario::outside_repository(), &[]) {
        let step = step(&setup, "git", "action");
        assert!(
            step.contains("the merge driver and Yard Log need a git repository"),
            "{command}: {step}"
        );
        assert!(
            step.contains("loft merge-driver --install"),
            "{command}: {step}"
        );
        for check in ["merge-driver", "hooks-loft"] {
            let (state, step) = state_and_step(&setup, check);
            assert_eq!(
                (state.as_str(), step),
                ("unchecked", None),
                "{command} {check}"
            );
        }
    }
}

#[test]
fn hooks_that_find_another_loft_name_both_versions_and_how_to_install_this_one() {
    let scenario = GitScenario::clean_on("main").launcher("loft", "loft 2.2.1");
    for (command, setup) in reports(&scenario, &[]) {
        let step = step(&setup, "hooks-loft", "action");
        assert!(step.contains("loft 2.2.1"), "{command}: {step}");
        assert!(
            step.contains(&format!("not loft {VERSION}")),
            "{command}: {step}"
        );
        assert!(
            step.contains(&format!("`npm install --global @ask-loft/loft@{VERSION}`")),
            "{command}: {step}"
        );
        assert!(
            step.contains(&format!(
                "`cargo install --locked loft-cli --version {VERSION}`"
            )),
            "{command}: {step}"
        );
        assert!(
            step.contains("Codex's hooks run `loft` from the search path alone"),
            "{command}: {step}"
        );
        assert!(!step.contains("loftCommand"), "{command}: {step}");
    }
}

#[test]
fn hooks_that_find_no_loft_say_so() {
    for (command, setup) in reports(&GitScenario::clean_on("main"), &[]) {
        let step = step(&setup, "hooks-loft", "action");
        assert!(step.contains("has no loft"), "{command}: {step}");
        assert!(
            step.contains(&format!("not loft {VERSION}")),
            "{command}: {step}"
        );
    }
}

#[test]
fn a_project_s_lockfile_names_the_runner_for_the_plugin_s_loft_command() {
    let scenario = GitScenario::clean_on("main").launcher("loft", "loft 2.2.1");
    for (lockfile, runner) in [
        ("pnpm-lock.yaml", "pnpm exec loft"),
        ("yarn.lock", "yarn loft"),
        ("package-lock.json", "npx --no-install loft"),
    ] {
        let holding = || {
            let scratch = Scratch::empty();
            fs::write(scratch.0.join(lockfile), "").unwrap();
            scratch
        };
        for (command, setup) in reports_in(&holding, &scenario, &[]) {
            let step = step(&setup, "hooks-loft", "action");
            assert!(
                step.contains(&format!(
                    "set the Claude Code plugin's loftCommand to `{runner}`"
                )),
                "{command} {lockfile}: {step}"
            );
        }
    }
}

#[test]
fn hooks_that_find_this_loft_are_ok_through_git() {
    let (_, setup) = &reports(&ready(), &[])[0];
    assert_eq!(state_and_step(setup, "hooks-loft"), ("ok".to_owned(), None));
}

#[test]
fn the_plugin_step_brings_an_installed_plugin_to_this_version() {
    for (command, setup) in reports(&ready(), &[]) {
        let step = step(&setup, "plugin", "unchecked");
        assert!(
            step.contains(&format!("loft@ask-loft at v{VERSION}")),
            "{command}: {step}"
        );
        assert!(
            step.contains("keeps its own entry until it is added again"),
            "{command}: {step}"
        );
        for call in [
            "`claude plugin marketplace remove ask-loft`".to_owned(),
            format!("`claude plugin marketplace add Ask-Loft/loft@v{VERSION}`"),
            "`claude plugin install loft@ask-loft`".to_owned(),
            "`claude plugin list`".to_owned(),
        ] {
            assert!(step.contains(&call), "{command}: {step}");
        }
    }
}

#[test]
fn the_plugin_is_listed_only_where_the_claude_code_skill_is_written_with_the_plugin() {
    let without = |args: &[&str], extra: &[&str]| {
        let schema = if args[0] == "init" {
            COMMANDS[0].1
        } else {
            COMMANDS[1].1
        };
        let (output, _) = run_in(
            &Scratch::empty(),
            args,
            &[extra, &["--format", "json"]].concat(),
            &ready(),
        );
        let json = schema::conforming(&output.stdout, schema);
        let setup = json["setup"].as_array().unwrap().clone();
        assert!(
            entry(&setup, "plugin").is_none(),
            "{args:?} {extra:?}: {setup:?}"
        );
        assert!(
            entry(&setup, "hooks-loft").is_some(),
            "{args:?} {extra:?}: {setup:?}"
        );
    };
    without(&["init"], &["--no-skill"]);
    without(&["init"], &["--no-plugin"]);
    without(&["skill", "install"], &["--no-plugin"]);
    without(&["skill", "install"], &["--host", "codex"]);
}

#[test]
fn a_refusal_gives_no_report() {
    let scratch = Scratch::empty();
    fs::create_dir(scratch.0.join("spec")).unwrap();
    let (output, _) = scratch.loft_in(&["init", "--format", "json"], &ready());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json = schema::conforming(&output.stdout, COMMANDS[0].1);
    assert!(json.get("setup").is_none(), "{json}");
    assert!(!stdout(&output).contains("next: "));

    let scratch = Scratch::empty();
    fs::create_dir_all(scratch.0.join(".claude/skills")).unwrap();
    std::os::unix::fs::symlink("missing", scratch.0.join(".claude/skills/loft-spec")).unwrap();
    let (output, _) = scratch.loft_in(&["skill", "install", "--format", "json"], &ready());
    assert_eq!(output.status.code(), Some(1), "{}", stderr(&output));
    let json = schema::conforming(&output.stdout, COMMANDS[1].1);
    assert!(json.get("setup").is_none(), "{json}");
}

#[test]
fn the_hooks_loft_is_found_on_the_search_path_without_node_modules_bin_under_real_git() {
    let scratch = Scratch::empty();
    let script = |dir: &str, version: &str| {
        let dir = scratch.0.join(dir);
        fs::create_dir_all(&dir).unwrap();
        let loft = dir.join("loft");
        fs::write(&loft, format!("#!/bin/sh\necho 'loft {version}'\n")).unwrap();
        fs::set_permissions(&loft, fs::Permissions::from_mode(0o755)).unwrap();
        dir
    };
    let local = script("node_modules/.bin", VERSION);
    let global = script("global", "0.0.1");
    software_plant::real_git::git(&scratch.0, &["init", "-q"]);
    let output = scratch
        .loft_under_real_git_with_path_first(&["init", "--format", "json"], &[local, global]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    let json = schema::conforming(&output.stdout, COMMANDS[0].1);
    let step = step(json["setup"].as_array().unwrap(), "hooks-loft", "action");
    assert!(step.contains("starts loft 0.0.1"), "{step}");
}
