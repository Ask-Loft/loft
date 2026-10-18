//! The tree at a base commit, for `loft status --base <revision>`: the commit where HEAD left
//! the revision, as `git merge-base` gives it, and the files under `spec/` that the commit
//! holds, listed by `git ls-tree` and read in one `git cat-file --batch`.

use std::io::Write;
use std::path::Path;
use std::process::{Output, Stdio};
use std::{io, thread};

use loftspec::codec::{Files, SpecPath};

use super::{GitCommand, succeeded, text};
use crate::StoreError;
use crate::fs::{SPEC, hidden};

/// The commit where HEAD left a revision, completed by the revision.
pub const MERGE_BASE: [&str; 2] = ["merge-base", "HEAD"];
/// The `-z` records of a commit's files, completed by the commit, `--` and `spec`.
pub const LIST_TREE: [&str; 3] = ["ls-tree", "-r", "-z"];
/// Each object whose id is a line of standard input, after a header of its id, type and size.
pub const BATCH: [&str; 2] = ["cat-file", "--batch"];

/// What `git merge-base HEAD <revision>` says of a revision that names a commit.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MergeBase {
    /// The commit where HEAD left the revision.
    Commit(String),
    /// HEAD and the revision share no history.
    Unrelated,
}

impl GitCommand {
    /// The commit where HEAD left `revision`.
    ///
    /// # Errors
    ///
    /// `revision` names no commit, or git fails or cannot be run.
    pub fn merge_base(&self, root: &Path, revision: &str) -> Result<MergeBase, StoreError> {
        if revision.is_empty() || revision.starts_with('-') {
            return Err(StoreError::UnknownRevision(revision.to_owned()));
        }
        let call: Vec<&str> = MERGE_BASE.into_iter().chain([revision]).collect();
        let output = self.run(root, &call)?;
        match output.status.code() {
            Some(1) => return Ok(MergeBase::Unrelated),
            Some(128)
                if String::from_utf8_lossy(&output.stderr).contains("Not a valid object name") =>
            {
                return Err(StoreError::UnknownRevision(revision.to_owned()));
            }
            _ => {}
        }
        Ok(MergeBase::Commit(text(&succeeded(&call, output)?)))
    }

    /// The files under `spec/` that `commit` holds, by their paths under `spec/`, leaving out a
    /// path with a component that begins with `.`, as the reader does.
    ///
    /// # Errors
    ///
    /// git fails or cannot be run, or answers in a shape the port does not read.
    pub fn spec_files_at(&self, root: &Path, commit: &str) -> Result<Files, StoreError> {
        let call: Vec<&str> = LIST_TREE.into_iter().chain([commit, "--", SPEC]).collect();
        let listed = succeeded(&call, self.run(root, &call)?)?;
        let blobs = blobs(&listed, &call)?;
        let mut input = String::new();
        for (object, _) in &blobs {
            input.push_str(object);
            input.push('\n');
        }
        let call = BATCH;
        let output = self.run_with_input(root, &call, input.as_bytes())?;
        let answer = succeeded(&call, output)?;
        let mut files = Files::new();
        let mut at = 0;
        for (object, path) in blobs {
            let (bytes, next) = object_at(&answer, at, &object).ok_or_else(|| StoreError::Git {
                call: call.join(" "),
                message: format!("no object {object} for {path}"),
            })?;
            files.insert(SpecPath::new(&path), bytes.to_vec());
            at = next;
        }
        Ok(files)
    }

    /// Runs one call with `input` on its standard input, written from another thread so that a
    /// call that answers while it reads cannot stall on a full pipe.
    fn run_with_input(
        &self,
        root: &Path,
        call: &[&str],
        input: &[u8],
    ) -> Result<Output, StoreError> {
        let failed = |error: io::Error| match error.kind() {
            io::ErrorKind::NotFound => StoreError::GitUnavailable(error.to_string()),
            _ => StoreError::Git {
                call: call.join(" "),
                message: error.to_string(),
            },
        };
        let mut child = (self.command)()
            .arg("-C")
            .arg(root)
            .args(call)
            .env("GIT_OPTIONAL_LOCKS", "0")
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(failed)?;
        let mut stdin = child.stdin.take().expect("the input is piped");
        thread::scope(|scope| {
            scope.spawn(move || stdin.write_all(input));
            child.wait_with_output()
        })
        .map_err(failed)
    }
}

/// The object id and the path under `spec/` of each blob of `ls-tree -z` records, whose paths
/// run from the working directory.
fn blobs(listed: &[u8], call: &[&str]) -> Result<Vec<(String, String)>, StoreError> {
    let unread = |record: &str| StoreError::Git {
        call: call.join(" "),
        message: format!("a record the port does not read: {record:?}"),
    };
    let mut blobs = Vec::new();
    for record in listed
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let record = String::from_utf8_lossy(record);
        let (header, path) = record.split_once('\t').ok_or_else(|| unread(&record))?;
        let mut fields = header.split(' ');
        let (Some(_mode), Some(kind), Some(object)) = (fields.next(), fields.next(), fields.next())
        else {
            return Err(unread(&record));
        };
        let Some(path) = path
            .strip_prefix(SPEC)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            return Err(unread(&record));
        };
        if kind != "blob" || path.split('/').any(hidden) {
            continue;
        }
        blobs.push((object.to_owned(), path.to_owned()));
    }
    Ok(blobs)
}

/// The bytes of the object of a `cat-file --batch` answer that starts at `at`, when it is
/// `object`, and where the next one starts.
fn object_at<'a>(answer: &'a [u8], at: usize, object: &str) -> Option<(&'a [u8], usize)> {
    let rest = answer.get(at..)?;
    let end = rest.iter().position(|byte| *byte == b'\n')?;
    let header = String::from_utf8_lossy(&rest[..end]);
    let mut fields = header.split(' ');
    let (Some(id), Some(_kind), Some(size)) = (fields.next(), fields.next(), fields.next()) else {
        return None;
    };
    let size: usize = size.parse().ok()?;
    let start = end + 1;
    if id != object || rest.get(start + size) != Some(&b'\n') {
        return None;
    }
    Some((&rest[start..start + size], at + start + size + 1))
}
