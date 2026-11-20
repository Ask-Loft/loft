//! `loft interface status`: compares each copy with the state its pin names, which confirms it
//! or names its first difference, and with its remote's head, where its Interface is current,
//! behind or withdrawn. It writes nothing; a mismatch exits 1.

use std::path::Path;

use loftspec::codec::{self, CopyError};
use loftspec::copy::Copy;
use loftspec::document::Document;
use serde_json::{Value, json};

use super::reach::Remotes;
use super::update::foreign_name;
use super::{Report, connected, document, entry_of};
use crate::stop::Stop;

/// One copy as its remote serves it: its pin confirmed or its first difference, and how the
/// head holds its Interface.
struct Checked<'a> {
    copy: &'a Copy,
    difference: Option<String>,
    head: u64,
    freshness: &'static str,
}

pub(super) fn run(root: &Path, browser: bool) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    connected(&document)?;
    let mut remotes = Remotes::new(browser);
    let mut checked = Vec::new();
    for copy in &document.copies {
        checked.push(check(&mut remotes, &document, copy)?);
    }
    Ok(report(&checked))
}

fn check<'a>(
    remotes: &mut Remotes,
    document: &Document,
    copy: &'a Copy,
) -> Result<Checked<'a>, Stop> {
    let entry = entry_of(document, copy);
    let pinned = copy.publish_sequence;
    let difference = match remotes.state(entry, pinned)? {
        None => Some(format!(
            "the remote holds no published state of publish sequence {pinned}"
        )),
        Some((files, lock)) => {
            let taken =
                codec::copy_of(&copy.handle, files, copy.keys()[0], pinned, lock.state_root);
            first_difference(copy, taken)
        }
    };
    let head = remotes.head(entry)?;
    let freshness = if head == pinned {
        "current"
    } else {
        match remotes.state(entry, head)? {
            None => return Err(super::reach::not_held(&entry.remote, head)),
            Some((files, lock)) => {
                match codec::copy_of(&copy.handle, files, copy.keys()[0], head, lock.state_root) {
                    Ok(at_head) if at_head.interface_hash == copy.interface_hash => "current",
                    Ok(_) => "behind",
                    Err(_) => "withdrawn",
                }
            }
        }
    };
    Ok(Checked {
        copy,
        difference,
        head,
        freshness,
    })
}

/// How `copy` differs from the copy `taken` from the state its pin names: none where it is that
/// copy, byte for byte.
fn first_difference(copy: &Copy, taken: Result<Copy, CopyError>) -> Option<String> {
    let served = match taken {
        Ok(served) => served,
        Err(CopyError::NotHeld(_)) => {
            return Some("the state its pin names does not hold its Interface".to_owned());
        }
        Err(CopyError::Unreadable(_)) => {
            return Some("the state its pin names does not read".to_owned());
        }
    };
    if copy.state_root != served.state_root {
        return Some(format!(
            "its state root is {}, and the remote's is {}",
            copy.state_root, served.state_root
        ));
    }
    if copy.interface_hash != served.interface_hash {
        return Some("its Interface is not the one the remote published".to_owned());
    }
    let labels = (served.labels.iter()).find(|(key, label)| copy.labels.get(key) != Some(label));
    if let Some((key, label)) = labels {
        return Some(format!(
            "the label of {key} is {}, and the remote's is {label}",
            copy.labels
                .get(key)
                .map_or_else(|| "missing".to_owned(), ToString::to_string)
        ));
    }
    if copy.labels.len() != served.labels.len() {
        return Some("it labels a node the remote's state does not".to_owned());
    }
    (copy.bytes != served.bytes)
        .then(|| "its file is not the one `loft interface` writes".to_owned())
}

fn report(checked: &[Checked<'_>]) -> Report {
    let mismatched = checked.iter().any(|copy| copy.difference.is_some());
    let mut lines = Vec::new();
    let mut copies = Vec::new();
    for copy in checked {
        let name = foreign_name(copy.copy, copy.copy.keys()[0]);
        let pinned = copy.copy.publish_sequence;
        let pin = match &copy.difference {
            Some(difference) => format!("mismatch: {difference}"),
            None => "confirmed".to_owned(),
        };
        lines.push(format!(
            "{name} at publish sequence {pinned}, {pin}; {} at the head, publish sequence {}",
            copy.freshness, copy.head
        ));
        let mut entry = json!({
            "interface": name,
            "publishSequence": pinned,
            "head": copy.head,
            "pin": if copy.difference.is_some() { "mismatch" } else { "confirmed" },
            "freshness": copy.freshness,
        });
        if let Some(difference) = &copy.difference {
            entry["difference"] = Value::String(difference.clone());
        }
        copies.push(entry);
    }
    let outcome = if mismatched { "mismatch" } else { "confirmed" };
    Report {
        outcome,
        fields: json!({ "copies": copies }),
        lines,
        failed: mismatched,
    }
}
