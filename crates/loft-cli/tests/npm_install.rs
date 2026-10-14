//! The install script of `@ask-loft/loft` (the Feature "The npm package"): it records that npm
//! installed `loft`, an install never fails because of it, and an install whose script never ran
//! behaves as one whose did. The launcher honours both switches.

use std::fs;
use std::path::Path;
use std::process::{Command, Output};

use crate::common::npm::{assembled, host, launch, launcher, manifest, project, unpacked};
use crate::common::{PLATFORMS, Scratch, config_under, isolate, stderr, stdout};
use serde_json::{Value, json};

/// The install script of `@ask-loft/loft` run in `project` for a user whose home is `home`.
fn postinstall(project: &Path, home: &Path) -> Output {
    let mut node = Command::new("node");
    isolate(&mut node, home);
    node.arg(project.join("node_modules/@ask-loft/loft/bin/postinstall.js"))
        .current_dir(project)
        .output()
        .unwrap()
}

#[test]
fn only_the_launcher_package_has_an_install_script_and_each_names_the_repository() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    for platform in PLATFORMS {
        let name = format!("loft-{platform}");
        let package = manifest(&unpacked(&scratch, &out, &name));
        assert_eq!(package["scripts"], Value::Null, "{name}");
        assert_eq!(
            package["repository"]["url"], "git+https://github.com/Ask-Loft/loft.git",
            "{name}"
        );
    }
    let main = manifest(&unpacked(&scratch, &out, "loft"));
    assert_eq!(
        main["scripts"],
        json!({ "postinstall": "node bin/postinstall.js" })
    );
    assert_eq!(
        main["repository"]["url"],
        "git+https://github.com/Ask-Loft/loft.git"
    );
    assert_eq!(main["bin"], json!({ "loft": "bin/loft.js" }));
}

#[test]
fn the_install_script_records_npm_for_the_platform_binary_and_prints_nothing() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    let output = postinstall(&project, &scratch.home());
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
    let binary = project.join(format!("node_modules/@ask-loft/loft-{}/bin/loft", host()));
    let records: Value =
        serde_json::from_slice(&fs::read(scratch.config().join("install.json")).unwrap()).unwrap();
    assert_eq!(records["installs"][0]["way"], json!("npm"));
    assert_eq!(
        records["installs"][0]["path"],
        json!(fs::canonicalize(binary).unwrap())
    );
}

#[test]
fn the_install_succeeds_when_the_platform_package_is_missing() {
    let scratch = Scratch::empty();
    let project = project(&scratch, false);
    let output = postinstall(&project, &scratch.home());
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
    assert!(!scratch.config().join("install.json").exists());
}

#[test]
fn the_install_succeeds_when_the_record_cannot_be_written() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    let blocked = scratch.0.join("not-a-directory");
    fs::write(&blocked, "").unwrap();
    let output = postinstall(&project, &blocked);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(stdout(&output), "");
    assert_eq!(stderr(&output), "");
    assert!(!config_under(&blocked).exists());
}

#[test]
fn an_install_whose_script_never_ran_runs_every_command_as_one_whose_script_did() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    let home = scratch.home();
    let commands: [&[&str]; 3] = [&["--version"], &["check"], &["telemetry", "status"]];
    let without: Vec<Output> = commands
        .iter()
        .map(|args| launch(&project, &home, None, args))
        .collect();
    assert_eq!(postinstall(&project, &home).status.code(), Some(0));
    assert!(scratch.config().join("install.json").exists());
    for (args, before) in commands.iter().zip(without) {
        let after = launch(&project, &home, None, args);
        assert_eq!(after.status.code(), before.status.code(), "{args:?}");
        assert_eq!(stdout(&after), stdout(&before), "{args:?}");
        assert_eq!(stderr(&after), stderr(&before), "{args:?}");
    }
}

#[test]
fn the_launcher_honours_both_switches() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    let home = scratch.home();
    let collector = software_plant::Collector::accepting();
    let feed = software_plant::ReleaseFeed::listing(vec![]);
    let run = |value: Option<&str>| {
        let mut node = launcher(&project, &home, None);
        node.env("LOFT_TELEMETRY_ENDPOINT", collector.url())
            .env("LOFT_UPDATE_ENDPOINT", feed.url());
        for name in ["LOFT_TELEMETRY", "LOFT_UPDATE_CHECK"] {
            match value {
                Some(value) => node.env(name, value),
                None => node.env_remove(name),
            };
        }
        node.arg("check").output().unwrap();
    };
    run(Some("off"));
    assert!(collector.requests().is_empty());
    assert!(feed.requests().is_empty());
    run(None);
    assert_eq!(collector.events().len(), 1);
    assert_eq!(feed.requests().len(), 1);
    for concern in ["telemetry", "update-check"] {
        let recorded = launcher(&project, &home, None)
            .args([concern, "off"])
            .output()
            .unwrap();
        assert_eq!(recorded.status.code(), Some(0), "{concern}");
    }
    run(None);
    assert_eq!(collector.events().len(), 1);
    assert_eq!(feed.requests().len(), 1);
}
