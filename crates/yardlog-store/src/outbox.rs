//! The outbox, `~/.yard/outbox/`: a transactional outbox of the versions not yet sent to Yard
//! Server. The write that seals a version puts an entry for it there, `<version key>.json`,
//! holding the address of the Yard Server it is for and the version's record of a batch of
//! versions, ready to be sent as it is. One outbox serves every tree and every Yard Server on the
//! machine; each is read for one address alone, oldest version first, with no git, no lock of
//! Yard Log and no worktree. A file that is not an entry is moved into `dead/` and never read
//! again, as is an entry Yard Server will never take, each beside a `.reason` file that says
//! why; an entry of a newer format is left for a newer `loft`.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::io::ErrorKind;
use std::path::{Path, PathBuf};

use loft_ports::io_error;
use loft_ports::sync::{move_into_place, read_if_present, remove_if_present, replace_synced};
use yardlog::YardLog;
use yardlog::entry::{self, Read};
use yardlog::keys::VersionKey;

use crate::YardStoreError;

pub use yardlog::entry::FORMAT;

/// Where the files that cannot be sent are moved, inside the outbox.
pub const DEAD: &str = "dead";

/// The extension of the file beside each file of `dead/` that holds why it was moved there.
pub const REASON: &str = "reason";

/// The reason of a file moved into `dead/` because it is not an entry.
pub const NOT_AN_ENTRY: &str = "not-an-entry";

/// What waits in the outbox, read without moving a file.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Survey {
    /// The version keys of the entries for each address, in their order.
    pub waiting: BTreeMap<String, Vec<String>>,
    /// The entries of a newer format, whatever their address.
    pub newer: usize,
}

/// The outbox of one Yard Server's address.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outbox {
    dir: PathBuf,
    endpoint: String,
}

/// An entry taken from the outbox: its version's key and its record, as the entry holds it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub key: String,
    pub record: String,
}

/// What a take found in the outbox for its address.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Taken {
    /// The oldest entries, at most as many as were asked for.
    pub entries: Vec<Entry>,
    /// Every entry waiting for the address, those taken included.
    pub waiting: usize,
    /// The entries of a newer format, left in place.
    pub skipped: usize,
    /// The files moved into `dead/`.
    pub dead: usize,
}

impl Outbox {
    /// The outbox in `dir` of the Yard Server at `endpoint`.
    #[must_use]
    pub fn new(dir: impl Into<PathBuf>, endpoint: &str) -> Self {
        Self {
            dir: dir.into(),
            endpoint: endpoint.to_owned(),
        }
    }

    #[must_use]
    pub fn dir(&self) -> &Path {
        &self.dir
    }

    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Puts an entry for `key`, holding `record`, through a temporary file renamed into place.
    ///
    /// # Errors
    ///
    /// The outbox or the entry cannot be written.
    pub fn put(&self, key: VersionKey, record: &str) -> Result<(), YardStoreError> {
        fs::create_dir_all(&self.dir).map_err(io_error(&self.dir))?;
        let text = entry::write(&self.endpoint, record);
        Ok(replace_synced(
            &self.dir.join(file_of(&key.to_string())),
            text.as_bytes(),
            false,
        )?)
    }

    /// Puts an entry for each version of `log` whose key `before` does not hold and that no
    /// merge brought in: how many it put, and a warning that names the outbox and each version
    /// left out of it, where any was.
    pub(crate) fn put_new(
        &self,
        before: &BTreeSet<VersionKey>,
        log: &YardLog,
    ) -> (usize, Option<String>) {
        let mut put = 0;
        let mut left_out = Vec::new();
        let new = log
            .versions()
            .filter(|(_, _, version)| !before.contains(&version.key) && version.arrived.is_none());
        for (stage, block, version) in new {
            let record = yardlog::batch::record(log.root.key, stage, block, version);
            match self.put(version.key, &record) {
                Ok(()) => put += 1,
                Err(error) => left_out.push(format!("{} ({error})", version.key)),
            }
        }
        let warning = (!left_out.is_empty()).then(|| {
            format!(
                "the outbox {} could not take version {}; it is saved in Yard Log and will not be sent to Yard Server",
                self.dir.display(),
                left_out.join(", ")
            )
        });
        (put, warning)
    }

