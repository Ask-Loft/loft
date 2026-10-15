//! What `OpenSpec`'s validator sees in one rendered requirement: its requirement text, and
//! whether a level-4 heading has text under it. The renderer reads its own output as the
//! validator's requirement reader does, so a body that holds a heading or a fence is read the
//! same way.

use crate::convert::openspec::read::{fence_mask, heading_level};
use crate::convert::openspec::statement::{first_modal, metadata_entry};

/// What the validator reads in the lines after a requirement's heading.
pub(super) struct Read {
    /// The requirement text: the lines before the first heading outside fenced code, other than
    /// blank lines, lines in fenced code and metadata lines, each trimmed, joined by line
    /// breaks; the metadata lines when no other line is left.
    pub text: String,
    /// Whether some level-4 heading has a line that is not blank before the next heading of
    /// level 4 or above.
    pub scenario: bool,
}

impl Read {
    /// Whether the requirement text holds `SHALL` or `MUST` as a word.
    pub fn modal(&self) -> bool {
        first_modal(&self.text).is_some()
    }

    pub fn length(&self) -> usize {
        self.text.chars().count()
    }
}

pub(super) fn read(lines: &[String]) -> Read {
    let fenced = fence_mask(lines);
    let level = |index: usize| heading_level(&lines[index]).filter(|_| !fenced[index]);
    let (mut text, mut metadata) = (Vec::new(), Vec::new());
    for (index, line) in lines.iter().enumerate() {
        if fenced[index] {
            continue;
        }
        if level(index).is_some() {
            break;
        }
        let trimmed = line.trim();
        if metadata_entry(trimmed).is_some() {
            metadata.push(trimmed);
        } else if !trimmed.is_empty() {
            text.push(trimmed);
        }
    }
    let scenario = (0..lines.len())
        .filter(|&index| level(index) == Some(4))
        .any(|start| {
            (start + 1..lines.len())
                .take_while(|&index| level(index).is_none_or(|level| level > 4))
                .any(|index| !lines[index].trim().is_empty())
        });
    Read {
        text: if text.is_empty() { metadata } else { text }.join("\n"),
        scenario,
    }
}
