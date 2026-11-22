//! `loft interface login <handle>`: a new sign-in to the remote that the handle's entry of
//! `documents` names.

use std::path::Path;

use loftspec::slug::Slug;
use serde_json::json;

use super::{Report, document, refused};
use crate::remote;
use crate::stop::Stop;

pub(super) fn run(root: &Path, handle: &str, browser: bool) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    let entry = Slug::parse(handle)
        .ok()
        .and_then(|handle| (document.documents.iter()).find(|entry| entry.handle == handle))
        .ok_or_else(|| refused(format!("`documents` does not list {handle}")))?;
    let key = entry.key.to_string();
    let whose = remote::Whose::Other {
        handle,
        key: Some(&key),
    };
    remote::login(&entry.remote, whose, browser)?;
    let address = &entry.remote;
    Ok(Report {
        outcome: "signed in",
        fields: json!({ "handle": handle, "address": address }),
        lines: vec![format!("signed in to {address} for {handle}")],
        failed: false,
    })
}
