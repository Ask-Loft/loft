//! Running `loft remote` against the remote simulator: a manifest that names the remote, a
//! sign-in kept in `loft/remotes.json`, the browser simulator in `BROWSER`, and the repository
//! whose history holds each published state.

use std::fmt::Write as _;
use std::fs;
use std::path::PathBuf;
use std::process::Output;

use loftspec::codec::{self, SpecPath};
use loftspec::publish::{Lock, PublishedAt};
use serde_json::{Value, json};
use sha2::{Digest as _, Sha256};
use software_plant::remote::{Remote, StoredState};
use software_plant::{BrowserScenario, Clock, GitScenario, InstalledBrowser};

use super::remote::{History, spec};
use super::{EPOCH, Scratch, tree};

/// The manifest, by its path under `spec/`.
pub const MANIFEST: &str = "loftspec.yaml";

/// Rewrites the manifest under `scratch` to name `remote`, marked `connected` or not.
pub fn name_remote(scratch: &Scratch, remote: Option<&str>, connected: bool) {
    let mut document = codec::read(&tree(&scratch.0)).unwrap();
    document.connected = connected;
    document.remote = remote.map(str::to_owned);
    let manifest = SpecPath::new(MANIFEST);
    fs::write(scratch.file(MANIFEST), &codec::write(&document)[&manifest]).unwrap();
}

/// The path of `loft/remotes.json` for `scratch`.
pub fn sign_ins_file(scratch: &Scratch) -> PathBuf {
    scratch.config().join("remotes.json")
}

/// Every sign-in `loft/remotes.json` holds for `scratch`, by address.
pub fn sign_ins(scratch: &Scratch) -> Value {
    scratch.setting("remotes.json")["remotes"].clone()
}

/// A sign-in to `remote` as `loft` keeps it, its access token expiring `seconds` after
/// [`EPOCH`], issued by the simulator so that the remote admits it.
pub fn granted(remote: &Remote, seconds: u64) -> Value {
    let grant = remote.grant();
    json!({
        "tokenEndpoint": format!("{}/auth/token", remote.origin()),
        "resource": format!("{}/documents", remote.origin()),
        "clientId": grant.client_id,
        "accessToken": grant.access_token,
        "expiresAt": PublishedAt::from_unix_seconds(EPOCH + seconds).unwrap().to_string(),
        "refreshToken": grant.refresh_token,
    })
}

/// Writes `sign_ins`, by address, as `loft/remotes.json` for `scratch`.
pub fn keep(scratch: &Scratch, sign_ins: &Value) {
    fs::create_dir_all(scratch.config()).unwrap();
    let file = json!({ "remotes": sign_ins });
    fs::write(sign_ins_file(scratch), file.to_string()).unwrap();
}

/// Runs `loft` with `args` at [`EPOCH`], the browser simulator behaving as `browser`, and the
/// git simulator answering from `git` or no `git` at all.
pub fn run(
    scratch: &Scratch,
    args: &[&str],
    browser: BrowserScenario,
    git: Option<&GitScenario>,
) -> (Output, InstalledBrowser) {
    let dir = scratch.plant().join("browser");
    fs::create_dir_all(&dir).unwrap();
    let installed = browser.install(&dir);
    let mut command = scratch.command(git, &Clock::fixed(EPOCH));
    installed.apply(&mut command);
    (command.args(args).output().unwrap(), installed)
}

/// The JSON document a run printed.
pub fn document(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|_| {
        panic!(
            "no JSON on standard output\n{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

/// The commit that published sequence `sequence` in [`repository`].
pub fn commit(sequence: usize) -> String {
    format!("{sequence:040x}")
}

/// An object id for `text`, as the scenario names it.
fn object(text: &str) -> String {
    let digest = Sha256::digest(text.as_bytes());
    let mut hex = String::new();
    for byte in &digest[..20] {
        write!(hex, "{byte:02x}").unwrap();
    }
    hex
}

/// The bytes of a stored state's lock, as `loft.lock.json` holds them.
pub fn lock_bytes(state: &StoredState) -> Vec<u8> {
    Lock::read(state.lock.to_string().as_bytes())
        .unwrap()
        .to_bytes()
}

/// A repository on `main` whose history published each state of `history`, one commit each,
/// with its tree under `scratch` at the last state; the commits that published `sequences`
/// alone are in the history.
pub fn repository(scratch: &Scratch, history: &History, sequences: &[usize]) -> GitScenario {
    let last = history.states.last().unwrap();
    fs::remove_dir_all(scratch.0.join("spec")).unwrap();
    for (path, bytes) in spec(&last.files) {
        let target = scratch.file(path.as_str());
        fs::create_dir_all(target.parent().unwrap()).unwrap();
        fs::write(target, bytes).unwrap();
    }
    fs::write(scratch.0.join("loft.lock.json"), lock_bytes(last)).unwrap();
    let commits: Vec<String> = sequences.iter().map(|sequence| commit(*sequence)).collect();
    let listed: Vec<&str> = commits.iter().map(String::as_str).collect();
    let mut scenario = GitScenario::clean_on("main").lock_history(&listed);
    for sequence in sequences {
        let state = &history.states[sequence - 1];
        let commit = commit(*sequence);
        scenario = scenario.base(&commit, &commit);
        let lock = String::from_utf8(lock_bytes(state)).unwrap();
        scenario = scenario.file_at(&commit, "loft.lock.json", &object(&lock), &lock);
        for (path, text) in &state.files {
            scenario = scenario.file_at(&commit, path, &object(text), text);
        }
    }
    scenario
}
