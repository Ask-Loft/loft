//! What the command-line tests share: a scratch repository holding the Reading List's `spec/`
//! tree and the `loft` binary run in it, in front of the Software Plant's simulators: a fixed
//! clock, and the git simulator for a command that reads the repository, with no `git` on the
//! search path otherwise.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;
use software_plant::{Clock, GitScenario};

/// The time every run reads unless a test gives another: 2026-01-01T00:00:00Z.
pub const EPOCH: u64 = 1_767_225_600;

const READING_LIST: &str =
    include_str!("../../../../testdata/v1/reading-list/loft-authored-state.json");

/// A directory removed when the test ends.
pub struct Scratch(pub PathBuf);

impl Scratch {
    /// A fresh directory holding the Reading List's `spec/` tree.
    pub fn with_reading_list() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("loft-cli-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        let document = convert(&Export::from_json(READING_LIST).unwrap())
            .unwrap()
            .document;
        fs::create_dir_all(root.join("spec")).unwrap();
        loftspec_store::FileStore::open(&root)
            .unwrap()
            .write_files(codec::write(&document))
            .unwrap();
        Self(root)
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

    /// The `loft` command in the scratch tree: the git simulator answering from `git` first on
    /// its search path, or no `git` at all, and the time `clock` gives.
    pub fn command(&self, git: Option<&GitScenario>, clock: &Clock) -> Command {
        let plant = self.plant();
        fs::create_dir_all(&plant).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        match git {
            Some(git) => {
                git.install(&plant).apply(&mut command);
            }
            None => {
                command.env("PATH", &plant);
            }
        }
        clock.apply(&mut command);
        command.current_dir(&self.0);
        command
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
