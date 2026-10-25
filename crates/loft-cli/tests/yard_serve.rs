//! `loft yard serve`: one page server on the machine, listening on the loopback interface. A run
//! that finds one of its version or a newer one leaves it serving; one that finds an older one
//! asks it to stop through `~/.yard/stop.json` and serves once its lock is free; a page server
//! asked to stop by a newer `loft` finishes and exits; and the commands count a page server as
//! running only while it holds its lock.

use std::fs;
use std::io::Read;
use std::net::TcpListener;
use std::time::Duration;

use serde_json::{Value, json};
use sha2::Digest;
use software_plant::Clock;
use yardlog_store::lock;

use crate::common::{EPOCH, Scratch, stderr};
use crate::served::{Served, first_document, free_port, within};
use crate::yard::{ASK, EPOCH_TEXT, Yard, document};

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// The page's file `name`, as kept in the repository.
fn page_file(name: &str) -> Vec<u8> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("yard-page")
        .join(name);
    fs::read(path).unwrap()
}

/// `~/.yard/` of `yard`'s home.
fn yard_home(yard: &Yard) -> std::path::PathBuf {
    yard.scratch.home().join(".yard")
}

/// Names in `~/.yard/server.json` a page server of `version` on `port`, and holds its lock for
/// as long as the result lives, as a page server of that version would.
fn pretend_server(yard: &Yard, port: u16, version: &str) -> lock::Locked {
    let dir = yard_home(yard);
    fs::create_dir_all(&dir).unwrap();
    let server = json!({ "port": port, "pid": 1, "version": version, "formats": ["yardlog/1"] });
    fs::write(dir.join("server.json"), server.to_string()).unwrap();
    lock::lock(&dir.join("server.lock")).unwrap()
}

#[test]
fn the_page_server_serves_its_page_at_the_loopback_address_and_port() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    assert_eq!(
        served.started["url"],
        format!("http://127.0.0.1:{}/", served.port).as_str()
    );
    let page = served.get("/");
    assert_eq!(page.status, 200);
    assert!(page.head.contains("text/html"), "{}", page.head);
    assert_eq!(page.body, page_file("index.html"));
}

#[test]
fn the_page_s_files_are_the_ones_the_running_loft_was_built_with() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    for name in ["index.html", "yard.css"] {
        let file = served.get(&format!("/{name}"));
        assert_eq!(file.status, 200, "{name}");
        assert_eq!(file.body, page_file(name), "{name}");
    }
    assert_eq!(served.get("/nothing.js").status, 404);
}

#[test]
fn a_file_of_the_page_the_browser_holds_is_not_sent_again() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let first = served.get("/app.js");
    let tag = first
        .head
        .lines()
        .find_map(|line| line.strip_prefix("etag: "))
        .unwrap_or_else(|| panic!("{}", first.head))
        .trim()
        .to_owned();
    let head = format!(
        "GET /app.js HTTP/1.1\r\nhost: 127.0.0.1:{}\r\nif-none-match: {tag}\r\n",
        served.port
    );
    let again = served.send(&head, &[]);
    assert_eq!(again.status, 304, "{}", again.head);
    assert_eq!(again.body.len(), 0, "a 304 carries no body");
    let other = format!(
        "GET /app.js HTTP/1.1\r\nhost: 127.0.0.1:{}\r\nif-none-match: \"another\"\r\n",
        served.port
    );
    let changed = served.send(&other, &[]);
    assert_eq!(changed.status, 200);
    assert_eq!(changed.body, page_file("app.js"));
}

#[test]
fn mermaid_is_served_compressed_and_decompresses_to_the_release_its_readme_names() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let script = served.get("/vendor/mermaid.min.js");
    assert_eq!(script.status, 200);
    assert!(
        script.head.contains("content-encoding: gzip"),
        "{}",
        script.head
    );
    assert!(script.head.contains("text/javascript"), "{}", script.head);
    assert_eq!(script.body, page_file("vendor/mermaid.min.js.gz"));
    let mut text = String::new();
    flate2::read::GzDecoder::new(script.body.as_slice())
        .read_to_string(&mut text)
        .unwrap();
    let readme = String::from_utf8(page_file("vendor/README.md")).unwrap();
    let version = readme
        .split("Mermaid ")
        .nth(1)
        .and_then(|rest| rest.split(',').next())
        .unwrap();
    assert!(text.starts_with("\"use strict\";"), "{}", &text[..80]);
    assert!(
        text.contains(&format!("version:\"{version}\"")),
        "not {version}"
    );
    let digest = format!("{:x}", sha2::Sha256::digest(text.as_bytes()));
    assert!(
        readme.contains(&digest),
        "the README names another digest than {digest}"
    );
    assert_eq!(
        served.get("/vendor/mermaid.LICENSE").body,
        page_file("vendor/mermaid.LICENSE")
    );
}

