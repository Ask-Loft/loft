//! The publish recorded after the merge (the Record a publish after the merge Feature): a
//! branch's specification is published on the publish branch, after the branch is merged and its
//! Yard Log closed, so a publish is recorded on the base branch as the one block that closed Yard
//! Log takes: a `loft` block of the `publish` view in its last stage, after its close. It is
//! told apart from a publish the branch recorded itself by the branch it was recorded on.

use crate::act::{Act, Actor, Agent, Time};
use crate::body::{Body, Kind, Loft, View};
use crate::keys::{BlockKey, Handle, VersionKey};
use crate::log::{Block, Version, YardLog};

/// The id the block takes, or, where Yard Log holds a block of that id, the first free one of
/// it suffixed.
const ID: &str = "published";

/// Why a publish was not recorded.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Unpublished {
    #[error("Yard Log already holds a publish recorded after its close, block `{0}`")]
    Recorded(Handle),
    #[error("Yard Log holds no stage to record the publish in")]
    NoStage,
    #[error("a publish is recorded as a `loft` block of the `publish` view")]
    NotPublish,
}

impl YardLog {
    /// The publish recorded after the close, where one is: a `loft` block of the `publish` view
    /// recorded on a branch other than Yard Log's own.
    #[must_use]
    pub fn published_after_close(&self) -> Option<&Block> {
        self.blocks().find(|block| match &block.current().body {
            Body::Loft(loft) => {
                loft.view == View::Publish
                    && loft
                        .recorded
                        .branch
                        .as_ref()
                        .is_some_and(|branch| *branch != self.root.branch)
            }
            _ => false,
        })
    }

    /// Records `loft`, the `publish` view the agent computed on the base branch, as a new block at
    /// the end of Yard Log's last stage, leaving its close as it was. Gives the block's id and key
    /// and Yard Log's revision.
    ///
    /// # Errors
    ///
    /// Yard Log holds a publish recorded after its close already, or no stage; or `loft` is not
    /// of the `publish` view.
    pub fn record_published(
        &mut self,
        loft: Loft,
        agent: Agent,
        at: Time,
    ) -> Result<(Handle, BlockKey, u64), Unpublished> {
        if loft.view != View::Publish {
            return Err(Unpublished::NotPublish);
        }
        if let Some(block) = self.published_after_close() {
            return Err(Unpublished::Recorded(block.id.clone()));
        }
        let id = self.free_id();
        let revision = self.root.revision + 1;
        let file = self.stages.last_mut().ok_or(Unpublished::NoStage)?;
        let key = BlockKey::mint();
        file.blocks.push(Block {
            key,
            id: id.clone(),
            kind: Kind::Loft,
            number: None,
            versions: vec![Version {
                key: VersionKey::mint(),
                act: Act {
                    at,
                    revision,
                    actor: Actor::Agent(agent),
                },
                arrived: None,
                body: Body::Loft(Box::new(loft)),
            }],
            responses: Vec::new(),
        });
        self.root.revision = revision;
        Ok((id, key, revision))
    }

    /// [`ID`], or the first of it suffixed that no block of Yard Log holds.
    fn free_id(&self) -> Handle {
        let base = Handle::parse(ID).expect("the id is one");
        let taken = |id: &Handle| self.blocks().any(|block| block.id == *id);
        if !taken(&base) {
            return base;
        }
        // A Yard Log of n blocks leaves one of the first n + 1 suffixes free.
        let blocks = u32::try_from(self.blocks().count()).unwrap_or(u32::MAX);
        (2..=blocks.saturating_add(2))
            .map(|n| base.suffixed(n))
            .find(|id| !taken(id))
            .expect("some suffix is free")
    }
}
