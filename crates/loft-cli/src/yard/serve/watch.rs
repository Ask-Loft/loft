//! What the page server looks at to learn of a change, in two tiers, so that what a look costs
//! grows with the trees it serves and their current branches' Yard Logs, not with every Yard Log
//! those trees hold. A quick look takes each tree's HEAD file, and the folder and the files of the
//! Yard Log of the branch the tree is on, where agents write. A sweep, less often, finds the Yard
//! Logs each tree holds and takes one stamp of the root file of every other one: each write to a
//! Yard Log rewrites its root, since the root holds the revision.

use std::collections::hash_map::DefaultHasher;
use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use yardlog::codec::ROOT;
use yardlog::codec::session::{ACTIVITY, TOKENS};
use yardlog_store::YARD;
use yardlog_store::git::YardGit;
use yardlog_store::home::Home;
use yardlog_store::session::branches;

use super::live::{Area, Digests};
use super::trees;

/// A file's size and modification time, as the file system keeps them.
type Stamp = Option<(u64, Option<SystemTime>)>;

/// What the page server knows of a tree's HEAD: where its file lies, asked of git once; the
/// stamp it last saw on it; and the branch it named then, asked of git only when the stamp moves,
/// so that a look runs no git command while the tree stays on its branch.
#[derive(Default)]
struct Head {
    path: Option<PathBuf>,
    stamp: Stamp,
    branch: Option<String>,
    asked: bool,
}

/// What the page server keeps between looks: each tree's HEAD, and the digests of the last sweep.
#[derive(Default)]
pub struct Watch {
    heads: HashMap<PathBuf, Head>,
    swept: Digests,
}

impl Watch {
    /// A digest of each area of what the page shows: a quick look always, and a sweep where
    /// `sweep` asks for one; between sweeps, the last sweep's digests stand for the Yard Logs it
    /// took.
    pub fn look(&mut self, home: &Home, sweep: bool) -> Digests {
        let served = trees::served(home).unwrap_or_default();
        let mut digests = Digests::new();
        let mut listed = DefaultHasher::new();
        let mut current = BTreeSet::new();
        for tree in &served {
            tree.hash(&mut listed);
            let head = self.head(tree);
            head.stamp.hash(&mut listed);
            if let Some(branch) = head.branch.clone() {
                let shown = tree.display().to_string();
                quick(tree, &shown, &branch, &mut digests);
                current.insert((shown, branch));
            }
        }
        digests.insert(Area::Trees, listed.finish());
        if sweep {
            self.swept = swept(&served, &current);
        }
        for (area, digest) in &self.swept {
            digests.entry(area.clone()).or_insert(*digest);
        }
        digests
    }

    /// The tree's HEAD as it stands, its branch asked of git again where its stamp moved.
    fn head(&mut self, tree: &Path) -> &Head {
        let head = self.heads.entry(tree.to_owned()).or_default();
        let git = YardGit::default();
        if head.path.is_none() && !head.asked {
            head.path = git.git_path(tree, "HEAD").ok();
        }
        let stamp = head.path.as_deref().and_then(stamp_at);
        if !head.asked || stamp != head.stamp {
            head.branch = git.branch(tree).ok().flatten();
            head.stamp = stamp;
            head.asked = true;
        }
        head
    }
}

/// The digests of the Yard Log of `branch`, the branch the tree is on: of its own files, and
/// apart, of its sessions' files. A folder not there yet has a digest too, which its first file
/// changes.
pub fn quick(tree: &Path, shown: &str, branch: &str, digests: &mut Digests) {
    let folder = tree.join(YARD).join(branch);
    let mut sessions = DefaultHasher::new();
    files(&folder.join(TOKENS), &mut sessions);
    files(&folder.join(ACTIVITY), &mut sessions);
    let mut log = DefaultHasher::new();
    files(&folder, &mut log);
    let (tree, branch) = (shown.to_owned(), branch.to_owned());
    digests.insert(
        Area::Log {
            tree: tree.clone(),
            branch: branch.clone(),
        },
        log.finish(),
    );
    digests.insert(Area::Sessions { tree, branch }, sessions.finish());
}

