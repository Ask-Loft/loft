//! The sign-ins kept in the file a caller names: here a `loft/remotes.json` in a scratch
//! directory of the test's own, never the person's.

use std::fs;
use std::path::PathBuf;
use std::time::Duration;

use serde_json::Value;

use super::{SignIn, expires_at, forget, live, save};
use crate::remote::RemoteError;

/// A `loft/remotes.json` in a scratch directory of its own, not yet written.
fn scratch_file() -> PathBuf {
    let dir = std::env::temp_dir().join(format!("loft-sign-ins-{}", uuid::Uuid::new_v4()));
    dir.join("remotes.json")
}

#[test]
fn a_sign_in_is_kept_in_the_file_the_caller_names_with_its_lock_beside_it() {
    let file = scratch_file();
    let dir = file.parent().unwrap().to_owned();
    let address = "https://remote.example/documents/one";
    let sign_in = SignIn {
        token_endpoint: "https://remote.example/auth/token".to_owned(),
        resource: "https://remote.example/documents".to_owned(),
        client_id: "client-1".to_owned(),
        access_token: "access-1".to_owned(),
        expires_at: expires_at(3600),
        refresh_token: "refresh-1".to_owned(),
        issuer: None,
        resources: Vec::new(),
    };
    save(&file, address, &sign_in).unwrap();
    assert!(
        dir.join("remotes.json.lock").exists(),
        "the lock lies beside it"
    );
    let written: Value = serde_json::from_slice(&fs::read(&file).unwrap()).unwrap();
    assert_eq!(written["remotes"][address]["refreshToken"], "refresh-1");
    // An access token that expires later than five minutes ahead is given as kept, with no
    // request.
    let timeout = Duration::from_secs(1);
    assert_eq!(live(&file, address, timeout), Ok(sign_in));
    assert_eq!(forget(&file, address), Ok(true));
    let none = RemoteError::NoSignIn {
        address: address.to_owned(),
    };
    assert_eq!(live(&file, address, timeout), Err(none));
    assert_eq!(forget(&file, address), Ok(false));
    fs::remove_dir_all(dir).unwrap();
}
