//! What the tests of the kept sign-ins share: a `loft/remotes.json` in a scratch directory of
//! its own, never the person's, and sign-ins that the remote simulator issues without a browser.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Companion, Remote, RemoteScenario};

use super::expires_at;

/// The time limit of each call a test makes.
pub(super) const TIMEOUT: Duration = Duration::from_secs(30);

/// A `loft/remotes.json` in a scratch directory of its own, which is removed when it is dropped.
pub(super) struct Scratch {
    dir: PathBuf,
    pub(super) file: PathBuf,
}

impl Scratch {
    /// A scratch directory whose `loft/remotes.json` is not written yet.
    pub(super) fn new() -> Self {
        let dir = std::env::temp_dir().join(format!("loft-sign-ins-{}", uuid::Uuid::new_v4()));
        let file = dir.join("remotes.json");
        Self { dir, file }
    }

    /// A scratch directory whose `loft/remotes.json` keeps `entry` for the address of `remote`.
    pub(super) fn holding(remote: &Remote, entry: &Value) -> Self {
        let scratch = Self::new();
        fs::create_dir_all(&scratch.dir).unwrap();
        let file = json!({ "remotes": { remote.address(): entry } });
        fs::write(&scratch.file, serde_json::to_vec_pretty(&file).unwrap()).unwrap();
        scratch
    }

    /// The file's bytes.
    pub(super) fn bytes(&self) -> Vec<u8> {
        fs::read(&self.file).unwrap()
    }

    /// The sign-in that the file keeps for the address of `remote`.
    pub(super) fn kept(&self, remote: &Remote) -> Value {
        let file: Value = serde_json::from_slice(&self.bytes()).unwrap();
        file["remotes"][remote.address()].clone()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// A remote simulator under `scenario` that lists the companion resource of the kind `notes`.
pub(super) fn listing_notes(scenario: RemoteScenario) -> Remote {
    Remote::start(scenario.accompanied_by(Companion::new("notes", &["notes"])))
}

/// A sign-in over the protected resource of `remote` and its companion resources of `kinds`, as
/// `loft` keeps it, issued without a browser: every access token expires `seconds` from now, and
/// a companion resource holds its own only where `tokens` is true. A sign-in over no companion
/// resource holds the six members alone.
pub(super) fn over(remote: &Remote, kinds: &[&str], seconds: u64, tokens: bool) -> Value {
    let own = remote.resource();
    let companions: Vec<String> = kinds.iter().map(|kind| remote.companion(kind)).collect();
    let mut resources = vec![own.as_str()];
    resources.extend(companions.iter().map(String::as_str));
    let grant = remote.grant_over(&resources);
    let expires = expires_at(seconds);
    let mut entry = json!({
        "tokenEndpoint": format!("{}/auth/token", remote.origin()),
        "resource": own,
        "clientId": grant.client_id,
        "accessToken": grant.access_token,
        "expiresAt": expires,
        "refreshToken": grant.refresh_token,
    });
    if !kinds.is_empty() {
        let listed = kinds.iter().zip(&companions).map(|(kind, resource)| {
            let mut covered = json!({ "resource": resource, "kind": kind });
            if tokens {
                covered["accessToken"] = json!(grant.access[resource]);
                covered["expiresAt"] = json!(expires);
            }
            covered
        });
        entry["issuer"] = json!(remote.issuer());
        entry["resources"] = Value::Array(listed.collect());
    }
    entry
}

/// Each refresh that the token endpoint of `remote` received, in order: the refresh token it
/// presented and each `resource` it named.
pub(super) fn sent(remote: &Remote) -> Vec<(String, Vec<String>)> {
    let requests = remote.requests().into_iter();
    let refreshes = requests.filter(|request| request.path == "/auth/token");
    let read = refreshes.map(|request| {
        let form = pairs(&String::from_utf8_lossy(&request.body));
        let presented = value(&form, "refresh_token").unwrap_or_default().to_owned();
        let named = form.iter().filter(|(field, _)| field == "resource");
        (
            presented,
            named.map(|(_, resource)| resource.clone()).collect(),
        )
    });
    read.collect()
}

/// The refresh token that `entry`, a sign-in as `loft` keeps it, presents.
pub(super) fn refresh_token(entry: &Value) -> String {
    entry["refreshToken"].as_str().unwrap().to_owned()
}
