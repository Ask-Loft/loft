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

/// The names of the packages `package` depends on, in each field that lists dependencies.
fn dependencies(package: &Value) -> Vec<String> {
    let fields = [
        "dependencies",
        "optionalDependencies",
        "peerDependencies",
        "bundleDependencies",
        "bundledDependencies",
    ];
    let mut names = Vec::new();
    for field in fields {
        match &package[field] {
            Value::Null | Value::Bool(_) => {}
            Value::Object(listed) => names.extend(listed.keys().cloned()),
            Value::Array(listed) => names.extend(
                listed
                    .iter()
                    .map(|name| name.as_str().expect(field).to_owned()),
            ),
            other => panic!("{field}: {other}"),
        }
    }
    names
}

/// The quoted text at the start of `rest`, with a template's leading `${name}` replaced by the
/// text the script's `const name` holds; a module named by anything but a quote is refused.
fn quoted(script: &str, rest: &str) -> String {
    let rest = rest.trim_start();
    let quote = rest.chars().next().unwrap_or(' ');
    assert!(
        ['"', '\'', '`'].contains(&quote),
        "a module named by an expression: {rest}"
    );
    let text = rest[1..].split(quote).next().unwrap().to_owned();
    let Some(name) = text
        .strip_prefix("${")
        .and_then(|after| after.split('}').next())
    else {
        return text;
    };
    let definition = format!("const {name} = ");
    let (_, value) = script
        .split_once(&definition)
        .unwrap_or_else(|| panic!("no {definition}in the script"));
    text.replacen(&format!("${{{name}}}"), &quoted(script, value), 1)
}

/// The module each `require(…)`, `require.resolve(…)` and `import` in `script` names.
fn required(script: &str) -> Vec<String> {
    let mut modules = Vec::new();
    for call in ["require(", "require.resolve(", "import("] {
        for (at, _) in script.match_indices(call) {
            modules.push(quoted(script, &script[at + call.len()..]));
        }
    }
    for line in script.lines().map(str::trim_start) {
        let rest = if line.starts_with("import ") || line.starts_with("} from ") {
            line.rsplit_once("from ")
                .map_or(line.strip_prefix("import "), |(_, rest)| Some(rest))
        } else {
            None
        };
        modules.extend(rest.map(|rest| quoted(script, rest)));
    }
    modules
}

/// Whether `module`, required by the script at `script` in the package at `root`, is a built-in
/// of Node, a file of the package itself or a package of the `@ask-loft` scope.
fn within_reach(module: &str, script: &Path, root: &Path) -> bool {
    if module.starts_with("./") || module.starts_with("../") {
        let root = fs::canonicalize(root).unwrap();
        return fs::canonicalize(script.parent().unwrap().join(module))
            .is_ok_and(|target| target.starts_with(root));
    }
    module.starts_with("node:") || module.starts_with("@ask-loft/")
}

/// Every JavaScript file under `dir`.
fn scripts(dir: &Path, found: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            scripts(&path, found);
        } else if path
            .extension()
            .is_some_and(|extension| ["js", "cjs", "mjs"].contains(&extension.to_str().unwrap()))
        {
            found.push(path);
        }
    }
}

#[test]
fn the_launcher_depends_on_no_package_outside_the_ask_loft_scope() {
    let scratch = Scratch::empty();
    let root = unpacked(&scratch, &assembled(&scratch), "loft");
    let package = manifest(&root);
    let names = dependencies(&package);
    assert!(
        !names.is_empty(),
        "the launcher lists its platform packages"
    );
    for name in names {
        assert!(name.starts_with("@ask-loft/"), "{name}");
    }
    let launcher = root.join(package["bin"]["loft"].as_str().unwrap());
    assert_eq!(
        fs::read(&launcher).unwrap(),
        fs::read(workspace().join("npm/loft/bin/loft.js")).unwrap(),
        "the packed launcher is npm/loft/bin/loft.js"
    );
    let mut found = Vec::new();
    scripts(&root, &mut found);
    assert!(found.contains(&launcher), "{found:?}");
    for script in found {
        let modules = required(&fs::read_to_string(&script).unwrap());
        assert!(
            !modules.is_empty(),
            "{}: requires nothing",
            script.display()
        );
        for module in modules {
            assert!(
                within_reach(&module, &script, &root),
                "{}: {module}",
                script.display()
            );
        }
    }
}
