//! The session files that more than one post may write at once. Each text is numbered as it is
//! made, in the order the sessions' events were kept, and is written only over an older one, so a
//! post that writes last never leaves an older text in place of a newer.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

/// The newest text written of each file.
#[derive(Default)]
pub struct Newest {
    next: AtomicU64,
    files: Mutex<HashMap<PathBuf, Arc<Mutex<u64>>>>,
}

impl Newest {
    /// The number of a text made now; taken where the events are kept, so that numbers follow
    /// the order in which they were.
    pub fn generation(&self) -> u64 {
        self.next.fetch_add(1, Ordering::SeqCst) + 1
    }

    /// Writes the text numbered `generation` to `path` with `write`, unless a newer one was
    /// written there; one write at a time to each file. Gives whether it wrote.
    pub fn write<E>(
        &self,
        path: &Path,
        generation: u64,
        write: impl FnOnce() -> Result<(), E>,
    ) -> Result<bool, E> {
        let file = {
            let mut files = self.files.lock().unwrap_or_else(PoisonError::into_inner);
            Arc::clone(files.entry(path.to_owned()).or_default())
        };
        let mut written = file.lock().unwrap_or_else(PoisonError::into_inner);
        if *written > generation {
            return Ok(false);
        }
        write()?;
        *written = generation;
        Ok(true)
    }
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use super::Newest;

    const TOKENS: &str = "/work/.yard/feat/x/tokens/s1.yaml";

    fn writes(newest: &Newest, path: &str, generation: u64, written: &mut Vec<u64>) {
        let wrote = newest.write(Path::new(path), generation, || {
            written.push(generation);
            Ok::<_, ()>(())
        });
        assert!(wrote.is_ok());
    }

    #[test]
    fn a_text_older_than_the_one_written_is_not_written_over_it() {
        let newest = Newest::default();
        let (older, newer) = (newest.generation(), newest.generation());
        let mut written = Vec::new();
        writes(&newest, TOKENS, newer, &mut written);
        writes(&newest, TOKENS, older, &mut written);
        assert_eq!(written, [newer]);
    }

    #[test]
    fn each_file_is_written_by_its_own_texts_alone() {
        let newest = Newest::default();
        let (older, newer) = (newest.generation(), newest.generation());
        let mut written = Vec::new();
        writes(&newest, TOKENS, newer, &mut written);
        writes(
            &newest,
            "/work/.yard/feat/x/activity/s1.yaml",
            older,
            &mut written,
        );
        assert_eq!(written, [newer, older]);
    }

    #[test]
    fn a_text_whose_write_failed_is_not_taken_for_written() {
        let newest = Newest::default();
        let (older, newer) = (newest.generation(), newest.generation());
        let failed = newest.write(Path::new(TOKENS), newer, || Err(()));
        assert!(failed.is_err());
        let mut written = Vec::new();
        writes(&newest, TOKENS, older, &mut written);
        assert_eq!(written, [older]);
    }
}
