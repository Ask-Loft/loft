//! The npm packages (the Feature "The npm package"): `scripts/npm-packages.sh` packs them into
//! tarballs from a binary for each platform, here the `loft` under test copied for every platform,
//! and the launcher of `@ask-loft/loft` runs the binary of the platform it is installed on.
//! `scripts/npm-smoke.sh` runs that launcher from the tarballs, as the release does before it
//! publishes them.

use std::fs;
use std::os::unix::fs::PermissionsExt;
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

/// The tarballs packed in `scratch/out` from the binary under test, each copied in without its
/// execute bits, as a download of the build's artifacts leaves it.
fn assembled(scratch: &Scratch) -> PathBuf {
    for platform in PLATFORMS {
        let dir = scratch.0.join("binaries").join(platform);
        fs::create_dir_all(&dir).unwrap();
        let name = if platform.starts_with("win32") {
            "loft.exe"
        } else {
            "loft"
        };
        let binary = dir.join(name);
        fs::copy(env!("CARGO_BIN_EXE_loft"), &binary).unwrap();
        fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
    }
    let output = Command::new(workspace().join("scripts/npm-packages.sh"))
        .arg(scratch.0.join("binaries"))
        .arg(scratch.0.join("out"))
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    scratch.0.join("out")
}

/// The package packed as `<out>/<name>.tgz`, extracted into `into` as npm installs it.
fn unpack(out: &Path, name: &str, into: &Path) -> PathBuf {
    fs::create_dir_all(into).unwrap();
    let output = Command::new("tar")
        .arg("-xzf")
        .arg(out.join(format!("{name}.tgz")))
        .args(["--strip-components", "1", "-C"])
        .arg(into)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(0), "{name}: {}", stderr(&output));
    into.to_path_buf()
}

/// The package packed as `<out>/<name>.tgz`, extracted in the scratch directory.
fn unpacked(scratch: &Scratch, out: &Path, name: &str) -> PathBuf {
    unpack(out, name, &scratch.0.join("unpacked").join(name))
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
/// platform package, extracted from their tarballs as npm installs them.
fn project(scratch: &Scratch, platform: bool) -> PathBuf {
    let out = assembled(scratch);
    let scope = scratch.0.join("project/node_modules/@ask-loft");
    unpack(&out, "loft", &scope.join("loft"));
    if platform {
        let name = format!("loft-{}", host());
        unpack(&out, &name, &scope.join(&name));
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

/// `scripts/npm-smoke.sh` run over the tarballs in `out`.
fn smoke(out: &Path) -> Output {
    Command::new(workspace().join("scripts/npm-smoke.sh"))
        .arg(out)
        .output()
        .unwrap()
}

#[test]
fn every_package_carries_the_version_of_loft() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    let main = manifest(&unpacked(&scratch, &out, "loft"));
    assert_eq!(main["version"], env!("CARGO_PKG_VERSION"));
    let mut optional = json!({});
    for platform in PLATFORMS {
        let package = manifest(&unpacked(&scratch, &out, &format!("loft-{platform}")));
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
        let dir = unpacked(&scratch, &out, &format!("loft-{platform}"));
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
        if os != "win32" {
            let mode = fs::metadata(dir.join(binary)).unwrap().permissions().mode();
            assert_eq!(mode & 0o111, 0o111, "{platform}: {mode:o}");
        }
    }
}

#[test]
fn no_package_has_an_install_script_and_each_names_the_repository() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    let names = std::iter::once("loft".to_owned()).chain(PLATFORMS.map(|p| format!("loft-{p}")));
    for name in names {
        let package = manifest(&unpacked(&scratch, &out, &name));
        assert_eq!(package["scripts"], Value::Null, "{name}");
        assert_eq!(
            package["repository"]["url"], "git+https://github.com/Ask-Loft/loft.git",
            "{name}"
        );
    }
    assert_eq!(
        manifest(&unpacked(&scratch, &out, "loft"))["bin"],
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

#[test]
fn the_smoke_check_runs_loft_from_the_tarballs_and_reports_its_version() {
    let scratch = Scratch::empty();
    let output = smoke(&assembled(&scratch));
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert!(
        stdout(&output).contains(env!("CARGO_PKG_VERSION")),
        "{}",
        stdout(&output)
    );
}

#[test]
fn the_smoke_check_fails_where_the_packed_binary_cannot_run() {
    let scratch = Scratch::empty();
    let out = assembled(&scratch);
    let name = format!("loft-{}", host());
    let staged = scratch.0.join("repack");
    fs::create_dir_all(&staged).unwrap();
    fs::rename(unpacked(&scratch, &out, &name), staged.join("package")).unwrap();
    let binary = staged.join("package/bin/loft");
    fs::set_permissions(&binary, fs::Permissions::from_mode(0o644)).unwrap();
    let packed = Command::new("tar")
        .arg("-czf")
        .arg(out.join(format!("{name}.tgz")))
        .arg("-C")
        .arg(&staged)
        .arg("package")
        .output()
        .unwrap();
    assert_eq!(packed.status.code(), Some(0), "{}", stderr(&packed));
    let output = smoke(&out);
    assert_ne!(output.status.code(), Some(0), "{}", stdout(&output));
}
