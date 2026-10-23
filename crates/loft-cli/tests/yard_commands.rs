//! The rules every `loft yard` command follows: one JSON document held to its own schema, the
//! base branch, the clock, the page's address learned without a connection, the tags on the
//! commands' Features, and a usage error that names its argument and changes nothing.

use std::fs;
use std::net::TcpListener;

use serde_json::{Value, json};
use software_plant::Clock;
use yardlog_store::lock;

use crate::commands::{COMMANDS, fixture};
use crate::common::{EPOCH, stderr, workspace};
use crate::conformance::tagged;
use crate::help;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, EPOCH_TEXT, Yard, block, document};

/// The entries of the yard commands in the command table.
fn yard_entries() -> Vec<crate::commands::Entry> {
    COMMANDS
        .into_iter()
        .filter(|(_, args, ..)| args[0] == "yard")
        .collect()
}

#[test]
fn every_yard_command_has_one_entry_in_the_commands() {
    let mut listed = help::yard_commands();
    listed.sort();
    let mut entries: Vec<&str> = yard_entries().iter().map(|(_, args, ..)| args[1]).collect();
    entries.sort_unstable();
    assert_eq!(listed, entries);
}

#[test]
fn each_yard_command_prints_one_document_its_own_schema_holds() {
    for (slug, args, kind, schema) in yard_entries() {
        assert_eq!(schema, format!("yard-{}", args[1]), "{slug}");
        let scratch = fixture(kind);
        let output = crate::commands::command(&scratch, kind)
            .args(args)
            .args(["--format", "json"])
            .output()
            .unwrap();
        let document = document(args[1], &output);
        for field in ["schema", "outcome", "errors"] {
            assert!(document.get(field).is_some(), "{slug}: {field}");
        }
        assert_eq!(
            document["schema"],
            format!("loft-{schema}/1").as_str(),
            "{slug}"
        );
    }
}

#[test]
fn a_refusal_is_listed_among_the_errors_and_exits_1() {
    let yard = Yard::new();
    let output = yard.run(&["close", "--session", "s1"]);
    let refused = document("close", &output);
    assert_eq!(output.status.code(), Some(1));
    assert_eq!(refused["outcome"], "refused");
    assert_eq!(refused["errors"][0]["kind"], "absent");
}

#[test]
fn yard_log_opened_without_base_records_develop_as_its_base_branch() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &[]);
    assert_eq!(yard.read("s1", &[])["yardLog"]["base"], "develop");
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding("develop", &root, "1111111");
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "merge");
}

#[test]
fn the_base_branch_is_recorded_by_the_write_that_opens_yard_log_and_kept_after() {
    let mut yard = Yard::new();
    yard.write("s1", ASK, &["--base", "main"]);
    let later = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "x"}]"#;
    let written = yard.write("s1", later, &["--base", "trunk"]);
    assert_eq!(written["outcome"], "written", "{written}");
    assert_eq!(yard.read("s1", &[])["yardLog"]["base"], "main");
    let root = format!(".yard/{BRANCH}/yardlog.yaml");
    yard.git = yard.git.clone().holding("develop", &root, "1111111");
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "open");
    yard.git = yard.git.clone().holding("main", &root, "2222222");
    assert_eq!(yard.read("s1", &[])["yardLog"]["closedBy"], "merge");
}

#[test]
fn no_yard_command_but_write_takes_base() {
    let yard = Yard::new();
    for args in [
        &["read", "--session", "s1", "--base", "main"][..],
        &["close", "--session", "s1", "--base", "main"],
    ] {
        let output = yard.run(args);
        assert_eq!(output.status.code(), Some(2), "{args:?}");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(stderr.contains("--base"), "{stderr}");
    }
}

#[test]
fn no_yard_command_sends_a_command_event_or_asks_for_an_update() {
    let commands = tagged("yard-command");
    assert!(commands.len() >= 7, "{commands:?}");
    for tag in ["sends-telemetry", "checks-for-updates"] {
        let listing = tagged(tag);
        for slug in &commands {
            assert!(!listing.contains(slug), "{slug} lists `{tag}`");
        }
    }
}

#[test]
fn each_yard_command_has_a_feature_that_lists_yard_command() {
    let commands = tagged("yard-command");
    for (slug, ..) in yard_entries() {
        assert!(commands.contains(&slug.to_owned()), "{slug}");
    }
}

#[test]
fn each_yard_command_but_serve_and_protocol_lists_needs_repository() {
    let needing = tagged("needs-repository");
    for (slug, args, ..) in yard_entries() {
        let needs = !["serve", "protocol"].contains(&args[1]);
        assert_eq!(needing.contains(&slug.to_owned()), needs, "{slug}");
    }
}

