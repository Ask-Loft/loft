//! What `OpenSpec`'s validator sees in one rendered requirement: its requirement text, and
//! whether a level-4 heading has text under it. The renderer reads its own output, finding
//! fenced code and headings as the converter does, so a body that holds a heading or a fence is
//! read as the validator reads it.

use crate::convert::openspec::read::{fence_mask, heading_level};
use crate::convert::openspec::statement::{first_modal, metadata_entry};

/// What the validator reads in the lines after a requirement's heading.
pub(super) struct Read {
    /// The requirement text: the lines before the first level-4 heading, other than blank
    /// lines, lines in fenced code and metadata lines, each trimmed, joined by line breaks.
    pub text: String,
    /// Whether some level-4 heading has a line that is not blank before the next heading.
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
    let mut text = Vec::new();
    let (mut before_scenarios, mut under_level_four, mut scenario) = (true, false, false);
    for (line, fenced) in lines.iter().zip(fence_mask(lines)) {
        if let Some(level) = heading_level(line).filter(|_| !fenced) {
            before_scenarios &= level != 4;
            under_level_four = level == 4;
            continue;
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        scenario |= under_level_four;
        if before_scenarios && !fenced && metadata_entry(trimmed).is_none() {
            text.push(trimmed);
        }
    }
    Read {
        text: text.join("\n"),
        scenario,
    }
}
