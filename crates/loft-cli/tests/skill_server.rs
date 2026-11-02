//! `loft skill install --server`: the skill package a server publishes, installed beside
//! `loft-spec` under the remote simulator that serves it, and each package, manifest, name and
//! origin it refuses with nothing written.

use std::fs;
use std::process::Output;
use std::time::{Duration, Instant};

use flate2::Compression;
use flate2::write::GzEncoder;
use serde_json::{Value, json};
use software_plant::Clock;
use software_plant::http::{Answer, Request, Responder, Response, Server, refusing_url};
use software_plant::remote::wire::sha256_hex;
use software_plant::remote::{Remote, RemoteScenario};
use tar::{Builder, EntryType, Header};

use crate::common::{EPOCH, Scratch, stderr, stdout};

const KEY: &str = "01a18e30-0000-7000-8000-000000000021";
const SKILL: &[u8] = b"---\nname: review\n---\nReview a change.\n";
const GUIDE: &[u8] = b"Read the diff first.\n";

/// An entry of a package: its path as the archive holds it, its type, its bytes, and the target
/// of a link.
struct Entry<'a> {
    path: &'a str,
    kind: EntryType,
    bytes: &'a [u8],
    link: Option<&'a str>,
}

const fn file<'a>(path: &'a str, bytes: &'a [u8]) -> Entry<'a> {
    Entry {
        path,
        kind: EntryType::Regular,
        bytes,
        link: None,
    }
}

const fn other<'a>(path: &'a str, kind: EntryType, link: Option<&'a str>) -> Entry<'a> {
    Entry {
        path,
        kind,
        bytes: b"",
        link,
    }
}

/// A gzip-compressed tar archive of `entries`, each path written as given, unchecked, as a
/// hostile server could write it.
fn package(entries: &[Entry]) -> Vec<u8> {
    let mut builder = Builder::new(GzEncoder::new(Vec::new(), Compression::default()));
    for entry in entries {
        let mut header = Header::new_ustar();
        header.as_old_mut().name[..entry.path.len()].copy_from_slice(entry.path.as_bytes());
        header.set_entry_type(entry.kind);
        header.set_mode(0o644);
        header.set_size(entry.bytes.len() as u64);
        if let Some(link) = entry.link {
            header.as_old_mut().linkname[..link.len()].copy_from_slice(link.as_bytes());
        }
        header.set_cksum();
        builder.append(&header, entry.bytes).unwrap();
    }
    builder.into_inner().unwrap().finish().unwrap()
}

/// The skill `review`: its top directory as `./`, its router, and a reference in a directory.
fn review() -> Vec<u8> {
    package(&[
        other("./", EntryType::Directory, None),
        file("./SKILL.md", SKILL),
        other("./references/", EntryType::Directory, None),
        file("./references/guide.md", GUIDE),
    ])
}

fn serving(name: &str, package: Vec<u8>) -> Remote {
    Remote::start(RemoteScenario::accepting(KEY).skill(name, "1.0.0", package))
}

/// `loft skill install --server <origin>` with `more` arguments, as JSON.
fn install(scratch: &Scratch, origin: &str, more: &[&str]) -> Output {
    let mut args = vec!["skill", "install", "--server", origin, "--format", "json"];
    args.extend(more);
    scratch.loft(&args)
}

/// Asserts that `output` refused with exit 1, naming each of `named`, and that the empty
/// `scratch` was left empty.
fn assert_refused(output: &Output, scratch: &Scratch, named: &[&str]) {
    assert_eq!(output.status.code(), Some(1), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["outcome"], "error");
    assert_eq!(document["errors"][0]["kind"], "install");
    let error = stderr(output);
    for name in named {
        assert!(error.contains(name), "`{name}` in {error}");
    }
    assert_nothing_written(scratch);
}

fn assert_nothing_written(scratch: &Scratch) {
    let entries: Vec<_> = fs::read_dir(&scratch.0).unwrap().collect();
    assert!(entries.is_empty(), "{entries:?}");
}

