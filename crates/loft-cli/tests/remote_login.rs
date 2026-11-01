//! `loft remote login`: a sign-in through OAuth to the remote the manifest names, run end to
//! end against the remote simulator's authorization server and the browser simulator.

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::TcpStream;
use std::process::Stdio;

use loftspec::codec::Files;
use serde_json::{Value, json};
use software_plant::BrowserScenario;
use software_plant::remote::wire::{pairs, value};
use software_plant::remote::{Remote, RemoteScenario};

use crate::common::signed_in::{
    self, document, granted, keep, name_remote, run, sign_ins, sign_ins_file,
};
use crate::common::{Scratch, tree};

/// A connected Reading List whose manifest names `remote`.
fn connected(remote: &Remote) -> Scratch {
    let scratch = Scratch::with_reading_list();
    name_remote(&scratch, Some(&remote.address()), true);
    scratch
}

/// The value of `name` in the query of `url`.
fn query(url: &str, name: &str) -> Option<String> {
    let (_, query) = url.split_once('?')?;
    value(&pairs(query), name).map(str::to_owned)
}

/// Every token the remote's authorization server issued in `sign_in`.
fn tokens(sign_in: &Value) -> [String; 2] {
    [
        sign_in["accessToken"].as_str().unwrap().to_owned(),
        sign_in["refreshToken"].as_str().unwrap().to_owned(),
    ]
}

#[test]
fn login_signs_in_through_the_browser_and_keeps_the_sign_in_under_the_remote_s_address() {
    let remote = Remote::start(RemoteScenario::accepting("doc-login"));
    let scratch = connected(&remote);
    let before: Files = tree(&scratch.0);
    let (output, _) = run(
        &scratch,
        &["remote", "login", "--format", "json"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let result = document(&output);
    assert_eq!(result["outcome"], "signed in");
    assert_eq!(result["address"], remote.address());
    assert_eq!(tree(&scratch.0), before, "no file of the tree is written");

    let sign_in = &sign_ins(&scratch)[remote.address()];
    assert_eq!(
        sign_in["tokenEndpoint"],
        format!("{}/auth/token", remote.origin())
    );
    assert_eq!(
        sign_in["resource"],
        format!("{}/documents", remote.origin())
    );
    assert!(
        sign_in["clientId"]
            .as_str()
            .is_some_and(|id| !id.is_empty())
    );
    assert!(sign_in["expiresAt"].as_str().unwrap().ends_with('Z'));
    let [access, refresh] = tokens(sign_in);
    assert!(remote.admits(&access), "the access token is the one issued");
    let printed = format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!printed.contains(&access) && !printed.contains(&refresh));
}

#[test]
fn a_sign_in_learns_the_server_registers_and_asks_for_a_code_with_pkce_on_a_loopback_port() {
    let remote = Remote::start(RemoteScenario::accepting("doc-request"));
    let scratch = connected(&remote);
    let (output, browser) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let printed = String::from_utf8_lossy(&output.stderr).into_owned();
    let sign_in = &sign_ins(&scratch)[remote.address()];
    let requests = remote.requests();
    let first = &requests[0];
    assert_eq!(
        (first.method.as_str(), first.path.as_str()),
        ("GET", "/documents/doc-request/head")
    );
    assert_eq!(first.header("authorization"), None);
    let registration = requests
        .iter()
        .find(|r| r.path == "/auth/register")
        .unwrap();
    let body: Value = serde_json::from_slice(&registration.body).unwrap();
    assert_eq!(body["client_name"], "loft");
    assert_eq!(body["token_endpoint_auth_method"], "none");
    assert_eq!(
        body["grant_types"],
        json!(["authorization_code", "refresh_token"])
    );
    assert_eq!(body["response_types"], json!(["code"]));
    assert_eq!(body["redirect_uris"], json!(["http://127.0.0.1/callback"]));

    let visits = browser.visits();
    assert_eq!(visits.len(), 1, "{visits:?}");
    let url = &visits[0];
    assert!(printed.contains(url.as_str()), "the address is printed too");
    assert_eq!(query(url, "response_type").as_deref(), Some("code"));
    assert_eq!(query(url, "code_challenge_method").as_deref(), Some("S256"));
    assert_eq!(
        query(url, "client_id").as_deref(),
        sign_in["clientId"].as_str()
    );
    assert_eq!(
        query(url, "resource"),
        Some(format!("{}/documents", remote.origin()))
    );
    let redirect = query(url, "redirect_uri").unwrap();
    let port = redirect
        .strip_prefix("http://127.0.0.1:")
        .and_then(|rest| rest.strip_suffix("/callback"))
        .unwrap();
    assert!(port.parse::<u16>().is_ok_and(|port| port > 0), "{redirect}");
    assert!(query(url, "state").is_some_and(|state| state.len() >= 16));
    assert_eq!(query(url, "code_challenge").map(|c| c.len()), Some(43));
    assert!(
        TcpStream::connect(format!("127.0.0.1:{port}")).is_err(),
        "loft stops listening"
    );

    let exchange = requests.iter().find(|r| r.path == "/auth/token").unwrap();
    let form = String::from_utf8(exchange.body.clone()).unwrap();
    for field in [
        "grant_type=authorization_code",
        "code=",
        "redirect_uri=",
        "client_id=",
        "code_verifier=",
        "resource=",
    ] {
        assert!(form.contains(field), "{field} in {form}");
    }
}

#[cfg(unix)]
#[test]
fn the_sign_in_file_is_written_for_its_owner_alone_through_a_rename() {
    use std::os::unix::fs::PermissionsExt as _;

    let remote = Remote::start(RemoteScenario::accepting("doc-private"));
    let scratch = connected(&remote);
    let (output, _) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let mode = std::fs::metadata(sign_ins_file(&scratch))
        .unwrap()
        .permissions()
        .mode();
    assert_eq!(mode & 0o777, 0o600);
    let left: Vec<String> = std::fs::read_dir(scratch.config())
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with('.'))
        .collect();
    assert!(left.is_empty(), "no file beside it is left: {left:?}");
}

