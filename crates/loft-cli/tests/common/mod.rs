//! What the command-line tests share: a scratch repository holding the Reading List's `spec/`
//! tree and the `loft` binary run in it, in front of the Software Plant's simulators: a fixed
//! clock, and the git simulator for a command that reads the repository, with no `git` on the
//! search path otherwise; or, for a contract suite, under real git.

#[cfg(unix)]
pub mod npm;
pub mod telemetry;

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use software_plant::real_git;
use software_plant::{Clock, GitScenario, Installed};

/// The time every run reads unless a test gives another: 2026-01-01T00:00:00Z.
/// The files the Format places beside `spec/`.
pub const BESIDE: [&str; 4] = [
    loftspec_store::fs::SIDECAR,
    loftspec::convert::openspec::SIDECAR,
    loftspec::publish::LOCK,
    loftspec::trace::TRACE,
];

/// Every file under `<root>/spec/`, by its path under `spec/`.
pub fn tree(root: &Path) -> codec::Files {
    loftspec_store::FileStore::open(root)
        .unwrap()
        .read_files()
        .unwrap()
}

pub const EPOCH: u64 = 1_767_225_600;

const READING_LIST: &str =
    include_str!("../../../../testdata/v1/reading-list/loft-authored-state.json");

/// The platforms `loft` is released for, by Node's names.
pub const PLATFORMS: [&str; 6] = [
    "darwin-arm64",
    "darwin-x64",
    "linux-arm64",
    "linux-x64",
    "win32-arm64",
    "win32-x64",
];

/// The root of the repository.
pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Every Rust source file beneath `dir`.
pub fn sources(dir: &Path) -> Vec<PathBuf> {
    let mut found = Vec::new();
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            found.extend(sources(&path));
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            found.push(path);
        }
    }
    found
}

/// `path` as a guard reports it: from the root of the repository.
pub fn shown(path: &Path) -> String {
    path.strip_prefix(workspace())
        .unwrap()
        .display()
        .to_string()
}

/// The source of the `loft-spec` skill.
pub fn skill_source() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("skill/loft-spec")
}

/// A directory removed when the test ends.
pub struct Scratch(pub PathBuf);

