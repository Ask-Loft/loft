//! A scratch repository under real git, kept from the user's configuration, holding the
//! Reading List with `loft merge-driver` registered for `spec/`: what the suites that hold Loft
//! to git's own contract share.

use std::fs;
use std::process::Output;

use loftspec_store::merge_driver::{ATTRIBUTE, ATTRIBUTES, driver};
use software_plant::real_git::setup_git;

use super::common::{Scratch, stderr};

/// A scratch repository holding the Reading List's tree, committed on `main`, with the driver
/// registered for `spec/` as `loft merge-driver --install` registers it.
pub struct Repo(pub Scratch);

impl Repo {
    pub fn new() -> Self {
        let scratch = Scratch::with_reading_list();
        fs::write(scratch.0.join(ATTRIBUTES), format!("{ATTRIBUTE}\n")).unwrap();
        let repo = Self(scratch);
        repo.git_ok(&["init", "--quiet", "-b", "main"]);
        repo.commit("base");
        repo
    }

    /// Runs real git in the repository, the driver registered, automatic maintenance off and an
    /// editor that keeps the message git proposes.
    pub fn git(&self, args: &[&str]) -> Output {
        // The command `loft merge-driver --install --command '<loft>'` registers.
        let launcher = format!("'{}'", env!("CARGO_BIN_EXE_loft"));
        let driver = format!("merge.loft.driver={}", driver(&launcher));
        setup_git()
            .args(["-c", "core.editor=true", "-c", &driver])
            .arg("-C")
            .arg(&self.0.0)
            .args(args)
            .output()
            .unwrap()
    }

    pub fn git_ok(&self, args: &[&str]) -> Output {
        let output = self.git(args);
        assert!(output.status.success(), "git {args:?}: {}", stderr(&output));
        output
    }

    pub fn commit(&self, message: &str) {
        self.git_ok(&["add", "-A"]);
        self.git_ok(&["commit", "--quiet", "-m", message]);
    }

    pub fn text(&self, path: &str) -> String {
        fs::read_to_string(self.0.file(path)).unwrap()
    }
}
