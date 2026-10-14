//! The npm packages as the tests build them: `scripts/npm-packages.sh` packs them into tarballs
//! from a binary for each platform, here the `loft` under test copied for every platform, or,
//! where a test runs only this machine's, a stand-in for each other one; a project installs them
//! as npm does, and Node runs their launcher.

use std::fs;
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use super::{PLATFORMS, Scratch, isolate, stderr, stdout, workspace};
use serde_json::Value;

/// The tarballs packed in `scratch/out` from the binary under test, each copied in without its
/// execute bits, as a download of the build's artifacts leaves it.
pub fn assembled(scratch: &Scratch) -> PathBuf {
    pack(scratch, |_| {
        Path::new(env!("CARGO_BIN_EXE_loft")).to_owned()
    })
}

/// [`assembled`], with `binary` for this machine's platform and a stand-in that never runs for
/// every other one, so that a test that runs only this machine's packs a single real binary.
pub fn assembled_with(scratch: &Scratch, binary: &Path) -> PathBuf {
    let stand_in = scratch.0.join("stand-in");
    fs::write(&stand_in, "a binary for another platform\n").unwrap();
    let host = host();
    pack(scratch, |platform| {
        if platform == host {
            binary.to_owned()
        } else {
            stand_in.clone()
        }
    })
}

/// The tarballs packed in `scratch/out` from the file `binary` gives for each platform, each
/// copied in without its execute bits.
pub fn pack(scratch: &Scratch, binary: impl Fn(&str) -> PathBuf) -> PathBuf {
    for platform in PLATFORMS {
        let dir = scratch.0.join("binaries").join(platform);
        fs::create_dir_all(&dir).unwrap();
        let name = if platform.starts_with("win32") {
            "loft.exe"
        } else {
            "loft"
        };
        let copy = dir.join(name);
        fs::copy(binary(platform), &copy).unwrap();
        fs::set_permissions(&copy, fs::Permissions::from_mode(0o644)).unwrap();
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
pub fn unpacked(scratch: &Scratch, out: &Path, name: &str) -> PathBuf {
    unpack(out, name, &scratch.0.join("unpacked").join(name))
}

pub fn manifest(dir: &Path) -> Value {
    serde_json::from_str(&fs::read_to_string(dir.join("package.json")).unwrap()).unwrap()
}

/// Node's name for this machine's platform and architecture.
pub fn host() -> String {
    let output = Command::new("node")
        .args(["-p", "process.platform + '-' + process.arch"])
        .output()
        .expect("node runs the launcher's tests");
    stdout(&output).trim().to_owned()
}

/// A project whose `node_modules` holds `@ask-loft/loft` and, with `platform`, this machine's
/// platform package, extracted from their tarballs as npm installs them.
pub fn project(scratch: &Scratch, platform: bool) -> PathBuf {
    project_from(scratch, &assembled(scratch), platform)
}

/// [`project`], from the tarballs in `out`.
pub fn project_from(scratch: &Scratch, out: &Path, platform: bool) -> PathBuf {
    let scope = scratch.0.join("project/node_modules/@ask-loft");
    unpack(out, "loft", &scope.join("loft"));
    if platform {
        let name = format!("loft-{}", host());
        unpack(out, &name, &scope.join(&name));
    }
    scratch.0.join("project")
}

/// The launcher installed in `project`, run there by Node for a user whose home is `home`, with
/// Node first loading `preload` if given.
pub fn launcher(project: &Path, home: &Path, preload: Option<&str>) -> Command {
    let mut node = Command::new("node");
    isolate(&mut node, home);
    if let Some(script) = preload {
        let path = project.join("preload.js");
        fs::write(&path, script).unwrap();
        node.arg("--require").arg(path);
    }
    node.arg(project.join("node_modules/@ask-loft/loft/bin/loft.js"))
        .current_dir(project);
    node
}

/// The launcher run with `args` in `project` for a user whose home is `home`, with Node first
/// loading `preload` if given.
pub fn launch(project: &Path, home: &Path, preload: Option<&str>, args: &[&str]) -> Output {
    launcher(project, home, preload)
        .args(args)
        .output()
        .unwrap()
}

/// `scripts/npm-smoke.sh` run over the tarballs in `out`.
pub fn smoke(out: &Path) -> Output {
    Command::new(workspace().join("scripts/npm-smoke.sh"))
        .arg(out)
        .output()
        .unwrap()
}