impl Scratch {
    /// A fresh, empty directory.
    pub fn empty() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("loft-cli-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    /// A fresh directory holding the Reading List's `spec/` tree.
    pub fn with_reading_list() -> Self {
        let scratch = Self::empty();
        let document = convert(&Export::from_json(READING_LIST).unwrap())
            .unwrap()
            .document;
        for (path, bytes) in codec::write(&document) {
            let target = scratch.0.join("spec").join(path.as_str());
            fs::create_dir_all(target.parent().unwrap()).unwrap();
            fs::write(target, bytes).unwrap();
        }
        scratch
    }

    pub fn file(&self, path: &str) -> PathBuf {
        self.0.join("spec").join(path)
    }

    pub fn edit(&self, path: &str, from: &str, to: &str) {
        let text = fs::read_to_string(self.file(path)).unwrap();
        assert!(text.contains(from), "{from:?} in {path}");
        fs::write(self.file(path), text.replacen(from, to, 1)).unwrap();
    }

    /// Runs `loft` at [`EPOCH`] with no `git` on its search path, as every command but a
    /// publish needs.
    pub fn loft(&self, args: &[&str]) -> Output {
        self.loft_with(args, None, &Clock::fixed(EPOCH))
    }

    /// Runs `loft` with the git simulator answering from `git`, or no `git` at all, and the time
    /// `clock` gives.
    pub fn loft_with(&self, args: &[&str], git: Option<&GitScenario>, clock: &Clock) -> Output {
        self.command(git, clock).args(args).output().unwrap()
    }

    /// Runs `loft` at [`EPOCH`] with the git simulator answering from `git`, and gives its output
    /// and the scenario as the run left it.
    pub fn loft_in(&self, args: &[&str], git: &GitScenario) -> (Output, GitScenario) {
        let (mut command, installed) = self.installed_command(Some(git), &Clock::fixed(EPOCH));
        let output = command.args(args).output().unwrap();
        let scenario = installed.expect("a scenario was installed").scenario;
        (output, GitScenario::read(&scenario).unwrap())
    }

    /// Runs `loft` at [`EPOCH`] under real git, kept from the user's configuration, as a
    /// contract suite runs it.
    pub fn loft_under_real_git(&self, args: &[&str]) -> Output {
        let mut command = self.bare(&Clock::fixed(EPOCH));
        real_git::isolate(&mut command).args(args).output().unwrap()
    }

    /// The `loft` command in the scratch tree: the git simulator answering from `git` first on
    /// its search path, or no `git` at all, and the time `clock` gives.
    pub fn command(&self, git: Option<&GitScenario>, clock: &Clock) -> Command {
        self.installed_command(git, clock).0
    }

    /// [`Self::command`], with the simulator's scenario as installed, if `git` is given.
    fn installed_command(
        &self,
        git: Option<&GitScenario>,
        clock: &Clock,
    ) -> (Command, Option<Installed>) {
        let plant = self.plant();
        fs::create_dir_all(&plant).unwrap();
        let mut command = self.bare(clock);
        let installed = git.map(|git| git.install(&plant));
        match &installed {
            Some(installed) => {
                installed.apply(&mut command);
            }
            None => {
                command.env("PATH", &plant);
            }
        }
        (command, installed)
    }

    /// The `loft` command in the scratch tree at the time `clock` gives, its search path as
    /// inherited.
    fn bare(&self, clock: &Clock) -> Command {
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        clock.apply(&mut command);
        isolate(&mut command, &self.home());
        command.current_dir(&self.0);
        command
    }

    /// The home directory every run is given, beside the scratch tree, created on first use.
    pub fn home(&self) -> PathBuf {
        let home = self.beside(".home");
        fs::create_dir_all(&home).unwrap();
        home
    }

    /// The directory where `loft` keeps its settings under [`Self::home`].
    pub fn config(&self) -> PathBuf {
        config_under(&self.home())
    }

    /// The settings file `name` under [`Self::config`], or an empty object while there is none.
    pub fn setting(&self, name: &str) -> serde_json::Value {
        fs::read(self.config().join(name))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_else(|| serde_json::json!({}))
    }

    /// Records that `way` installed the `loft` under test, as `loft record-install` does.
    pub fn record_install(&self, way: &str) {
        let output = self
            .command(None, &Clock::fixed(EPOCH))
            .args(["record-install", way])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }

    /// `loft check` in the scratch tree at the time `clock` gives, run by `script` on a
    /// pseudo-terminal, so that its standard error is a terminal; what it writes comes out on
    /// the standard output of `script`. The two families of `script` take their arguments apart.
    #[cfg(unix)]
    pub fn check_on_terminal(&self, clock: &Clock) -> Command {
        let loft = env!("CARGO_BIN_EXE_loft");
        let mut command = Command::new("script");
        if cfg!(target_os = "linux") {
            command.args(["-q", "-e", "-c", &format!("'{loft}' check"), "/dev/null"]);
        } else {
            command.args(["-q", "/dev/null", loft, "check"]);
        }
        clock.apply(&mut command);
        isolate(&mut command, &self.home());
        command.current_dir(&self.0).stdin(process::Stdio::null());
        command
    }

    /// The `loft` command in the scratch tree at the time `clock` gives, run from the binary
    /// under test placed where an npm install puts it, inside `node_modules/@ask-loft/`, so that
    /// `loft` infers that npm installed it. The binary is linked there where the file system
    /// allows, and copied otherwise.
    pub fn npm_installed(&self, clock: &Clock) -> Command {
        let bin = self.0.join("node_modules/@ask-loft/loft-test/bin");
        let copy = bin.join("loft");
        if !copy.exists() {
            fs::create_dir_all(&bin).unwrap();
            if fs::hard_link(env!("CARGO_BIN_EXE_loft"), &copy).is_err() {
                fs::copy(env!("CARGO_BIN_EXE_loft"), &copy).unwrap();
            }
        }
        let mut command = Command::new(&copy);
        clock.apply(&mut command);
        isolate(&mut command, &self.home());
        command.current_dir(&self.0);
        command
    }

    /// The directory beside the scratch tree that holds the simulators' scenarios.
    fn plant(&self) -> PathBuf {
        self.beside(".plant")
    }

    /// The path beside the scratch tree whose name ends in `suffix`.
    fn beside(&self, suffix: &str) -> PathBuf {
        let mut path = self.0.clone().into_os_string();
        path.push(suffix);
        PathBuf::from(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
        let _ = fs::remove_dir_all(self.plant());
        let _ = fs::remove_dir_all(self.beside(".home"));
    }
}

/// The variables that would let a test run reach the user's own settings, or tell `loft` that
/// an agent or CI started it.
const OUTSIDE: [&str; 20] = [
    "ALL_PROXY",
    "all_proxy",
    "HTTPS_PROXY",
    "https_proxy",
    "HTTP_PROXY",
    "http_proxy",
    "NO_PROXY",
    "no_proxy",
    "XDG_CONFIG_HOME",
    "CARGO_HOME",
    "CI",
    "CLAUDECODE",
    "CLAUDE_CODE_ENTRYPOINT",
    "CLAUDE_CODE_IS_COWORK",
    "CODEX_THREAD_ID",
    "CODEX_SANDBOX",
    "CODEX_SANDBOX_NETWORK_DISABLED",
    "CODEX_CI",
    "AI_AGENT",
    "AGENT",
];

/// Gives a run `home` as its home directory and keeps it from the user's settings, from agent
/// and CI markers, from any repository git's environment names, and from the network:
/// telemetry and the update check are off, both services are a closed port on the loopback
/// interface, and no proxy is named. A test that exercises either switches it on and points it
/// at a simulator.
pub fn isolate(command: &mut Command, home: &Path) {
    for name in OUTSIDE {
        command.env_remove(name);
    }
    real_git::forget_repository(command);
    command
        .env("HOME", home)
        .env("USERPROFILE", home)
        .env("APPDATA", home.join("AppData/Roaming"))
        .env("LOFT_TELEMETRY", "off")
        .env("LOFT_UPDATE_CHECK", "off")
        .env("LOFT_TELEMETRY_ENDPOINT", CLOSED)
        .env("LOFT_UPDATE_ENDPOINT", CLOSED);
}

/// An address on the loopback interface where nothing listens.
pub const CLOSED: &str = "http://127.0.0.1:9";

/// The directory where `loft` keeps its settings for a user whose home is `home`, on the
/// platform the tests run on.
pub fn config_under(home: &Path) -> PathBuf {
    if cfg!(windows) {
        home.join("AppData/Roaming/loft")
    } else if cfg!(target_os = "macos") {
        home.join("Library/Application Support/loft")
    } else {
        home.join(".config/loft")
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}