#[test]
fn a_server_s_skill_is_written_from_the_top_beside_loft_spec_for_each_host_enabling_no_plugin() {
    let remote = serving("review", review());
    let scratch = Scratch::empty();
    let own = scratch.0.join(".claude/skills/loft-spec/SKILL.md");
    fs::create_dir_all(own.parent().unwrap()).unwrap();
    fs::write(&own, "the project's own").unwrap();
    let output = install(&scratch, remote.origin(), &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        document,
        json!({
            "schema": "loft-skill/2",
            "outcome": "installed",
            "skills": [".claude/skills/review", ".agents/skills/review"],
            "errors": [],
        }),
        "no setup report"
    );
    for host in [".claude/skills/review", ".agents/skills/review"] {
        assert_eq!(
            fs::read(scratch.0.join(format!("{host}/SKILL.md"))).unwrap(),
            SKILL
        );
        let guide = scratch.0.join(format!("{host}/references/guide.md"));
        assert_eq!(fs::read(guide).unwrap(), GUIDE);
    }
    assert_eq!(fs::read_to_string(&own).unwrap(), "the project's own");
    assert!(
        !scratch.0.join(".claude/settings.json").exists(),
        "no plugin"
    );
    let requests = remote.requests();
    let paths: Vec<&str> = requests.iter().map(|r| r.path.as_str()).collect();
    assert_eq!(
        paths,
        ["/.well-known/loft-skill.json", "/skills/review.tar.gz"]
    );
}

