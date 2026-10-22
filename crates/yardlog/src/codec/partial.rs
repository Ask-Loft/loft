//! Yard Log of a format newer than this `loft` reads, read for what it can: the fields this
//! `loft` does not know are passed over, and each stage, block or submission that does not read
//! in its terms is left out and counted. What is read is for showing only; it is never written.

use std::collections::BTreeSet;

use super::reader::{leave_out, leniently};
use super::{FileError, ROOT, parse, root, stage, stage_file};
use crate::body::Body;
use crate::keys::{Key, StageKey};
use crate::log::{StageFile, YardLog};

/// What a lenient read gave: Yard Log as far as this `loft` reads it, and the number of its
/// records that it left out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Partial {
    pub log: YardLog,
    pub left_out: usize,
}

impl YardLog {
    /// Reads Yard Log of any format for what this `loft` can read of it, from its root and the
    /// file of each stage the root lists, which `stage_bytes` gives by its name. A reference to
    /// a record left out is dropped with it.
    ///
    /// # Errors
    ///
    /// The root does not read even so.
    pub fn read_partial(
        root_bytes: &[u8],
        mut stage_bytes: impl FnMut(&str) -> Option<Vec<u8>>,
    ) -> Result<Partial, FileError> {
        let (read, left_out) = leniently(|| {
            let node = parse(ROOT, root_bytes)?;
            let mut root = root::read(&node).map_err(|error| FileError {
                file: ROOT.to_owned(),
                error,
            })?;
            let read: Vec<Option<StageFile>> = root
                .stages
                .iter()
                .map(|entry| stage_of(entry.key, &mut stage_bytes))
                .collect();
            let mut files = read.iter();
            root.stages
                .retain(|_| files.next().is_some_and(Option::is_some));
            for _ in read.iter().filter(|file| file.is_none()) {
                leave_out();
            }
            let stages = read.into_iter().flatten().collect();
            Ok(Self { root, stages })
        });
        let mut log = read?;
        log.drop_dangling();
        Ok(Partial { log, left_out })
    }

    /// Drops each reference to a block that Yard Log does not hold, and each submission left
    /// with no block.
    fn drop_dangling(&mut self) {
        let held: BTreeSet<Key> = self.blocks().map(|block| block.key.key()).collect();
        for submission in &mut self.root.submissions {
            submission.blocks.retain(|key| held.contains(&key.key()));
        }
        self.root
            .submissions
            .retain(|submission| !submission.blocks.is_empty());
        let versions = self
            .stages
            .iter_mut()
            .flat_map(|file| file.blocks.iter_mut())
            .flat_map(|block| block.versions.iter_mut());
        for version in versions {
            if let Body::Decision(decision) = &mut version.body {
                decision.question = decision
                    .question
                    .filter(|question| held.contains(&question.key()));
            }
        }
    }
}

/// The stage `key`'s file, where it is there and reads as that stage's.
fn stage_of(key: StageKey, bytes: &mut impl FnMut(&str) -> Option<Vec<u8>>) -> Option<StageFile> {
    let name = stage_file(key);
    let node = parse(&name, &bytes(&name)?).ok()?;
    stage::read(&node)
        .ok()
        .filter(|file| file.stage.key.stage() == key)
}
