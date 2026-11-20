//! `loft interface remove <handle>:<interface>`: deletes a copy that nothing of the document
//! references, and the handle's entry of `documents` once no copy is left under it, without
//! reaching any remote; and `loft interface login <handle>`: a new sign-in to the remote that
//! the handle's entry names.

use std::collections::BTreeSet;
use std::path::Path;

use loftspec::codec::{self, Files, MANIFEST, SpecPath};
use loftspec::slug::Slug;
use loftspec_store::journal::{self, Changes};
use serde_json::json;

use super::references::references;
use super::update::foreign_name;
use super::{Report, document, held, refused};
use crate::remote::{self, signin, signins};
use crate::stop::Stop;

pub(super) fn run(root: &Path, target: &str) -> Result<Report, Stop> {
    let (_, mut document) = document(root)?;
    let copy = held(&document, target)?;
    let name = foreign_name(copy, copy.keys()[0]);
    let keys: BTreeSet<_> = copy.keys().into_iter().collect();
    let naming = references(&document, &keys);
    if !naming.is_empty() {
        let each: Vec<String> = (naming.iter())
            .map(|reference| format!("{} `{}`", reference.element, reference.field))
            .collect();
        return Err(refused(format!(
            "{name} is referenced by {}: remove those references first",
            each.join(", ")
        )));
    }
    let path = copy.path();
    let handle = copy.handle.clone();
    document.copies.retain(|held| held.path() != path);
    let unlisted = !document.copies.iter().any(|held| held.handle == handle);
    if unlisted {
        document.documents.retain(|entry| entry.handle != handle);
    }
    let manifest = SpecPath::new(MANIFEST);
    let written = codec::write(&document)
        .remove(&manifest)
        .expect("the writer writes the manifest");
    let changes = Changes::of_tree(
        Files::from([(manifest, written)]),
        std::slice::from_ref(&path),
    );
    journal::save(root, &changes)?;
    let mut lines = vec![format!("removed the copy of {name}")];
    if unlisted {
        lines.push(format!("`documents` no longer lists {handle}"));
    }
    lines.push(format!(
        "Commit spec/: it no longer holds a copy of {name}."
    ));
    Ok(Report {
        outcome: "removed",
        fields: json!({
            "interface": name,
            "path": format!("spec/{}", path.as_str()),
            "unlisted": unlisted,
        }),
        lines,
        failed: false,
    })
}

/// `loft interface login <handle>`: signs the person in to the remote of the handle's entry.
pub(super) fn login(root: &Path, handle: &str, browser: bool) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    let entry = Slug::parse(handle)
        .ok()
        .and_then(|handle| (document.documents.iter()).find(|entry| entry.handle == handle))
        .ok_or_else(|| refused(format!("`documents` does not list {handle}")))?;
    let address = remote::checked(&entry.remote)?;
    let timeout = remote::timeout()?;
    let sign_in = signin::sign_in(address, browser, timeout)?;
    signins::save(address, &sign_in)?;
    Ok(Report {
        outcome: "signed in",
        fields: json!({ "handle": handle, "address": address }),
        lines: vec![format!("signed in to {address} for {handle}")],
        failed: false,
    })
}
