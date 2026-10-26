//! The Yard contract belongs to every host, including callers with identical native ids.

use serde_json::json;
use software_plant::Clock;

use crate::common::{EPOCH, stderr};
use crate::yard::{ASK, BRANCH, Yard, block, document};
use crate::yard_wait::{send, wait};

const OTHER: &[&str] = &["--host", "other-agent"];

#[test]
fn two_hosts_with_the_same_session_own_separate_stages_and_submissions() {
    let yard = Yard::new();
    yard.write("same", ASK, &["--send"]);
    let other = ASK
        .replace("shape", "other-shape")
        .replace("q1", "other-question");
    let written = yard.write("same", &other, &["--host", "other-agent", "--send"]);
    assert_eq!(written["outcome"], "written", "{written}");
    let read = yard.read("same", OTHER);
    assert_eq!(
        block(&read, "other-question")["version"]["act"]["by"]["host"],
        "other-agent"
    );
    assert_eq!(read["submissions"].as_array().unwrap().len(), 2);
    assert_eq!(read["stages"][0]["owner"]["host"], "claude-code");
    assert_eq!(read["stages"][1]["owner"]["host"], "other-agent");
}

#[test]
fn read_revisions_and_continuations_are_separate_for_identical_host_session_ids() {
    let yard = Yard::new();
    yard.write("same", ASK, &[]);
    yard.read("same", &["--mark-read"]);
    yard.write("same", ASK, &[]);
    yard.read("same", &["--host", "other-agent", "--mark-read"]);
    for host in ["claude-code", "other-agent"] {
        let output = yard.run(&[
            "continue",
            "--session",
            "next",
            "--from",
            "same",
            "--host",
            host,
        ]);
        assert_eq!(output.status.code(), Some(0), "{}", stderr(&output));
    }
    assert_eq!(yard.read("next", &[])["lastRead"], 1);
    assert_eq!(yard.read("next", OTHER)["lastRead"], 2);
    assert_eq!(
        yard.home_file("continuations.json"),
        json!({"next": "same", "other-agent~next": "same"})
    );
    let before = std::fs::read(yard.folder(BRANCH).join("yardlog.yaml")).unwrap();
    yard.read("next", OTHER);
    assert_eq!(
        std::fs::read(yard.folder(BRANCH).join("yardlog.yaml")).unwrap(),
        before
    );
}

#[test]
fn invalid_host_identifiers_are_usage_errors_without_writing_a_log() {
    let yard = Yard::new();
    for host in ["", "../outside", "Uppercase", "bad~host", "-host"] {
        let output = yard.run_with(&["write", "--session", "same", "--host", host], ASK);
        assert_eq!(output.status.code(), Some(2), "{host}: {}", stderr(&output));
        assert!(stderr(&output).contains("--host"));
        assert!(!yard.folder(BRANCH).exists());
    }
}

#[test]
fn a_host_takes_over_a_stopped_sender_even_when_the_native_session_id_matches() {
    let yard = Yard::new();
    let key = send(&yard, "same");
    let (code, taken) = wait(
        &yard,
        "same",
        &key,
        &["--host", "other-agent", "--timeout", "0"],
    );
    assert_eq!(code, Some(0));
    assert_eq!(taken["takenUp"], true);
    let read = yard.read("same", OTHER);
    assert_eq!(read["submissions"][0]["agent"]["host"], "other-agent");
}

