//! The outbox, `~/.yard/outbox/`: a transactional outbox of the versions not yet sent to Yard
//! Server. The write that seals a version puts an entry for it there, `<version key>.json`,
//! holding the address of the Yard Server it is for and the version's record of a batch of
//! versions, ready to be sent as it is. One outbox serves every tree and every Yard Server on the
//! machine; each is read for one address alone, oldest version first, with no git, no lock of
//! Yard Log and no worktree. A file that is not an entry is moved into `dead/` and never read
//! again; an entry of a newer format is left for a newer `loft`.

use std::collections::BTreeSet;
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

/// Where the files that are not entries are moved, inside the outbox.
pub const DEAD: &str = "dead";

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
            &self.dir.join(format!("{key}.json")),
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

    /// The oldest `most` entries for this outbox's address, in the order of their version
    /// keys, with the files that are not entries moved into `dead/` on the way. A file that
    /// cannot be read, or moved, is passed over, so that one file never holds up the rest.
    ///
    /// # Errors
    ///
    /// The outbox cannot be listed.
    pub fn take(&self, most: usize) -> Result<Taken, YardStoreError> {
        Ok(self.take_listed(self.names()?, most))
    }

    /// [`Self::take`] of the files a listing of the outbox named, some of which may be gone since.
    fn take_listed(&self, names: Vec<String>, most: usize) -> Taken {
        let mut taken = Taken::default();
        for name in names {
            let path = self.dir.join(&name);
            // Gone since the listing, sent or moved by another run, or not readable at all.
            let Ok(Some(bytes)) = read_if_present(&path) else {
                continue;
            };
            match entry::read(&bytes) {
                Read::Entry { endpoint, record } if endpoint == self.endpoint => {
                    taken.waiting += 1;
                    let room = taken.entries.len() < most;
                    taken.entries.extend(room.then(|| Entry {
                        key: name.trim_end_matches(".json").to_owned(),
                        record: record.to_owned(),
                    }));
                }
                Read::Entry { .. } => {}
                Read::Newer => taken.skipped += 1,
                Read::Foreign => taken.dead += usize::from(self.bury(&name).unwrap_or(false)),
            }
        }
        taken
    }

    /// Removes `entries` from the outbox; one already gone is no error.
    ///
    /// # Errors
    ///
    /// An entry is there and cannot be removed.
    pub fn remove(&self, entries: &[Entry]) -> Result<(), YardStoreError> {
        for taken in entries {
            remove_if_present(&self.dir.join(format!("{}.json", taken.key)))?;
        }
        Ok(())
    }

    /// The names of the files that may be entries, in order: those ending in `.json`, and not
    /// a temporary file, whose name starts with `.`.
    fn names(&self) -> Result<Vec<String>, YardStoreError> {
        let listing = match fs::read_dir(&self.dir) {
            Ok(listing) => listing,
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(io_error(&self.dir)(error).into()),
        };
        let mut names: Vec<String> = listing
            .filter_map(Result::ok)
            .filter(|entry| entry.file_type().is_ok_and(|kind| kind.is_file()))
            .filter_map(|entry| entry.file_name().into_string().ok())
            .filter(|name| {
                Path::new(name)
                    .extension()
                    .is_some_and(|extension| extension == "json")
                    && !name.starts_with('.')
            })
            .collect();
        names.sort();
        Ok(names)
    }

    /// Moves the file `name` into `dead/`: whether it was there to move.
    fn bury(&self, name: &str) -> Result<bool, YardStoreError> {
        let dead = self.dir.join(DEAD);
        fs::create_dir_all(&dead).map_err(io_error(&dead))?;
        let from = self.dir.join(name);
        match move_into_place(&from, &dead.join(name)) {
            Ok(()) => Ok(true),
            // Gone since it was read: moved or removed by another run.
            Err(_) if !from.exists() => Ok(false),
            Err(error) => Err(error.into()),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use yardlog::keys::VersionKey;

    use super::Outbox;

    #[test]
    fn an_entry_gone_since_the_listing_is_passed_over_and_the_batch_goes_on_without_it() {
        let dir = std::env::temp_dir().join(format!("loft-outbox-gone-{}", std::process::id()));
        let outbox = Outbox::new(&dir, "http://127.0.0.1:7420");
        let record = |id: &str| format!(r#"{{"id":"{id}"}}"#);
        let keys = [VersionKey::mint(), VersionKey::mint(), VersionKey::mint()];
        outbox.put(keys[0], &record("a")).unwrap();
        outbox.put(keys[2], &record("c")).unwrap();
        // The listing named the second entry too, which another run sent before it was read.
        let mut listed: Vec<String> = keys.iter().map(|key| format!("{key}.json")).collect();
        listed.sort();
        let mut expected: Vec<String> =
            [keys[0], keys[2]].iter().map(ToString::to_string).collect();
        expected.sort();
        let taken = outbox.take_listed(listed, 16);
        fs::remove_dir_all(&dir).unwrap();
        let sent: Vec<String> = taken.entries.into_iter().map(|entry| entry.key).collect();
        assert_eq!(sent, expected);
        assert_eq!((taken.waiting, taken.skipped, taken.dead), (2, 0, 0));
    }
}