    /// The oldest entries for this outbox's address, in the order of their version keys, at
    /// most `most` of them and at most `most_bytes` of entry files in all, and at least one: an
    /// entry that would bring them over `most_bytes` waits for the next take, unless it is the
    /// first. The files that are not entries are moved into `dead/` on the way. A file that
    /// cannot be read, or moved, is passed over, so that one file never holds up the rest.
    ///
    /// # Errors
    ///
    /// The outbox cannot be listed.
    pub fn take(&self, most: usize, most_bytes: u64) -> Result<Taken, YardStoreError> {
        self.take_after(most, most_bytes, &BTreeSet::new())
    }

    /// As [`Outbox::take`], passing over the entries whose keys `passed` holds, as a run passes
    /// over the entries it has taken once, whatever became of them: they are neither taken nor
    /// counted as waiting.
    ///
    /// # Errors
    ///
    /// The outbox cannot be listed.
    pub fn take_after(
        &self,
        most: usize,
        most_bytes: u64,
        passed: &BTreeSet<String>,
    ) -> Result<Taken, YardStoreError> {
        let mut taken = Taken::default();
        let mut room = Room {
            most,
            most_bytes,
            bytes: 0,
            full: false,
        };
        for (name, size) in listed(&self.dir)? {
            if passed.contains(key_of(&name)) {
                continue;
            }
            let path = self.dir.join(&name);
            // Gone since the listing, sent or moved by another run, or not readable at all.
            let Ok(Some(text)) = read_if_present(&path) else {
                continue;
            };
            match entry::read(&text) {
                Read::Entry { endpoint, record } if endpoint == self.endpoint => {
                    taken.waiting += 1;
                    room.take(&mut taken.entries, &name, record, size);
                }
                Read::Entry { .. } => {}
                Read::Newer => taken.skipped += 1,
                Read::Foreign => {
                    taken.dead += usize::from(self.bury_file(&name, NOT_AN_ENTRY).unwrap_or(false));
                }
            }
        }
        Ok(taken)
    }

    /// The path of `entry`'s file.
    #[must_use]
    pub fn path_of(&self, entry: &Entry) -> PathBuf {
        self.dir.join(file_of(&entry.key))
    }

    /// Moves `entry` into `dead/`, beside a `.reason` file that holds `reason`: whether it was
    /// there to move.
    ///
    /// # Errors
    ///
    /// The entry is there and cannot be moved, or its reason cannot be written.
    pub fn bury(&self, entry: &Entry, reason: &str) -> Result<bool, YardStoreError> {
        self.bury_file(&file_of(&entry.key), reason)
    }

    /// Removes `entries` from the outbox; one already gone is no error.
    ///
    /// # Errors
    ///
    /// An entry is there and cannot be removed.
    pub fn remove(&self, entries: &[Entry]) -> Result<(), YardStoreError> {
        for taken in entries {
            remove_if_present(&self.path_of(taken))?;
        }
        Ok(())
    }

    /// Moves the file `name` into `dead/`, writing its reason beside it first, so that a file
    /// in `dead/` without a reason is one whose move was cut short: whether it was there to move.
    fn bury_file(&self, name: &str, reason: &str) -> Result<bool, YardStoreError> {
        let dead = self.dir.join(DEAD);
        fs::create_dir_all(&dead).map_err(io_error(&dead))?;
        let from = self.dir.join(name);
        if !from.exists() {
            return Ok(false);
        }
        let reason_file = dead.join(format!("{name}.{REASON}"));
        replace_synced(&reason_file, format!("{reason}\n").as_bytes(), false)?;
        match move_into_place(&from, &dead.join(name)) {
            Ok(()) => Ok(true),
            // Gone since it was read, moved or removed by another run: its reason goes with it.
            Err(_) if !from.exists() => {
                remove_if_present(&reason_file)?;
                Ok(false)
            }
            Err(error) => Err(error.into()),
        }
    }
}

