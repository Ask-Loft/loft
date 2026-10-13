//! The npm packages (the Feature "The npm package"): `scripts/npm-packages.sh` assembles them from
//! a binary for each platform, here the `loft` under test copied for every platform, and the
//! launcher of `@ask-loft/loft` runs the binary of the platform it is installed on.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use crate::common::{Scratch, stderr, stdout, workspace};
use serde_json::{Value, json};

const PLATFORMS: [&str; 6] = [
    "darwin-arm64",
    "darwin-x64",
    "linux-arm64",
    "linux-x64",
    "win32-arm64",
    "win32-x64",
];

/// The packages assembled in `scratch/out` from the binary under test.
fn assembled(scratch: &Scratch) -> PathBuf {
    for platform in PLATFORMS {
        let dir = scratch.0.join("binaries").join(platform);
        fs::create_dir_all(&dir).unwrap();
        let name = if platform.starts_with("win32") {
            "loft.exe"
        } else {
            "loft"
        };
        fs::copy(env!("CARGO_BIN_EXE_loft"), dir.join(name)).unwrap();
    }
    let output = Command::new(workspace().join("scripts/npm-packages.sh"))
        .arg(scratch.0.join("binaries"))
        .arg(scratch.0.join("out"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch.0.join("out")
}

fn manifest(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap()
}

/// Node's name for this machine's platform and architecture.
fn host() -> String {
    let output = Command::new("node")
        .args(["-p", "process.platform + '-' + process.arch"])
        .output()
        .expect("node runs the launcher's tests");
    stdout(&output).trim().to_owned()
}

/// A project whose `node_modules` holds `@ask-loft/loft` and, with `platform`, this machine's
/// platform package, as npm installs them.
fn project(scratch: &Scratch, platform: bool) -> PathBuf {
    let out = assembled(scratch);
    let scope = scratch.0.join("project/node_modules/@ask-loft");
    fs::create_dir_all(&scope).unwrap();
    fs::rename(out.join("loft"), scope.join("loft")).unwrap();
    if platform {
        let name = format!("loft-{}", host());
        fs::rename(out.join(&name), scope.join(&name)).unwrap();
    }
    scratch.0.join("project")
}

/// The launcher run with `args` in `project`, with Node first loading `preload` if given.
fn launch(project: &Path, preload: Option<&str>, args: &[&str]) -> Output {
    let mut node = Command::new("node");
    if let Some(script) = preload {
        let path = project.join("preload.js");
        fs::write(&path, script).unwrap();
        node.arg("--require").arg(path);
    }
    node.arg(project.join("node_modules/@ask-loft/loft/bin/loft.js"))
        .args(args)
        .current_dir(project)
        .output()
        .unwrap()
}

#[test]
fn every_package_carries_the_version_of_loft() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    let main = manifest(&out.join("loft"));
    assert_eq!(main["version"], env!("CARGO_PKG_VERSION"));
    let mut optional = json!({});
    for platform in PLATFORMS {
        let package = manifest(&out.join(format!("loft-{platform}")));
        assert_eq!(package["version"], env!("CARGO_PKG_VERSION"), "{platform}");
        optional[format!("@ask-loft/loft-{platform}")] = json!(env!("CARGO_PKG_VERSION"));
    }
    assert_eq!(main["optionalDependencies"], optional);
}

#[test]
fn each_platform_package_declares_its_platform_and_holds_its_binary() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    for platform in PLATFORMS {
        let dir = out.join(format!("loft-{platform}"));
        let package = manifest(&dir);
        let (os, cpu) = platform.split_once('-').unwrap();
        assert_eq!(package["name"], format!("@ask-loft/loft-{platform}"));
        assert_eq!(package["os"], json!([os]));
        assert_eq!(package["cpu"], json!([cpu]));
        let binary = if os == "win32" {
            "bin/loft.exe"
        } else {
            "bin/loft"
        };
        assert!(dir.join(binary).is_file(), "{platform}");
    }
}

#[test]
fn no_package_has_an_install_script_and_each_names_the_repository() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    let names = std::iter::once("loft".to_owned()).chain(PLATFORMS.map(|p| format!("loft-{p}")));
    for name in names {
        let package = manifest(&out.join(&name));
        assert_eq!(package["scripts"], Value::Null, "{name}");
        assert_eq!(
            package["repository"]["url"], "git+https://github.com/Ask-Loft/loft.git",
            "{name}"
        );
    }
    assert_eq!(
        manifest(&out.join("loft"))["bin"],
        json!({ "loft": "bin/loft.js" })
    );
}

#[test]
fn the_launcher_runs_the_binary_with_its_arguments_streams_and_exit_code() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    for args in [&["--version"][..], &["check"][..], &["no-such-command"][..]] {
        let launched = launch(&project, None, args);
        let direct = Command::new(env!("CARGO_BIN_EXE_loft"))
            .args(args)
            .current_dir(&project)
            .output()
            .unwrap();
        assert_eq!(launched.status.code(), direct.status.code(), "{args:?}");
        assert_eq!(stdout(&launched), stdout(&direct), "{args:?}");
        assert_eq!(stderr(&launched), stderr(&direct), "{args:?}");
    }
}

#[test]
fn a_platform_without_a_package_is_reported_with_the_other_way_to_install() {
    let scratch = Scratch::empty();
    let project = project(&scratch, true);
    let elsewhere = "Object.defineProperty(process, 'platform', { value: 'aix' });";
    let output = launch(&project, Some(elsewhere), &["--version"]);
    assert_eq!(output.status.code(), Some(1));
    let report = stderr(&output);
    let arch = host().split_once('-').unwrap().1.to_owned();
    assert!(report.contains("aix") && report.contains(&arch), "{report}");
    assert!(report.contains("cargo install loft-cli"), "{report}");
}

#[test]
fn a_missing_platform_package_is_named_with_the_optional_dependencies() {
    let scratch = Scratch::empty();
    let project = project(&scratch, false);
    let output = launch(&project, None, &["--version"]);
    assert_eq!(output.status.code(), Some(1));
    let report = stderr(&output);
    assert!(
        report.contains(&format!("@ask-loft/loft-{}", host())),
        "{report}"
    );
    assert!(report.contains("optional"), "{report}");
}