#[test]
fn a_run_with_source_date_epoch_dates_its_acts_by_that_time() {
    let mut yard = Yard::new();
    yard.clock = Clock::fixed(EPOCH + 61);
    yard.write("s1", ASK, &[]);
    let read = yard.read("s1", &[]);
    assert_eq!(read["yardLog"]["opened"]["at"], "2026-01-01T00:01:01Z");
    assert_eq!(
        block(&read, "q1")["version"]["act"]["at"],
        "2026-01-01T00:01:01Z"
    );
    let other = Yard::new();
    other.write("s1", ASK, &[]);
    let read = other.read("s1", &[]);
    assert_eq!(block(&read, "q1")["version"]["act"]["at"], EPOCH_TEXT);
}

/// Writes `~/.yard/server.json` naming a server on `port`.
fn name_server(yard: &Yard, port: u16) {
    let dir = yard.scratch.home().join(".yard");
    fs::create_dir_all(&dir).unwrap();
    let server = json!({ "port": port, "pid": 1, "version": "2.2.1", "formats": ["yardlog/1"] });
    fs::write(dir.join("server.json"), server.to_string()).unwrap();
}

#[test]
fn the_address_is_that_of_the_page_server_server_json_names_with_the_tree_and_the_branch() {
    let yard = Yard::new();
    name_server(&yard, 7500);
    let held = lock::lock(&yard.scratch.home().join(".yard/server.lock")).unwrap();
    let written = yard.write("s1", ASK, &[]);
    drop(held);
    let tree = written["tree"].as_str().unwrap().replace('/', "%2F");
    assert_eq!(written["page"]["available"], true);
    assert_eq!(
        written["page"]["url"],
        format!("http://127.0.0.1:7500/#tree={tree}&branch=feat%2Fshelf").as_str()
    );
}

#[test]
fn where_no_running_page_server_is_named_the_result_says_the_page_is_unavailable() {
    let yard = Yard::new();
    let written = yard.write("s1", ASK, &[]);
    assert_eq!(written["page"], json!({ "available": false, "port": 7411 }));
    // A file a stopped server left names no running one: no process holds its lock.
    name_server(&yard, 7500);
    fs::write(yard.scratch.home().join(".yard/server.lock"), "").unwrap();
    let read = yard.read("s1", &["--port", "7600"]);
    assert_eq!(read["page"], json!({ "available": false, "port": 7600 }));
}

#[test]
fn a_command_learns_whether_a_page_server_runs_without_opening_a_connection() {
    let page = fs::read_to_string(workspace().join("crates/loft-cli/src/yard/page.rs")).unwrap();
    for connection in ["TcpStream", "std::net", "ureq", "http::"] {
        assert!(!page.contains(connection), "{connection}");
    }
    assert!(page.contains("lock::is_held"));
}

#[test]
fn a_command_whose_port_another_program_holds_still_does_its_work() {
    let other = TcpListener::bind("127.0.0.1:0").unwrap();
    let port = other.local_addr().unwrap().port().to_string();
    let yard = Yard::new();
    let output = yard.run_with(&["write", "--session", "s1", "--port", &port], ASK);
    let written = document("write", &output);
    assert_eq!(written["outcome"], "written");
    assert_eq!(written["page"]["port"].to_string(), port);
}

#[test]
fn upgrade_run_without_session_is_not_refused_for_it() {
    let yard = Yard::new();
    let output = yard.run(&["upgrade"]);
    assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    assert_eq!(document("upgrade", &output)["outcome"], "upgraded");
}

#[test]
fn an_argument_that_does_not_match_the_input_exits_2_names_it_and_changes_no_file() {
    let yard = Yard::new();
    let before = snapshot(&yard.scratch.0);
    let missing = yard.run(&["write", "--session", "s1", "--input", "no-such.json"]);
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("--input"), "{}", stderr(&missing));
    let malformed = yard.run_with(&["write", "--session", "s1"], "[{");
    assert_eq!(malformed.status.code(), Some(2));
    assert!(stderr(&malformed).contains("--input"));
    let unknown = yard.run(&["read", "--revision", "3"]);
    assert_eq!(unknown.status.code(), Some(2));
    assert!(stderr(&unknown).contains("--revision"));
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

/// The fields a schema describes, each as `<schema>/<pointer to its properties>/<name>`.
fn fields(schema: &Value, at: &str, found: &mut Vec<String>) {
    match schema {
        Value::Object(map) => {
            if let Some(Value::Object(properties)) = map.get("properties") {
                let named = properties
                    .keys()
                    .map(|name| format!("{at}/properties/{name}"));
                found.extend(named);
            }
            for (name, value) in map {
                fields(value, &format!("{at}/{name}"), found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                fields(item, &format!("{at}/{index}"), found);
            }
        }
        _ => {}
    }
}

/// Every field of every yard command's schema.
pub fn schema_fields() -> Vec<String> {
    let mut found = Vec::new();
    for (_, args, ..) in yard_entries() {
        let name = format!("yard-{}", args[1]);
        let text = fs::read_to_string(workspace().join(format!("schemas/{name}.json"))).unwrap();
        fields(&serde_json::from_str(&text).unwrap(), &name, &mut found);
    }
    found
}