#[test]
fn every_script_style_and_font_the_page_loads_comes_from_the_page_server() {
    let page = String::from_utf8(page_file("index.html")).unwrap();
    for attribute in ["src=", "href="] {
        for (at, _) in page.match_indices(attribute) {
            let value = &page[at + attribute.len()..];
            assert!(
                !value.starts_with("\"http") && !value.starts_with("\"//"),
                "the page loads {}",
                &value[..value.find('>').unwrap_or(value.len())]
            );
        }
    }
    let style = String::from_utf8(page_file("yard.css")).unwrap();
    for (at, _) in style.match_indices("url(") {
        let value = style[at + "url(".len()..].trim_start_matches(['"', '\'']);
        assert!(
            !value.starts_with("http") && !value.starts_with("//"),
            "the style loads {}",
            &value[..value.find(')').unwrap_or(value.len())]
        );
    }
    assert!(style.contains("url(\"vendor/inter-latin.woff2\")"));
}

#[test]
fn inter_is_served_as_a_font_with_its_licence() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let font = served.get("/vendor/inter-latin.woff2");
    assert_eq!(font.status, 200);
    assert!(font.head.contains("font/woff2"), "{}", font.head);
    assert_eq!(font.body, page_file("vendor/inter-latin.woff2"));
    let licence = served.get("/vendor/inter.LICENSE");
    assert_eq!(licence.status, 200);
    assert!(
        String::from_utf8(licence.body)
            .unwrap()
            .contains("SIL Open Font License")
    );
}

#[test]
fn serve_needs_no_git_repository() {
    let scratch = Scratch::empty();
    let port = free_port().to_string();
    let mut child = scratch
        .command(None, &Clock::fixed(EPOCH))
        .args(["yard", "serve", "--port", &port, "--format", "json"])
        .stdout(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    let started = first_document(&mut child);
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(started["outcome"], "serving", "{started}");
}

#[test]
fn a_port_another_program_holds_exits_1_naming_the_port() {
    let yard = Yard::new();
    let held = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = held.local_addr().unwrap().port().to_string();
    let output = yard.run(&["serve", "--port", &port]);
    assert_eq!(output.status.code(), Some(1));
    let failed = document("serve", &output);
    assert_eq!(failed["errors"][0]["kind"], "port");
    let message = failed["errors"][0]["message"].as_str().unwrap();
    assert!(message.contains(&port), "{message}");
    assert!(message.contains("another program"), "{message}");
    assert!(stderr(&output).contains(&port));
}

#[test]
fn serve_finding_a_page_server_of_its_version_or_a_newer_one_exits_0_and_starts_none() {
    for version in [VERSION, "99.0.0"] {
        let yard = Yard::new();
        let port = free_port();
        let _held = pretend_server(&yard, port, version);
        let output = yard.run(&["serve", "--port", &port.to_string()]);
        assert_eq!(output.status.code(), Some(0), "{version}");
        let running = document("serve", &output);
        assert_eq!(running["outcome"], "running", "{version}");
        assert_eq!(running["version"], version);
        assert!(TcpListener::bind(("127.0.0.1", port)).is_ok(), "{version}");
        assert!(!yard_home(&yard).join("stop.json").exists(), "{version}");
    }
}

#[test]
fn a_second_serve_leaves_the_running_page_server_serving() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let second = document(
        "serve",
        &yard.run(&["serve", "--port", &free_port().to_string()]),
    );
    assert_eq!(second["outcome"], "running");
    assert_eq!(second["port"], served.port);
    assert_eq!(served.get("/api/version").status, 200);
}

#[test]
fn two_serves_that_find_the_port_free_at_once_leave_one_page_server_serving_it() {
    let yard = Yard::new();
    let port = free_port().to_string();
    let mut children: Vec<_> = (0..2)
        .map(|_| yard.spawn(&["serve", "--port", &port]))
        .collect();
    let outcomes: Vec<Value> = children.iter_mut().map(first_document).collect();
    for child in &mut children {
        let _ = child.kill();
        let _ = child.wait();
    }
    let mut seen: Vec<&str> = outcomes
        .iter()
        .map(|document| document["outcome"].as_str().unwrap())
        .collect();
    seen.sort_unstable();
    assert_eq!(seen, ["running", "serving"], "{outcomes:?}");
}

