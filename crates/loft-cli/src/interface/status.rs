//! `loft interface status`: compares each copy with the state its pin names, which confirms it
//! or names its first difference, and with its remote's head, where its Interface is current,
//! behind or withdrawn. It writes nothing; a mismatch exits 1.

use std::path::Path;

use loftspec::codec::{self, CopyError};
use loftspec::copy::{Copy, CopyDifference};
use loftspec::document::Document;
use loftspec::identity::Label;
use serde_json::{Value, json};

use super::reach::{Remotes, unreadable};
use super::{Report, connected, document, entry_of};
use crate::remote::SignIn;
use crate::stop::Stop;

/// One copy as its remote serves it: its pin confirmed or its first difference, and how the
/// head holds its Interface.
struct Checked<'a> {
    copy: &'a Copy,
    difference: Option<String>,
    head: u64,
    freshness: &'static str,
}

pub(super) fn run(root: &Path) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    connected(&document)?;
    let mut remotes = Remotes::new(SignIn::Kept);
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
    let difference = pinned_difference(remotes, document, copy)?;
    let head = remotes.head(entry)?;
    let freshness = if head == pinned {
        "current"
    } else {
        match remotes.state(entry, head)? {
            None => return Err(super::reach::not_held(&entry.remote, head)),
            Some((files, lock)) => {
                match codec::copy_of(&copy.handle, files, copy.key(), head, lock.state_root) {
                    Ok(at_head) if at_head.interface_hash == copy.interface_hash => "current",
                    Ok(_) => "behind",
                    Err(CopyError::NotHeld(_)) => "withdrawn",
                    Err(CopyError::Unreadable(errors)) => {
                        return Err(unreadable(&copy.handle, head, &errors));
                    }
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

/// How `copy` differs from what its remote serves at the publish sequence its pin names: none
/// where it is the copy `loft interface` takes from that state.
fn pinned_difference(
    remotes: &mut Remotes,
    document: &Document,
    copy: &Copy,
) -> Result<Option<String>, Stop> {
    let pinned = copy.publish_sequence;
    Ok(match remotes.state(entry_of(document, copy), pinned)? {
        None => Some(format!(
            "the remote holds no published state of publish sequence {pinned}"
        )),
        Some((files, lock)) => {
            let taken = codec::copy_of(&copy.handle, files, copy.key(), pinned, lock.state_root);
            first_difference(copy, taken)
        }
    })
}

/// Confirms each copy of `document` against the state its pin names at the remote of its
/// entry, with the person's sign-in to that address, before a publish is sent.
///
/// # Errors
///
/// A remote cannot be reached or refuses, or a copy differs: the error names the copy's file and
/// its first difference.
pub fn confirm(document: &Document) -> Result<(), String> {
    let mut remotes = Remotes::new(SignIn::Kept);
    for copy in &document.copies {
        let difference = pinned_difference(&mut remotes, document, copy)
            .map_err(|stop| stop.lines.join("; "))?;
        if let Some(difference) = difference {
            return Err(format!(
                "spec/{}: {difference}; the copy is not the one its remote published, and \
                 `loft interface update` takes it again",
                copy.path().as_str()
            ));
        }
    }
    Ok(())
}

/// How `copy` differs from the copy `taken` from the state its pin names, in words: none where
/// it is that copy, byte for byte.
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
    let label = |label: Option<Label>| label.map_or_else(|| "none".to_owned(), |l| l.to_string());
    Some(match copy.difference(&served)? {
        CopyDifference::StateRoot { held, served } => {
            format!("its state root is {held}, and the remote's is {served}")
        }
        CopyDifference::Interface => "its Interface is not the one the remote published".to_owned(),
        CopyDifference::Label { key, held, served } => format!(
            "the label of {key} is {}, and the remote's is {}",
            label(held),
            label(served)
        ),
        CopyDifference::Bytes => "its file is not the one `loft interface` writes".to_owned(),
    })
}

fn report(checked: &[Checked<'_>]) -> Report {
    let mismatched = checked.iter().any(|copy| copy.difference.is_some());
    let mut lines = Vec::new();
    let mut copies = Vec::new();
    for copy in checked {
        let name = copy.copy.name_of(copy.copy.key());
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
