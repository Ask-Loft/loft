//! `loft remote remove`: the document is disconnected and the sign-in to its remote forgotten,
//! without reaching the remote.

use loftspec::codec;
use serde_json::json;
use software_plant::BrowserScenario;
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::signed_in::{
    document, granted, keep, name_remote, run, sign_ins, sign_ins_file,
};
use crate::common::{Scratch, tree};

#[test]
fn remove_disconnects_the_document_and_forgets_its_sign_in_alone() {
    let remote = Remote::start(RemoteScenario::accepting("doc-remove"));
    let scratch = Scratch::with_reading_list();
    let unconnected = codec::read(&tree(&scratch.0)).unwrap();
    name_remote(&scratch, Some(&remote.address()), true);
    let other = json!({ "tokenEndpoint": "https://other.example/token", "resource": "r",
        "clientId": "c", "accessToken": "a", "expiresAt": "2030-01-01T00:00:00Z",
        "refreshToken": "f" });
    keep(
        &scratch,
        &json!({ remote.address(): granted(&remote, 3600), "https://other.example/doc": other }),
    );
    #[cfg(unix)]
    let kept_inode = {
        use std::os::unix::fs::MetadataExt as _;
        std::fs::metadata(sign_ins_file(&scratch)).unwrap().ino()
    };
    let (output, _) = run(
        &scratch,
        &["remote", "remove", "--format", "json"],
        BrowserScenario::Absent,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "removed");
    assert_eq!(result["address"], remote.address());
    assert_eq!(result["documentKey"], unconnected.id.key().to_string());
    let manifest = std::fs::read_to_string(scratch.file("loftspec.yaml")).unwrap();
    assert!(!manifest.contains("remote:") && !manifest.contains("connected:"));
    assert_eq!(codec::read(&tree(&scratch.0)).unwrap(), unconnected);
    assert_eq!(
        sign_ins(&scratch),
        json!({ "https://other.example/doc": other })
    );
    assert!(remote.requests().is_empty(), "no connection is opened");
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt as _, PermissionsExt as _};
        let written = std::fs::metadata(sign_ins_file(&scratch)).unwrap();
        assert_eq!(written.permissions().mode() & 0o777, 0o600);
        assert_ne!(
            written.ino(),
            kept_inode,
            "a file beside it is renamed over it"
        );
    }
    let left: Vec<String> = std::fs::read_dir(scratch.config())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert_eq!(left, Vec::<String>::new(), "no file beside it is left");

    let (output, _) = run(
        &scratch,
        &["remote", "remove"],
        BrowserScenario::Absent,
        None,
    );
    assert_eq!(output.status.code(), Some(0));
    let (output, _) = run(
        &scratch,
        &["remote", "remove"],
        BrowserScenario::Absent,
        None,
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("not connected"), "{stdout}");
}

#[test]
fn a_disconnect_says_to_commit_the_manifest() {
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some("https://remote.example/documents/one"), true);
    let (output, _) = run(
        &scratch,
        &["remote", "remove"],
        BrowserScenario::Absent,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains(
            "Commit spec/loftspec.yaml: the document no longer publishes through its remote."
        ),
        "{stdout}"
    );
    assert!(
        !sign_ins_file(&scratch)
            .with_file_name("remotes.json.lock")
            .exists(),
        "no lock is taken for a missing file"
    );
    assert!(
        !sign_ins_file(&scratch).exists(),
        "a missing file is not created"
    );
}

#[test]
fn remove_leaves_an_unconnected_manifest_and_the_sign_in_file_as_they_were() {
    let scratch = Scratch::with_reading_list();
    let before = tree(&scratch.0);
    keep(
        &scratch,
        &json!({ "https://other.example/doc": { "accessToken": "a" } }),
    );
    let file = std::fs::read(sign_ins_file(&scratch)).unwrap();
    let (output, _) = run(
        &scratch,
        &["remote", "remove", "--format", "json"],
        BrowserScenario::Absent,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(document(&output)["outcome"], "unchanged");
    assert_eq!(tree(&scratch.0), before);
    assert_eq!(std::fs::read(sign_ins_file(&scratch)).unwrap(), file);
}

#[test]
fn every_subcommand_refuses_a_directory_without_a_manifest_naming_its_path() {
    let remote = Remote::start(RemoteScenario::accepting("doc-missing"));
    let address = remote.address();
    for args in [
        vec!["remote", "set", address.as_str()],
        vec!["remote", "login"],
        vec!["remote", "remove"],
    ] {
        let scratch = Scratch::empty();
        let (output, browser) = run(&scratch, &args, BrowserScenario::Visiting, None);
        assert_eq!(output.status.code(), Some(1), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("spec"), "{args:?}: {stderr}");
        assert!(browser.visits().is_empty(), "{args:?}");
        assert!(!scratch.0.join("spec").exists());
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}