/// The room left in one take: once an entry does not fit, none after it does, so that a batch
/// keeps the order of the keys.
struct Room {
    most: usize,
    most_bytes: u64,
    bytes: u64,
    full: bool,
}

impl Room {
    /// Adds the entry `name`, of `size` bytes and holding `record`, to `entries` where it fits.
    fn take(&mut self, entries: &mut Vec<Entry>, name: &str, record: &str, size: u64) {
        let taken = entries.len();
        self.full =
            self.full || taken == self.most || (taken > 0 && self.bytes + size > self.most_bytes);
        if !self.full {
            self.bytes += size;
            entries.push(Entry {
                key: key_of(name).to_owned(),
                record: record.to_owned(),
            });
        }
    }
}

/// What waits in the outbox in `dir`, for every address, with no file moved or written.
///
/// # Errors
///
/// The outbox cannot be listed.
pub fn survey(dir: &Path) -> Result<Survey, YardStoreError> {
    let mut survey = Survey::default();
    for (name, _) in listed(dir)? {
        let Ok(Some(text)) = read_if_present(&dir.join(&name)) else {
            continue;
        };
        match entry::read(&text) {
            Read::Entry { endpoint, .. } => survey
                .waiting
                .entry(endpoint.clone())
                .or_default()
                .push(key_of(&name).to_owned()),
            Read::Newer => survey.newer += 1,
            Read::Foreign => {}
        }
    }
    Ok(survey)
}

/// A file of `dead/`, with the reason kept beside it, where one is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dead {
    pub file: String,
    pub reason: Option<String>,
}

/// The files of `dead/` in the outbox in `dir`, in the order of their names, each with the reason
/// kept beside it, where one is; a `.reason` file is not listed, nor one without its file.
///
/// # Errors
///
/// `dead/` is there and cannot be listed.
pub fn dead(dir: &Path) -> Result<Vec<Dead>, YardStoreError> {
    let dead = dir.join(DEAD);
    let suffix = format!(".{REASON}");
    Ok(files(&dead, |name| !name.ends_with(&suffix))?
        .into_iter()
        .map(|(file, _)| {
            let reason = fs::read_to_string(dead.join(format!("{file}{suffix}")))
                .ok()
                .and_then(|text| text.lines().next().map(str::to_owned))
                .filter(|reason| !reason.is_empty());
            Dead { file, reason }
        })
        .collect())
}

/// The version key an entry file is named by.
fn key_of(name: &str) -> &str {
    name.strip_suffix(".json").unwrap_or(name)
}

/// The name of the entry file of the version `key`.
fn file_of(key: &str) -> String {
    format!("{key}.json")
}

/// The files in `dir` that may be entries, in order, each with its size from the listing: those
/// ending in `.json`, and not a temporary file, whose name starts with `.`.
fn listed(dir: &Path) -> Result<Vec<(String, u64)>, YardStoreError> {
    files(dir, |name| {
        Path::new(name)
            .extension()
            .is_some_and(|extension| extension == "json")
    })
}

/// The files in `dir` whose names `keep` keeps, in the order of their names, each with its size
/// from the listing; a temporary file, whose name starts with `.`, never; none where `dir` is not
/// there.
fn files(dir: &Path, keep: impl Fn(&str) -> bool) -> Result<Vec<(String, u64)>, YardStoreError> {
    let listing = match fs::read_dir(dir) {
        Ok(listing) => listing,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(io_error(dir)(error).into()),
    };
    let mut names: Vec<(String, u64)> = listing
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let metadata = entry.metadata().ok().filter(fs::Metadata::is_file)?;
            let name = entry.file_name().into_string().ok()?;
            (keep(&name) && !name.starts_with('.')).then_some((name, metadata.len()))
        })
        .collect();
    names.sort();
    Ok(names)
}