/// The digest of every Yard Log each of `served` holds, but the ones `current` names: the stamp
/// of its root file, and of no other file of it.
pub fn swept(served: &[PathBuf], current: &BTreeSet<(String, String)>) -> Digests {
    let mut digests = Digests::new();
    for tree in served {
        let shown = tree.display().to_string();
        for branch in branches(tree).unwrap_or_default() {
            let key = (shown.clone(), branch);
            if current.contains(&key) {
                continue;
            }
            let mut root = DefaultHasher::new();
            stamp_at(&tree.join(YARD).join(&key.1).join(ROOT)).hash(&mut root);
            let (tree, branch) = key;
            digests.insert(Area::Log { tree, branch }, root.finish());
        }
    }
    digests
}

/// The size and modification time of the file at `path`, where there is one.
fn stamp_at(path: &Path) -> Stamp {
    fs::metadata(path).ok().map(|meta| stamp_of(&meta))
}

/// The file's own stamp, as the file system keeps it: not a reading of the clock.
fn stamp_of(meta: &fs::Metadata) -> (u64, Option<SystemTime>) {
    (meta.len(), meta.modified().ok())
}

/// Hashes the name, size and modification time of each file right in `dir`.
fn files(dir: &Path, hasher: &mut DefaultHasher) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(Result::ok).collect();
    entries.sort_by_key(fs::DirEntry::file_name);
    for entry in entries {
        let Ok(meta) = entry.metadata() else {
            continue;
        };
        if meta.is_file() {
            entry.file_name().hash(hasher);
            stamp_of(&meta).hash(hasher);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;
    use std::fs;
    use std::path::PathBuf;

    use yardlog::codec::ROOT;

    use super::{Area, Digests, quick, swept};

    /// A scratch tree whose `.yard/` holds a Yard Log of `main` and one of `feat/shelf`, each with
    /// a root and a block file; removed when dropped.
    struct Tree(PathBuf);

    impl Tree {
        fn new(name: &str) -> Self {
            let dir =
                std::env::temp_dir().join(format!("loft-watch-{name}-{}", std::process::id()));
            let _ = fs::remove_dir_all(&dir);
            for branch in ["main", "feat/shelf"] {
                let folder = dir.join(".yard").join(branch);
                fs::create_dir_all(&folder).unwrap();
                fs::write(folder.join(ROOT), "revision: 1\n").unwrap();
                fs::write(folder.join("block.yaml"), "a: 1\n").unwrap();
            }
            Self(dir)
        }

        fn file(&self, branch: &str, name: &str) -> PathBuf {
            self.0.join(".yard").join(branch).join(name)
        }

        fn shown(&self) -> String {
            self.0.display().to_string()
        }

        fn area(&self, branch: &str) -> Area {
            Area::Log {
                tree: self.shown(),
                branch: branch.to_owned(),
            }
        }
    }

    impl Drop for Tree {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn quick_of(tree: &Tree) -> Digests {
        let mut digests = Digests::new();
        quick(&tree.0, &tree.shown(), "feat/shelf", &mut digests);
        digests
    }

    fn swept_of(tree: &Tree) -> Digests {
        let current = BTreeSet::from([(tree.shown(), "feat/shelf".to_owned())]);
        swept(std::slice::from_ref(&tree.0), &current)
    }

    #[test]
    fn a_quick_look_takes_the_current_branch_s_yard_log_and_no_other() {
        let tree = Tree::new("quick");
        let before = quick_of(&tree);
        assert!(!before.contains_key(&tree.area("main")));
        fs::write(tree.file("main", ROOT), "revision: 22\n").unwrap();
        fs::write(tree.file("main", "block.yaml"), "a: 22\n").unwrap();
        assert_eq!(quick_of(&tree), before);
        fs::write(tree.file("feat/shelf", "block.yaml"), "a: 22\n").unwrap();
        assert_ne!(quick_of(&tree), before);
    }

    #[test]
    fn a_sweep_takes_only_the_root_of_each_other_yard_log() {
        let tree = Tree::new("sweep");
        let before = swept_of(&tree);
        assert_eq!(before.keys().collect::<Vec<_>>(), [&tree.area("main")]);
        fs::write(tree.file("main", "block.yaml"), "a: 22\n").unwrap();
        assert_eq!(swept_of(&tree), before);
        fs::write(tree.file("main", ROOT), "revision: 22\n").unwrap();
        assert_ne!(swept_of(&tree), before);
    }
}