#[test]
fn login_replaces_the_sign_in_for_its_address_and_keeps_every_other() {
    let remote = Remote::start(RemoteScenario::accepting("doc-replace"));
    let scratch = connected(&remote);
    let old = granted(&remote, 3600);
    let other = json!({ "tokenEndpoint": "https://other.example/token", "resource": "r",
        "clientId": "c", "accessToken": "a", "expiresAt": "2030-01-01T00:00:00Z",
        "refreshToken": "f" });
    keep(
        &scratch,
        &json!({
            remote.address(): old,
            "https://other.example/doc": other,
            "https://newer.example/doc": { "accessToken": "a", "kept": "by a later loft" },
        }),
    );
    let (mut command, browser) = signed_in::command(&scratch, BrowserScenario::Visiting, None);
    // Neither variable changes what a sign-in does.
    command
        .env("LOFT_REMOTE_ENDPOINT", "http://127.0.0.1:9/elsewhere")
        .env("LOFT_REMOTE_TOKEN", "a-token-from-the-environment");
    let output = command.args(["remote", "login"]).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let kept = sign_ins(&scratch);
    assert_eq!(kept["https://other.example/doc"], other);
    assert_eq!(
        kept["https://newer.example/doc"],
        json!({ "accessToken": "a", "kept": "by a later loft" }),
        "an entry this loft does not read is kept as it was"
    );
    assert_ne!(kept[remote.address()]["accessToken"], old["accessToken"]);
    assert_eq!(browser.visits().len(), 1);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(&remote.address()), "{stdout}");
}

