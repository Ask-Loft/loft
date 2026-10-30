//! `loft remote`: names, signs in to and removes the current document's remote. The address is
//! the manifest's `remote`, shared by every clone; the person's sign-in to it is kept in
//! `loft/remotes.json`, private to the user. Like `loft telemetry`, it sends no command event and
//! asks for no update.

mod connect;

use std::path::Path;

use loftspec::codec::{self, Files, MANIFEST, SpecPath};
use loftspec::document::Document;
use loftspec_store::FileStore;
use serde_json::{Value, json};

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::remote::{self, Client, RemoteError, signin, signins};
use crate::stop::Stop;

/// The schema `--format json` writes to, by name and version.
const SCHEMA: &str = "loft-remote/1";

/// What the command does.
pub enum Action {
    Set { address: String, browser: bool },
    Login { browser: bool },
    Remove,
}

impl From<RemoteError> for Stop {
    fn from(error: RemoteError) -> Self {
        Stop::one("remote", error.to_string())
    }
}

/// What a subcommand reports: its outcome, its fields, and its lines of text.
struct Report {
    outcome: &'static str,
    fields: Value,
    lines: Vec<String>,
}

pub fn run(root: &Path, action: &Action, as_json: bool) -> Finished {
    let report = match action {
        Action::Set { address, browser } => set(root, address, *browser),
        Action::Login { browser } => login(root, *browser),
        Action::Remove => remove(root),
    };
    match report {
        Ok(report) => {
            if as_json {
                json::print(&json::result(
                    SCHEMA,
                    report.outcome,
                    report.fields,
                    Vec::new(),
                ));
            } else {
                for line in &report.lines {
                    outln!("{line}");
                }
            }
            Finished::success(report.outcome)
        }
        Err(stop) => stop.exit(SCHEMA, as_json),
    }
}

/// The tree's store and the document it holds.
fn document(root: &Path) -> Result<(FileStore, Document), Stop> {
    let store = FileStore::open(root)?;
    let document = codec::read(&store.read_files()?).map_err(Stop::from)?;
    Ok((store, document))
}

/// Writes the manifest of `document` alone, in one journaled save.
fn write_manifest(store: &FileStore, document: &Document) -> Result<(), Stop> {
    let path = SpecPath::new(MANIFEST);
    let mut written = codec::write(document);
    let manifest = written
        .remove(&path)
        .expect("the writer writes the manifest");
    store.write_files(Files::from([(path, manifest)]))?;
    Ok(())
}

/// `loft remote set <address>`: connects the document where the manifest does not mark it
/// connected, then names the address as its remote.
fn set(root: &Path, address: &str, browser: bool) -> Result<Report, Stop> {
    remote::checked(address)?;
    let timeout = remote::timeout()?;
    let wait = remote::seconds(remote::WAIT_ENV)?;
    let (store, mut document) = document(root)?;
    let key = document.id.key().to_string();
    let mut fields = json!({ "documentKey": key, "address": address });
    let mut lines = Vec::new();
    let outcome = if document.connected {
        signed_in(address, browser, timeout)?;
        lines.push(format!("the document {key} now names its remote {address}"));
        "set"
    } else {
        let lock = store.lock()?;
        let states = match &lock {
            Some(lock) => connect::history(root, lock)?,
            None => Vec::new(),
        };
        let token = signed_in(address, browser, timeout)?;
        let client = Client::new(address, Some(token), timeout, wait);
        let connected = connect::connect(&client, &document, lock.as_ref(), &states)?;
        fields["head"] = connected.head.json();
        lines.push(format!(
            "connected the document {key} to {address} at {}",
            connected.head
        ));
        if lock.is_none() {
            lines.push("The remote holds no published state of the document.".to_owned());
        } else if connected.imported {
            lines.push(format!("imported {} published states", states.len()));
        }
        lines.push("Commit spec/loftspec.yaml: it now names the document's remote.".to_owned());
        "connected"
    };
    document.remote = Some(address.to_owned());
    document.connected = true;
    write_manifest(&store, &document)?;
    Ok(Report {
        outcome,
        fields,
        lines,
    })
}

/// The access token of the person's sign-in to `address`: the one `loft/remotes.json` holds,
/// refreshed where it needs to be, or else, or where its refresh is refused, a new sign-in kept
/// there.
fn signed_in(address: &str, browser: bool, timeout: std::time::Duration) -> Result<String, Stop> {
    match signins::live(address, timeout) {
        Ok(sign_in) => Ok(sign_in.access_token),
        Err(RemoteError::NoSignIn { .. } | RemoteError::Lapsed { .. }) => {
            let sign_in = signin::sign_in(address, browser, timeout)?;
            let token = sign_in.access_token.clone();
            signins::save(address, &sign_in)?;
            Ok(token)
        }
        Err(error) => Err(error.into()),
    }
}

/// `loft remote login`: a new sign-in to the remote the manifest names.
fn login(root: &Path, browser: bool) -> Result<Report, Stop> {
    let (_, document) = document(root)?;
    let key = document.id.key().to_string();
    let address = document.remote.clone().ok_or_else(|| {
        Stop::one(
            "remote",
            format!(
                "the document {key} names no remote: `loft remote set <address>` connects it to \
                 its remote"
            ),
        )
    })?;
    remote::checked(&address)?;
    let timeout = remote::timeout()?;
    let sign_in = signin::sign_in(&address, browser, timeout)?;
    signins::save(&address, &sign_in)?;
    Ok(Report {
        outcome: "signed in",
        fields: json!({ "documentKey": key, "address": address }),
        lines: vec![format!("signed in to {address} for the document {key}")],
    })
}

/// `loft remote remove`: disconnects the document and forgets the sign-in to its remote.
fn remove(root: &Path) -> Result<Report, Stop> {
    let (store, mut document) = document(root)?;
    let key = document.id.key().to_string();
    if !document.connected && document.remote.is_none() {
        return Ok(Report {
            outcome: "unchanged",
            fields: json!({ "documentKey": key }),
            lines: vec![format!("the document {key} is not connected to a remote")],
        });
    }
    let address = document.remote.take();
    document.connected = false;
    let mut fields = json!({ "documentKey": key });
    if let Some(address) = &address {
        signins::forget(address)?;
        fields["address"] = json!(address);
    }
    write_manifest(&store, &document)?;
    Ok(Report {
        outcome: "removed",
        fields,
        lines: vec![
            format!(
                "disconnected the document {key}{}",
                address.map(|a| format!(" from {a}")).unwrap_or_default()
            ),
            "Commit spec/loftspec.yaml: the document no longer publishes through its remote."
                .to_owned(),
        ],
    })
}
