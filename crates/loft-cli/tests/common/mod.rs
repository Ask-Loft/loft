//! What the command-line tests share: a scratch repository holding the Reading List's `spec/`
//! tree and the `loft` binary run in it, in front of the Software Plant's simulators: a fixed
//! clock, and the git simulator for a command that reads the repository, with no `git` on the
//! search path otherwise.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use software_plant::{Clock, GitScenario, Installed};

/// The time every run reads unless a test gives another: 2026-01-01T00:00:00Z.
pub const EPOCH: u64 = 1_767_225_600;

const READING_LIST: &str =
    include_str!("../../../../testdata/v1/reading-list/loft-authored-state.json");

/// The root of the repository.
pub fn workspace() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
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
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        let installed = git.map(|git| git.install(&plant));
        match &installed {
            Some(installed) => {
                installed.apply(&mut command);
            }
            None => {
                command.env("PATH", &plant);
            }
        }
        clock.apply(&mut command);
        command.current_dir(&self.0);
        (command, installed)
    }

    /// The directory beside the scratch tree that holds the simulators' scenarios.
    fn plant(&self) -> PathBuf {
        let mut plant = self.0.clone().into_os_string();
        plant.push(".plant");
        PathBuf::from(plant)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
        let _ = fs::remove_dir_all(self.plant());
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}
