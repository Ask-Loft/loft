//! What the command-line tests share: a scratch repository holding the Reading List's `spec/`
//! tree and the `loft` binary run in it.

use std::path::PathBuf;
use std::process::{Command, Output};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

use loftspec::codec;
use loftspec::convert::v1::convert;
use loftspec::convert::v1::export::Export;

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
        loftspec_store::fs::write_changed(&root, &codec::write(&document)).unwrap();
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

    pub fn loft(&self, args: &[&str]) -> Output {
        Command::new(env!("CARGO_BIN_EXE_loft"))
            .args(args)
            .current_dir(&self.0)
            .output()
            .unwrap()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

pub fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

pub fn stderr(output: &Output) -> String {
    String::from_utf8(output.stderr.clone()).unwrap()
}
