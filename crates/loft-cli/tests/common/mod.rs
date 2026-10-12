//! What the command-line tests share: a scratch repository holding the Reading List's `spec/`
//! tree and the `loft` binary run in it, in front of the Software Plant's simulators: its git
//! simulator, clean on `main` unless a test scripts another repository, and a fixed clock.

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

    /// Runs `loft` in a clean repository on `main` at [`EPOCH`].
    pub fn loft(&self, args: &[&str]) -> Output {
        self.loft_with(args, &GitScenario::clean_on("main"), &Clock::fixed(EPOCH))
    }

    /// Runs `loft` with the git simulator answering from `git` and the time `clock` gives.
    pub fn loft_with(&self, args: &[&str], git: &GitScenario, clock: &Clock) -> Output {
        let plant = self.plant();
        fs::create_dir_all(&plant).unwrap();
        let mut command = Command::new(env!("CARGO_BIN_EXE_loft"));
        git.install(&plant).apply(&mut command);
        clock.apply(&mut command);
        command.args(args).current_dir(&self.0).output().unwrap()
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