#[test]
fn each_run_records_in_sessions_json_the_time_the_session_was_heard() {
    let mut yard = Yard::new();
    let _served = Served::start(&yard, &["--session", "s1"]);
    assert_eq!(yard.home_file("sessions.json")["s1"], EPOCH_TEXT);
    yard.clock = Clock::fixed(EPOCH + 5);
    let again = yard.run(&["serve", "--session", "s1"]);
    assert_eq!(document("serve", &again)["outcome"], "running");
    assert_eq!(
        yard.home_file("sessions.json")["s1"],
        "2026-01-01T00:00:05Z"
    );
}

#[test]
fn the_page_server_records_a_session_its_mod_says_is_live_as_heard() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let reply = served
        .post_as_mod("/api/heard", &json!({ "session": "s7" }))
        .json();
    assert_eq!(reply["session"], "s7", "{reply}");
    assert_eq!(yard.home_file("sessions.json")["s7"], EPOCH_TEXT);
}

#[test]
fn each_event_a_session_s_mod_posts_counts_as_hearing_the_session() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let dir = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let served = Served::start(&yard, &["--tree", &dir]);
    let event = json!({
        "session": "s8",
        "dir": dir,
        "at": "2026-01-01T10:00:00Z",
        "kind": "turn.start",
    });
    served.post_as_mod("/api/events", &json!([event]));
    assert_eq!(yard.home_file("sessions.json")["s8"], EPOCH_TEXT);
}

#[test]
fn server_json_gives_the_port_process_id_version_and_formats() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let named = yard.home_file("server.json");
    assert_eq!(named["port"], served.port);
    assert_eq!(named["pid"], served.child.id());
    assert_eq!(named["version"], VERSION);
    assert_eq!(named["formats"], json!(["yardlog/1"]));
}

#[test]
fn the_version_is_reported_without_the_control_token() {
    let yard = Yard::new();
    let served = Served::start(&yard, &[]);
    let version = served.get("/api/version");
    assert_eq!(version.status, 200);
    assert_eq!(version.json()["loft"], VERSION);
    assert_eq!(version.json()["formats"]["reads"], json!(["yardlog/1"]));
}

#[test]
fn a_newer_serve_asks_an_older_page_server_to_stop_and_serves_once_it_has() {
    let yard = Yard::new();
    let port = free_port();
    let older = pretend_server(&yard, port, "0.0.1");
    let mut newer = yard.spawn(&["serve", "--port", &port.to_string()]);
    let stop = yard_home(&yard).join("stop.json");
    assert!(within(Duration::from_secs(5), || stop.exists()));
    let request: Value = serde_json::from_slice(&fs::read(&stop).unwrap()).unwrap();
    assert_eq!(request["version"], VERSION);
    // The older one still holds its lock: the newer one does not serve the port yet.
    std::thread::sleep(Duration::from_millis(300));
    assert!(TcpListener::bind(("127.0.0.1", port)).is_ok());
    assert_eq!(yard.home_file("server.json")["version"], "0.0.1");
    drop(older);
    let started = first_document(&mut newer);
    assert_eq!(started["outcome"], "serving", "{started}");
    assert_eq!(yard.home_file("server.json")["version"], VERSION);
    assert!(!stop.exists());
    let _ = newer.kill();
    let _ = newer.wait();
}

#[test]
fn a_page_server_asked_to_stop_by_a_newer_loft_exits_within_one_second() {
    let yard = Yard::new();
    let mut served = Served::start(&yard, &[]);
    let stop = yard_home(&yard).join("stop.json");
    fs::write(&stop, json!({ "version": "99.0.0" }).to_string()).unwrap();
    assert!(served.ends_within(Duration::from_secs(1)));
    let held = yard_home(&yard).join("server.lock");
    assert!(!lock::is_held(&held).unwrap());
}

#[test]
fn a_stop_request_of_the_same_or_an_older_version_leaves_the_page_server_serving() {
    let yard = Yard::new();
    let mut served = Served::start(&yard, &[]);
    let stop = yard_home(&yard).join("stop.json");
    for version in [VERSION, "0.0.1"] {
        fs::write(&stop, json!({ "version": version }).to_string()).unwrap();
        assert!(!served.ends_within(Duration::from_millis(800)), "{version}");
        assert_eq!(served.get("/api/version").status, 200, "{version}");
    }
}

#[test]
fn a_page_server_whose_process_ended_is_no_longer_counted_as_running() {
    let mut yard = Yard::new();
    yard.clock = Clock::fixed(EPOCH);
    let mut served = Served::start(&yard, &[]);
    let written = document("write", &yard.run_with(&["write", "--session", "s1"], ASK));
    assert_eq!(written["page"]["available"], true);
    assert_eq!(written["page"]["port"], served.port);
    served.child.kill().unwrap();
    served.child.wait().unwrap();
    let read = yard.read("s1", &[]);
    assert_eq!(read["page"]["available"], false);
    assert!(yard_home(&yard).join("server.json").exists());
}
