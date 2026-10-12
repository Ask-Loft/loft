//! What the store tests share: a scratch directory and a snapshot of every file under it.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::{env, fs, process};

/// A directory removed when the test ends.
pub struct Scratch(pub PathBuf);

impl Scratch {
    /// A fresh, empty directory.
    pub fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let number = NEXT.fetch_add(1, Ordering::Relaxed);
        let root = env::temp_dir().join(format!("loftspec-store-{}-{number}", process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        Self(root)
    }

    /// Writes a file under the scratch directory, creating its parents.
    pub fn put(&self, path: &str, text: &str) {
        let path = self.0.join(path);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    pub fn snapshot(&self) -> BTreeMap<String, Vec<u8>> {
        snapshot(&self.0)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

/// Every file under `root` by its path from `root`, and every directory as an entry ending in
/// `/` with no bytes, so a directory left behind shows.
pub fn snapshot(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut entries = BTreeMap::new();
    let mut pending = vec![root.to_owned()];
    while let Some(dir) = pending.pop() {
        for entry in fs::read_dir(&dir).unwrap() {
            let path = entry.unwrap().path();
            let relative = path
                .strip_prefix(root)
                .unwrap()
                .to_string_lossy()
                .into_owned();
            if path.is_dir() {
                entries.insert(format!("{relative}/"), Vec::new());
                pending.push(path);
            } else {
                entries.insert(relative, fs::read(&path).unwrap());
            }
        }
    }
    entries
}