#[test]
fn a_live_sender_or_its_continuation_prevents_cross_host_takeover() {
    let yard = Yard::new();
    let key = send(&yard, "same");
    let home = yard.scratch.home().join(".yard");
    std::fs::create_dir_all(&home).unwrap();
    std::fs::write(
        home.join("sessions.json"),
        r#"{"same":"2026-01-01T00:00:00Z"}"#,
    )
    .unwrap();
    let (code, live) = wait(
        &yard,
        "same",
        &key,
        &["--host", "other-agent", "--timeout", "0"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(live["errors"][0]["kind"], "heard");
    yard.run(&["continue", "--session", "next", "--from", "same"]);
    let mut yard = yard;
    yard.clock = Clock::fixed(EPOCH + 61);
    let (code, continued) = wait(
        &yard,
        "same",
        &key,
        &["--host", "other-agent", "--timeout", "0"],
    );
    assert_eq!(code, Some(1));
    assert_eq!(continued["errors"][0]["kind"], "continued");
}

#[test]
fn another_hosts_continuation_does_not_continue_the_sender() {
    let yard = Yard::new();
    let key = send(&yard, "same");
    yard.run(&[
        "continue",
        "--session",
        "next",
        "--from",
        "same",
        "--host",
        "other-agent",
    ]);
    let (code, taken) = wait(
        &yard,
        "next",
        &key,
        &["--host", "other-agent", "--timeout", "0"],
    );
    assert_eq!(code, Some(0));
    assert_eq!(taken["takenUp"], true);
}

#[test]
fn a_subagent_cannot_take_over_another_hosts_submission() {
    let yard = Yard::new();
    let key = send(&yard, "same");
    let (code, refused) = wait(
        &yard,
        "same",
        &key,
        &[
            "--host",
            "other-agent",
            "--agent",
            "sub",
            "--agent-type",
            "reader",
            "--timeout",
            "0",
        ],
    );
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "not-sender");
    assert_eq!(document("read", &yard.run(&["read"]))["revision"], 1);
}

#[test]
fn events_and_heartbeats_of_identical_sessions_keep_their_hosts_separate() {
    use crate::served::Served;
    use crate::yard_serve_events::{event, post, session_file, tokens};
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    yard.write(
        "s1",
        &ASK.replace("shape", "other-shape").replace("q1", "other-q"),
        OTHER,
    );
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let result = post(
        &served,
        &[
            event(&yard, "request", 1, &json!({"tokens": tokens(1, 2, 3, 4)})),
            event(
                &yard,
                "request",
                2,
                &json!({"host": "other-agent", "tokens": tokens(10, 20, 30, 40)}),
            ),
        ],
    );
    assert_eq!(result["recorded"], 2);
    let read = yard.read("s1", &[]);
    let own = read["stages"][0]["key"].as_str().unwrap();
    let other = read["stages"][1]["key"].as_str().unwrap();
    let first = session_file(&yard, "tokens", "s1").unwrap();
    let second = session_file(&yard, "tokens", "other-agent~s1").unwrap();
    assert!(first.contains(&format!("stage: {own}")));
    assert!(!first.contains(&format!("stage: {other}")));
    assert!(second.contains(&format!("stage: {other}")));
    assert!(!second.contains(&format!("stage: {own}")));
    assert!(second.contains("session: s1"));
    let activity = session_file(&yard, "activity", "other-agent~s1").unwrap();
    assert!(activity.contains("host: other-agent"), "{activity}");
    let result = served.post_as_mod(
        "/api/heard",
        &json!({"host":"other-agent", "session":"beat"}),
    );
    assert_eq!(result.status, 200);
    assert!(
        yard.home_file("sessions.json")
            .get("other-agent~beat")
            .is_some()
    );
    assert!(
        yard.scratch
            .home()
            .join(".yard/events/other-agent~s1.jsonl")
            .exists()
    );
    drop(served);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    post(
        &served,
        &[event(
            &yard,
            "request",
            3,
            &json!({"host": "other-agent", "tokens": tokens(10, 20, 30, 40)}),
        )],
    );
    let second = session_file(&yard, "tokens", "other-agent~s1").unwrap();
    assert!(second.contains("input: 20"), "{second}");
    assert_eq!(session_file(&yard, "tokens", "s1").unwrap(), first);
}

#[test]
fn a_lease_prevents_a_second_host_from_taking_over() {
    let yard = Yard::new();
    let key = send(&yard, "same");
    let mut first = yard.spawn(&[
        "wait",
        "--session",
        "same",
        "--host",
        "other-agent",
        "--submission",
        &key,
    ]);
    let holder = yard
        .scratch
        .0
        .join(".git/yard/leases")
        .join(format!("{key}.holder"));
    let started = std::time::Instant::now();
    while !holder.exists() {
        if started.elapsed().as_secs() >= 10 {
            first.kill().ok();
            first.wait().ok();
            panic!("the first host did not acquire its lease");
        }
        std::thread::sleep(std::time::Duration::from_millis(20));
    }
    let (code, refused) = wait(&yard, "second", &key, &["--timeout", "0"]);
    first.kill().unwrap();
    first.wait().unwrap();
    assert_eq!(code, Some(1));
    assert_eq!(refused["errors"][0]["kind"], "leased");
}

#[test]
fn every_session_bearing_command_accepts_an_explicit_host() {
    let yard = Yard::new();
    let key = "01a16502-f6d2-7666-adad-3403c0a92870";
    let cases: &[(&str, &[&str])] = &[
        ("read", &[]),
        ("write", &["--input", "/missing-yard-input"]),
        ("wait", &["--submission", key, "--timeout", "0"]),
        ("withdraw", &["--submission", key]),
        ("close", &[]),
        ("published", &["--branch", "feature"]),
        ("answer", &["--input", "/missing-yard-input"]),
        ("continue", &["--from", "before"]),
        ("serve", &[]),
    ];
    for (command, extra) in cases {
        // An invalid session stops serve before it opens a listener.
        let session = if *command == "serve" { "." } else { "same" };
        let mut args = vec![*command, "--session", session, "--host", "other-agent"];
        args.extend_from_slice(extra);
        let output = yard.run(&args);
        assert!(
            !stderr(&output).contains("unexpected argument"),
            "{command}: {}",
            stderr(&output)
        );
        assert!(output.status.code().is_some(), "{command}");
    }
}