#[test]
fn login_without_a_remote_in_the_manifest_is_refused_naming_remote_set() {
    let remote = Remote::start(RemoteScenario::accepting("doc-none"));
    let scratch = Scratch::with_reading_list();
    let before = tree(&scratch.0);
    let (output, browser) = run(
        &scratch,
        &["remote", "login", "--format", "json"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    let result = document(&output);
    assert_eq!(result["outcome"], "error");
    let message = result["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains("loft remote set <address>"), "{message}");
    assert!(remote.requests().is_empty() && browser.visits().is_empty());
    assert_eq!(tree(&scratch.0), before);
    assert!(!sign_ins_file(&scratch).exists());
}

#[test]
fn a_refused_consent_ends_the_sign_in_naming_the_error_and_writes_nothing() {
    let remote = Remote::start(RemoteScenario::accepting("doc-deny").refusing_consent());
    let scratch = connected(&remote);
    let (output, _) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("access_denied"), "{stderr}");
    assert!(!sign_ins_file(&scratch).exists());
}

#[test]
fn a_remote_that_gives_no_bearer_challenge_is_refused_naming_its_address() {
    let remote = Remote::start(RemoteScenario::failing("doc-plain", 502, None));
    let scratch = connected(&remote);
    let (output, browser) = run(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Visiting,
        None,
    );
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains(&remote.address()) && stderr.contains("Bearer"),
        "{stderr}"
    );
    assert_eq!(browser.visits(), Vec::<String>::new());
    assert!(!sign_ins_file(&scratch).exists());
}

/// Runs `loft remote login` with `args`, reads the address it prints and then, where `until`
/// is given, the line holding it, and calls `visit` with the address while `loft` waits; gives
/// its output once it ends.
fn waiting(
    scratch: &Scratch,
    args: &[&str],
    browser: BrowserScenario,
    until: Option<&str>,
    visit: impl FnOnce(&str),
) -> (std::process::Output, Vec<String>) {
    let (mut command, installed) = signed_in::command(scratch, browser, None);
    let mut child = command
        .args(args)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stderr = BufReader::new(child.stderr.take().unwrap());
    let mut printed = String::new();
    let url = loop {
        let mut line = String::new();
        assert!(stderr.read_line(&mut line).unwrap() > 0, "{printed}");
        printed.push_str(&line);
        if let Some(url) = line.trim().strip_prefix("http") {
            break format!("http{url}");
        }
    };
    while let Some(until) = until
        && !printed.contains(until)
    {
        assert!(stderr.read_line(&mut printed).unwrap() > 0, "{printed}");
    }
    visit(&url);
    stderr.read_to_string(&mut printed).unwrap();
    let mut output = child.wait_with_output().unwrap();
    output.stderr = printed.into_bytes();
    (output, installed.visits())
}

/// Opens `url` as a person does by hand, through a browser of its own.
fn by_hand(scratch: &Scratch, url: &str) {
    let dir = scratch.plant().join("by-hand");
    std::fs::create_dir_all(&dir).unwrap();
    let _ = BrowserScenario::Visiting.install(&dir);
    assert_eq!(software_plant::browser::run(&dir, &[url.to_owned()]), 0);
}

/// A request for `target` on the loopback port of `redirect`, and the status line it answers.
fn status_of(redirect: &str, target: &str) -> String {
    let port = redirect
        .trim_start_matches("http://127.0.0.1:")
        .trim_end_matches("/callback");
    let mut stream = TcpStream::connect(format!("127.0.0.1:{port}")).unwrap();
    write!(stream, "GET {target} HTTP/1.1\r\nHost: x\r\n\r\n").unwrap();
    let mut answer = String::new();
    let _ = stream.read_to_string(&mut answer);
    answer.lines().next().unwrap_or_default().to_owned()
}

#[test]
fn with_no_browser_the_address_is_printed_and_a_wrong_state_does_not_end_the_wait() {
    let remote = Remote::start(RemoteScenario::accepting("doc-manual"));
    let scratch = connected(&remote);
    let (output, visits) = waiting(
        &scratch,
        &["remote", "login", "--no-browser"],
        BrowserScenario::Visiting,
        None,
        |url| {
            let redirect = query(url, "redirect_uri").unwrap();
            assert!(status_of(&redirect, "/elsewhere").contains("404"));
            assert!(status_of(&redirect, "/callback?state=wrong&code=x").contains("400"));
            by_hand(&scratch, url);
        },
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(visits.is_empty(), "no browser is opened: {visits:?}");
    assert!(sign_ins(&scratch)[remote.address()].is_object());
}

#[test]
fn a_browser_that_cannot_be_opened_is_reported_and_the_wait_goes_on() {
    let remote = Remote::start(RemoteScenario::accepting("doc-absent"));
    let scratch = connected(&remote);
    let (output, _) = waiting(
        &scratch,
        &["remote", "login"],
        BrowserScenario::Absent,
        // The callback arrives only once loft has seen the browser end.
        Some("No browser could be opened"),
        |url| by_hand(&scratch, url),
    );
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("No browser could be opened"), "{stderr}");
}
