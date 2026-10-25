//! Settling Yard Log after a merge, which git applies one file at a time: two records given one
//! id apart, the numbers of decisions written apart, and the marks of the records the merge
//! brought in. The next command that opens Yard Log settles it under its lock, as one Act.

use std::collections::BTreeMap;

use crate::act::{Act, Actor, Time};
use crate::keys::{BlockKey, Handle, Key};
use crate::log::{Arrival, Settling, YardLog};

impl YardLog {
    /// Settles Yard Log where a merge left it unsettled, and gives what the settling changed.
    pub fn settle(&mut self, at: Time, actor: Actor) -> Option<&Settling> {
        self.root.unsettled_since?;
        let act = self.act(at, actor);
        let renamed = self.rename();
        let renumbered = self.renumber();
        self.mark(act.revision);
        self.root.unsettled_since = None;
        self.root.settlings.push(Settling {
            act,
            renamed,
            renumbered,
        });
        self.root.settlings.last()
    }

    /// Each block in the order of its first version's Act, by key between two of one time.
    fn written_order(&self) -> Vec<(BlockKey, Handle)> {
        let mut blocks: Vec<((Time, Key), BlockKey, Handle)> = self
            .blocks()
            .map(|block| {
                let first = block.ordered()[0];
                ((first.act.at, block.key.key()), block.key, block.id.clone())
            })
            .collect();
        blocks.sort();
        blocks.into_iter().map(|(_, key, id)| (key, id)).collect()
    }

    /// Gives the later written of two records with one id the id with the suffix `-2`, or the
    /// next number free.
    fn rename(&mut self) -> Vec<(BlockKey, Handle, Handle)> {
        let order = self.written_order();
        let mut taken: Vec<Handle> = Vec::new();
        let mut renamed = Vec::new();
        for (key, id) in order {
            if !taken.contains(&id) {
                taken.push(id);
                continue;
            }
            let new = self.free(&id, &taken);
            taken.push(new.clone());
            self.block_mut(key).expect("held").id = new.clone();
            let entries = self.root.stages.iter_mut();
            for entry in entries.filter(|entry| entry.key.block() == key) {
                entry.id = new.clone();
            }
            renamed.push((key, id, new));
        }
        renamed
    }

    /// The first of `id` with the suffix `-2`, `-3` and so on that no block holds or takes.
    fn free(&self, id: &Handle, taken: &[Handle]) -> Handle {
        let mut n = 2;
        loop {
            let candidate = id.suffixed(n);
            if !taken.contains(&candidate) && self.find(&candidate).is_none() {
                return candidate;
            }
            n += 1;
        }
    }

    /// Numbers the decisions again from 1 in the order they were first written.
    fn renumber(&mut self) -> Vec<(BlockKey, u32, u32)> {
        let decisions: Vec<BlockKey> = self
            .written_order()
            .into_iter()
            .map(|(key, _)| key)
            .filter(|key| {
                self.block(*key)
                    .is_some_and(|(_, block)| block.number.is_some())
            })
            .collect();
        let mut renumbered = Vec::new();
        let mut numbers = BTreeMap::new();
        for (number, key) in (1..).zip(decisions) {
            numbers.insert(key, number);
        }
        for (key, number) in numbers {
            let block = self.block_mut(key).expect("held");
            let old = block.number.replace(number).expect("a decision");
            if old != number {
                renumbered.push((key, old, number));
            }
        }
        renumbered
    }

    /// Marks each record the merge brought in as arrived at `revision`.
    fn mark(&mut self, revision: u64) {
        let arriving = std::mem::take(&mut self.root.arriving);
        let settle = |act: &Act, arrived: &mut Option<Arrival>| {
            if *arrived == Some(Arrival::Pending) || arriving.contains(act) {
                *arrived = Some(Arrival::At(revision));
            }
        };
        for file in &mut self.stages {
            for block in std::iter::once(&mut file.stage).chain(&mut file.blocks) {
                block
                    .versions
                    .iter_mut()
                    .for_each(|v| settle(&v.act, &mut v.arrived));
                block
                    .responses
                    .iter_mut()
                    .for_each(|r| settle(&r.act, &mut r.arrived));
            }
        }
        for submission in &mut self.root.submissions {
            submission
                .changes
                .iter_mut()
                .for_each(|c| settle(&c.act, &mut c.arrived));
        }
    }
}