#[test]
fn the_text_of_a_server_s_skill_names_each_directory_written_and_gives_no_setup_report() {
    let remote = serving("review", review());
    let scratch = Scratch::empty();
    let output = scratch.loft(&["skill", "install", "--server", remote.origin()]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert_eq!(
        stdout(&output),
        "installed .claude/skills/review\ninstalled .agents/skills/review\ncommit \
         .claude/skills/review and .agents/skills/review, so that every clone's coding agents \
         have the skill\n"
    );
}

#[test]
fn with_host_a_server_s_skill_is_written_only_for_the_host_selected() {
    let remote = serving("review", review());
    let scratch = Scratch::empty();
    let output = install(&scratch, remote.origin(), &["--host", "codex"]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    let document: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(document["skills"], json!([".agents/skills/review"]));
    assert!(scratch.0.join(".agents/skills/review/SKILL.md").is_file());
    assert!(!scratch.0.join(".claude").exists());
}

#[test]
fn a_reinstall_replaces_a_server_s_skill_directory_whole() {
    let scratch = Scratch::empty();
    let first = serving("review", review());
    assert_eq!(
        install(&scratch, first.origin(), &[]).status.code(),
        Some(0)
    );
    let second = serving("review", package(&[file("SKILL.md", b"version two\n")]));
    let output = install(&scratch, second.origin(), &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    for host in [".claude/skills", ".agents/skills"] {
        let skill = scratch.0.join(format!("{host}/review"));
        assert_eq!(fs::read(skill.join("SKILL.md")).unwrap(), b"version two\n");
        assert!(!skill.join("references").exists(), "the old files are gone");
        let names: Vec<String> = fs::read_dir(scratch.0.join(host))
            .unwrap()
            .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
            .collect();
        assert_eq!(names, ["review"], "nothing staged is left beside it");
    }
}

#[test]
fn the_server_is_reached_whatever_the_telemetry_and_update_check_switches_say() {
    let remote = serving("review", review());
    for switches in ["off", "on"] {
        let scratch = Scratch::empty();
        let mut command = scratch.command(None, &Clock::fixed(EPOCH));
        for variable in ["LOFT_TELEMETRY", "LOFT_UPDATE_CHECK"] {
            if switches == "off" {
                command.env(variable, "off");
            } else {
                command.env_remove(variable);
            }
        }
        let output = command
            .args(["skill", "install", "--server", remote.origin()])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(0), "{switches}: {output:?}");
        assert!(scratch.0.join(".agents/skills/review/SKILL.md").is_file());
    }
    assert_eq!(remote.requests().len(), 4);
}

#[test]
fn a_package_other_than_the_manifest_names_is_refused_naming_both_digests_and_writes_nothing() {
    let served = review();
    let remote = Remote::start(
        RemoteScenario::accepting(KEY)
            .skill("review", "1.0.0", served.clone())
            .tampering(),
    );
    let mut received = served.clone();
    received.push(0);
    let scratch = Scratch::empty();
    let output = install(&scratch, remote.origin(), &[]);
    assert_refused(
        &output,
        &scratch,
        &[&sha256_hex(&served), &sha256_hex(&received)],
    );
}

#[test]
fn an_entry_outside_its_skill_directory_is_refused_naming_it_and_nothing_is_written() {
    for (entry, said) in [
        (file("/etc/review.md", GUIDE), "has an absolute path"),
        (
            file("references/../../escape.md", GUIDE),
            "has a `..` segment",
        ),
        (
            other("link.md", EntryType::Symlink, Some("SKILL.md")),
            "is a link",
        ),
        (
            other("hard.md", EntryType::Link, Some("SKILL.md")),
            "is a link",
        ),
        (
            other("pipe", EntryType::Fifo, None),
            "is neither a file nor a directory",
        ),
    ] {
        let path = entry.path;
        let remote = serving("review", package(&[file("SKILL.md", SKILL), entry]));
        let scratch = Scratch::empty();
        let output = install(&scratch, remote.origin(), &[]);
        assert_refused(&output, &scratch, &[&format!("entry {path} {said}")]);
    }
}

#[test]
fn a_name_that_is_loft_spec_or_not_a_slug_is_refused_naming_it_and_a_slug_is_accepted() {
    for name in ["loft-spec", "Review", "re/view", "re_view", ""] {
        let remote = serving(name, review());
        let scratch = Scratch::empty();
        let output = install(&scratch, remote.origin(), &[]);
        assert_refused(&output, &scratch, &[&format!("`{name}`")]);
        assert_eq!(remote.requests().len(), 1, "{name}: no package is fetched");
    }
    let remote = serving("team-review-2", review());
    let scratch = Scratch::empty();
    let output = install(&scratch, remote.origin(), &[]);
    assert_eq!(output.status.code(), Some(0), "{output:?}");
    assert!(
        scratch
            .0
            .join(".claude/skills/team-review-2/SKILL.md")
            .is_file()
    );
}

#[test]
fn an_http_origin_off_the_loopback_interface_is_refused_naming_it_and_never_contacted() {
    // An address reserved for documentation: a connection to it would wait out the time limit.
    let origin = "http://192.0.2.1";
    let scratch = Scratch::empty();
    let started = Instant::now();
    let output = install(&scratch, origin, &[]);
    assert!(
        started.elapsed() < Duration::from_secs(10),
        "never contacted"
    );
    assert_refused(&output, &scratch, &[origin]);
}

#[test]
fn an_origin_with_a_path_a_query_or_a_fragment_is_a_usage_error_naming_it_and_opening_nothing() {
    let remote = serving("review", review());
    for suffix in ["/skills", "/?q=1", "#top"] {
        let given = format!("{}{suffix}", remote.origin());
        let scratch = Scratch::empty();
        let output = install(&scratch, &given, &[]);
        assert_eq!(output.status.code(), Some(2), "{given}: {output:?}");
        assert!(stderr(&output).contains(&given), "{}", stderr(&output));
        assert_nothing_written(&scratch);
    }
    assert!(remote.requests().is_empty(), "no connection is opened");
}

/// A server whose manifest is not JSON.
struct Garbled;

impl Responder for Garbled {
    fn answer(&mut self, _: &Request) -> Answer {
        Response {
            status: 200,
            headers: Vec::new(),
            body: b"{ not a manifest".to_vec(),
        }
        .into()
    }
}

#[test]
fn a_server_that_refuses_is_silent_or_gives_no_manifest_that_reads_is_refused_naming_it() {
    let silent = Remote::start(RemoteScenario::silent(KEY));
    let garbled = Server::start(Garbled);
    let without = Remote::start(RemoteScenario::accepting(KEY));
    for origin in [
        refusing_url(),
        silent.origin().to_owned(),
        garbled.url(),
        without.origin().to_owned(),
    ] {
        let scratch = Scratch::empty();
        let output = scratch
            .command(None, &Clock::fixed(EPOCH))
            .args(["skill", "install", "--server", &origin, "--format", "json"])
            .env("LOFT_REMOTE_TIMEOUT", "1")
            .output()
            .unwrap();
        assert_refused(&output, &scratch, &[&origin]);
    }
}

#[test]
fn no_plugin_beside_server_is_a_usage_error() {
    let scratch = Scratch::empty();
    let output = scratch.loft(&[
        "skill",
        "install",
        "--server",
        "https://example.org",
        "--no-plugin",
    ]);
    assert_eq!(output.status.code(), Some(2), "{output:?}");
    assert_nothing_written(&scratch);
}
