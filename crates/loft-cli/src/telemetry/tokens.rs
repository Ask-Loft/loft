//! The tokens that let the collector tell runs apart without knowing whose they are: an
//! installation token for the user on the machine, a project token for one checkout, and a
//! branch token keyed with a secret that never leaves the checkout. Each is drawn at random, so
//! none can be computed from what it stands for.

use std::fmt::Write as _;
use std::fs;
use std::path::Path;

use hmac::{Hmac, Mac};
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};
use sha2::Sha256;

use super::event::Checkout;

/// The directory beside `spec/` that holds the checkout's tokens, which git never sees.
const LOCAL: &str = ".loft";

/// The file in [`LOCAL`] that holds them.
const FILE: &str = "telemetry.json";

/// The installation token kept in `kept`, the telemetry settings, drawn there when it has none.
/// Gives the token and whether it was drawn.
pub fn installation(kept: &mut Value) -> (String, bool) {
    if let Some(token) = kept["installation"].as_str().filter(|token| valid(token)) {
        return (token.to_owned(), false);
    }
    let token = random_token();
    kept["installation"] = json!(token);
    (token, true)
}

/// The tokens of the checkout at `root`, if it holds `spec/`: its project token and the token
/// of its branch, drawn and kept in `.loft/telemetry.json` when missing. None when the file
/// cannot be written.
pub fn checkout(root: &Path) -> Option<Checkout> {
    if !root.join("spec").is_dir() {
        return None;
    }
    let path = root.join(LOCAL).join(FILE);
    let kept: Value = fs::read(&path)
        .ok()
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_else(|| json!({}));
    let project = kept["project"].as_str().filter(|token| valid(token));
    let secret = kept["branchSecret"]
        .as_str()
        .filter(|secret| secret.len() == 64 && secret.bytes().all(|b| b.is_ascii_hexdigit()));
    let (project, secret) = if let (Some(project), Some(secret)) = (project, secret) {
        (project.to_owned(), secret.to_owned())
    } else {
        let drawn = (random_token(), random_secret());
        keep(
            root,
            &json!({ "project": drawn.0, "branchSecret": drawn.1 }),
        )?;
        drawn
    };
    let branch = GitCommand::default()
        .branch(root)
        .ok()
        .flatten()
        .map(|name| keyed(&secret, &name));
    Some(Checkout { project, branch })
}

/// Writes the checkout's tokens, with a `.gitignore` that keeps every file of `.loft/` out of git.
fn keep(root: &Path, tokens: &Value) -> Option<()> {
    let local = root.join(LOCAL);
    fs::create_dir_all(&local).ok()?;
    let ignore = local.join(".gitignore");
    if !ignore.exists() {
        fs::write(&ignore, "*\n").ok()?;
    }
    let temporary = local.join(format!("{FILE}.new"));
    fs::write(&temporary, tokens.to_string()).ok()?;
    fs::rename(&temporary, local.join(FILE)).ok()
}

/// The branch `name` as HMAC-SHA256 keyed with `secret`, its first 16 hexadecimal digits.
fn keyed(secret: &str, name: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC takes a key of any length");
    mac.update(name.as_bytes());
    mac.finalize().into_bytes()[..8]
        .iter()
        .fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        })
}

/// A random UUID, version 4.
fn random_token() -> String {
    uuid::Uuid::new_v4().to_string()
}

/// A random secret of 64 hexadecimal digits.
fn random_secret() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

/// Whether `token` is a UUID of version 4.
fn valid(token: &str) -> bool {
    uuid::Uuid::parse_str(token).is_ok_and(|uuid| uuid.get_version_num() == 4)
}

#[cfg(test)]
mod tests {
    use super::keyed;

    #[test]
    fn a_branch_token_is_16_hex_digits_that_depend_on_the_secret_and_the_name() {
        let secret = "0".repeat(64);
        let token = keyed(&secret, "main");
        assert_eq!(token.len(), 16);
        assert!(token.bytes().all(|b| b.is_ascii_hexdigit()));
        assert_eq!(keyed(&secret, "main"), token);
        assert_ne!(keyed(&secret, "develop"), token);
        assert_ne!(keyed(&"1".repeat(64), "main"), token);
    }
}
