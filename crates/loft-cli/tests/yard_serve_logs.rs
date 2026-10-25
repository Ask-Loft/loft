//! What the page server serves of the trees registered with it: the trees kept in
//! `~/.yard/trees.json`, each branch's Yard Log in each, the current branch's first and those
//! carried from another branch after it, to read only; the person's answers and decisions sent
//! from the page; and the changes it tells open pages of.

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpStream;
use std::path::Path;
use std::time::Duration;

use serde_json::{Value, json};
use software_plant::{Clock, GitScenario};

use crate::common::{EPOCH, Scratch};
use crate::served::Served;
use crate::snapshot::snapshot;
use crate::yard::{ASK, BRANCH, Yard, block};

/// The directory of `scratch`'s tree, as the page server names it.
pub fn tree(scratch: &Scratch) -> String {
    scratch.0.canonicalize().unwrap().display().to_string()
}

/// `yard`, whose tree holds the Yard Log of `main` as well as its own branch's: one carried from
/// the branch it was cut from.
pub fn with_inherited() -> Yard {
    let mut yard = Yard::on("main");
    yard.write("s1", ASK, &["--send"]);
    yard.git = GitScenario::clean_on(BRANCH)
        .with_config("user.name", "Ada Reader")
        .with_config("user.email", "ada@example.com");
    yard.write("s2", ASK, &["--send"]);
    yard
}

/// The page's answer of `labels` to `q1` of `branch`'s Yard Log.
fn answer(yard: &Yard, branch: &str, labels: &[&str]) -> Value {
    json!({
        "tree": tree(&yard.scratch),
        "branch": branch,
        "answers": [{ "block": "q1", "labels": labels }],
    })
}

#[test]
fn the_trees_served_are_kept_in_trees_json_once_each_and_served_again_by_a_later_page_server() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let dir = yard.scratch.0.display().to_string();
    {
        let _served = Served::start(&yard, &["--tree", &dir]);
        let again = yard.run(&["serve", "--tree", &dir]);
        assert_eq!(again.status.code(), Some(0));
    }
    assert_eq!(
        yard.home_file("trees.json")["trees"],
        json!([tree(&yard.scratch)])
    );
    let later = Served::start(&yard, &[]);
    let listing = later.get("/api/trees").json();
    let trees = &listing["repositories"][0]["trees"];
    assert_eq!(trees.as_array().unwrap().len(), 1, "{listing}");
    assert_eq!(trees[0]["tree"], tree(&yard.scratch).as_str());
}

#[test]
fn trees_of_different_repositories_are_served_by_one_page_server() {
    let first = Yard::new();
    first.write("s1", ASK, &[]);
    let second = Yard::new();
    second.write("s1", ASK, &[]);
    let served = Served::start(&first, &["--tree", &first.scratch.0.display().to_string()]);
    let other = second.scratch.0.display().to_string();
    assert_eq!(
        served
            .post_as_mod("/api/register", &json!({ "tree": other }))
            .status,
        200
    );
    let listing = served.get("/api/trees").json();
    let repositories = listing["repositories"].as_array().unwrap();
    assert_eq!(repositories.len(), 2, "{listing}");
    let named: Vec<&str> = repositories
        .iter()
        .map(|repository| repository["repository"].as_str().unwrap())
        .collect();
    assert!(named.contains(&tree(&first.scratch).as_str()), "{named:?}");
    assert!(named.contains(&tree(&second.scratch).as_str()), "{named:?}");
}

#[test]
fn a_tree_that_no_longer_exists_is_dropped_from_those_served() {
    let yard = Yard::new();
    let gone = Scratch::empty();
    let served = Served::start(&yard, &["--tree", &gone.0.display().to_string()]);
    let path = gone.0.clone();
    drop(gone);
    assert!(!Path::new(&path).exists());
    let listing = served.get("/api/trees").json();
    assert_eq!(listing["repositories"], json!([]));
    // `trees.json` is read without its lock and pruned at the next registration.
    let own = tree(&yard.scratch);
    served.post_as_mod("/api/register", &json!({ "tree": own }));
    assert_eq!(yard.home_file("trees.json")["trees"], json!([own]));
}

#[test]
fn the_page_server_takes_no_answer_for_an_inherited_yard_log() {
    let yard = with_inherited();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let before = snapshot(&yard.scratch.0);
    let refused = served.post_as_page("/api/answer", &answer(&yard, "main", &["Top"]));
    assert_eq!(refused.status, 409);
    assert_eq!(refused.json()["errors"][0]["kind"], "inherited");
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
    let read = served.get(&format!(
        "/api/log?tree={}&branch=main",
        tree(&yard.scratch)
    ));
    assert_eq!(read.status, 200);
    assert_eq!(read.json()["inherited"], true);
    assert_eq!(read.json()["yardLog"]["branch"], "main");
}

#[test]
fn a_branch_s_yard_log_reads_as_loft_yard_read_gives_it_with_every_version() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let read = served
        .get(&format!(
            "/api/log?tree={}&branch={BRANCH}",
            tree(&yard.scratch)
        ))
        .json();
    assert_eq!(read["schema"], "loft-yard-read/1");
    assert_eq!(read["inherited"], false);
    assert_eq!(block(&read, "q1")["versions"].as_array().unwrap().len(), 1);
    let command = yard.read("s1", &[]);
    assert_eq!(read["revision"], command["revision"]);
    assert_eq!(read["submissions"], command["submissions"]);
}

#[test]
fn answers_sent_from_the_page_are_recorded_at_once_with_the_person_and_the_channel_page() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let before = yard.read("s1", &[])["revision"].as_u64().unwrap();
    let answered = served.post_as_page("/api/answer", &answer(&yard, BRANCH, &["Top"]));
    assert_eq!(answered.status, 200);
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], before + 1);
    let recorded = &block(&read, "q1")["answer"];
    assert_eq!(recorded["channel"], "page");
    assert_eq!(recorded["act"]["by"]["name"], "Ada Reader");
    assert_eq!(recorded["act"]["by"]["email"], "ada@example.com");
    let key = recorded["key"].as_str().unwrap();
    assert_ne!(key, block(&read, "q1")["key"].as_str().unwrap());
    assert!(
        answered.json()["recorded"]
            .as_array()
            .unwrap()
            .iter()
            .any(|recorded| recorded["key"] == block(&read, "q1")["key"]),
        "the answer is recorded by the key of the block it answers"
    );
}

#[test]
fn an_answer_given_elsewhere_first_is_kept_and_the_page_s_later_one_refused() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Bottom"]}]"#);
    let later = served.post_as_page("/api/answer", &answer(&yard, BRANCH, &["Top"]));
    let document = later.json();
    assert_eq!(document["outcome"], "answered elsewhere", "{document}");
    assert_eq!(document["elsewhere"][0]["channel"], "terminal");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "q1")["answer"]["labels"], json!(["Bottom"]));
}

/// Two questions, an approval and a plan, sent.
const ASKED: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top"}, {"label": "Bottom"}]},
  {"id": "q2", "kind": "question", "stage": "shape", "prompt": "Which spine?", "header": "Spine",
   "options": [{"label": "Red"}, {"label": "Blue"}]},
  {"id": "go", "kind": "approval", "stage": "shape", "title": "Order it", "markdown": "Order it."},
  {"id": "how", "kind": "plan", "stage": "shape", "title": "The plan", "markdown": "1. Order"}
]"#;

/// The page's send of `answers` for the branch's Yard Log.
fn send(yard: &Yard, served: &Served, answers: &Value) -> Value {
    let sent = json!({ "tree": tree(&yard.scratch), "branch": BRANCH, "answers": answers });
    served.post_as_page("/api/answer", &sent).json()
}

#[test]
fn the_other_answers_of_a_send_are_recorded_when_one_was_answered_elsewhere_first() {
    let yard = Yard::new();
    yard.write("s1", ASKED, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Bottom"]}]"#);
    let answers = json!([
        { "block": "q1", "labels": ["Top"] },
        { "block": "q2", "labels": ["Red"] },
    ]);
    let document = send(&yard, &served, &answers);
    assert_eq!(document["outcome"], "answered elsewhere", "{document}");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "q1")["answer"]["labels"], json!(["Bottom"]));
    assert_eq!(block(&read, "q2")["answer"]["labels"], json!(["Red"]));
    assert_eq!(block(&read, "q2")["answer"]["channel"], "page");
}

#[test]
fn an_approval_and_a_plan_decided_on_the_page_record_their_outcome_with_the_person() {
    let yard = Yard::new();
    yard.write("s1", ASKED, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let before = yard.read("s1", &[])["revision"].as_u64().unwrap();
    send(
        &yard,
        &served,
        &json!([{ "block": "go", "verdict": "approved" }]),
    );
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], before + 1);
    let approved = &block(&read, "go")["outcome"];
    assert_eq!(approved["verdict"], "approved");
    assert_eq!(approved["channel"], "page");
    assert_eq!(approved["act"]["by"]["email"], "ada@example.com");
    let reason = "Order fewer.";
    send(
        &yard,
        &served,
        &json!([{ "block": "how", "verdict": "rejected", "reason": reason }]),
    );
    let read = yard.read("s1", &[]);
    assert_eq!(read["revision"], before + 2);
    assert_eq!(block(&read, "how")["outcome"]["verdict"], "rejected");
    assert_eq!(block(&read, "how")["outcome"]["reason"], reason);
}

#[test]
fn a_block_decided_elsewhere_first_keeps_its_first_outcome() {
    let yard = Yard::new();
    yard.write("s1", ASKED, &["--send"]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    yard.answer(
        "s1",
        r#"[{"block": "go", "verdict": "rejected", "reason": "Not yet."}]"#,
    );
    let document = send(
        &yard,
        &served,
        &json!([{ "block": "go", "verdict": "approved" }]),
    );
    assert_eq!(document["outcome"], "answered elsewhere", "{document}");
    assert_eq!(document["elsewhere"][0]["channel"], "terminal");
    let read = yard.read("s1", &[]);
    assert_eq!(block(&read, "go")["outcome"]["verdict"], "rejected");
}

#[test]
fn a_yard_log_of_a_newer_format_is_listed_as_such_and_takes_no_answer() {
    let yard = Yard::new();
    yard.write("s1", ASK, &["--send"]);
    let root = yard.folder(BRANCH).join("yardlog.yaml");
    let text = fs::read_to_string(&root).unwrap();
    fs::write(
        &root,
        text.replace("format: yardlog/1", "format: yardlog/9"),
    )
    .unwrap();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let listing = served.get("/api/trees").json();
    let log = &listing["repositories"][0]["trees"][0]["logs"][0];
    assert_eq!(log["newer"], true, "{listing}");
    assert_eq!(log["format"], "yardlog/9");
    let before = snapshot(&yard.scratch.0);
    let refused = served.post_as_page("/api/answer", &answer(&yard, BRANCH, &["Top"]));
    assert_eq!(refused.status, 409);
    assert!(snapshot(&yard.scratch.0) == before, "a file changed");
}

/// Reads the stream until an event named `name` arrives, and says whether one did.
pub fn stream_until(reader: &mut impl BufRead, name: &str) -> bool {
    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => return false,
            Ok(_) if line.trim_end() == format!("event: {name}") => return true,
            Ok(_) => {}
        }
    }
}

#[test]
fn a_change_written_by_a_loft_yard_command_reaches_open_pages_within_two_seconds() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let mut stream = TcpStream::connect(("127.0.0.1", served.port)).unwrap();
    stream
        .write_all(
            format!(
                "GET /api/stream HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n\r\n",
                served.port
            )
            .as_bytes(),
        )
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    assert!(stream_until(&mut reader, "hello"));
    // Let the page server take note of the files as they are before the change.
    std::thread::sleep(Duration::from_millis(600));
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Seen."}]"#;
    yard.write("s1", note, &[]);
    assert!(
        stream_until(&mut reader, "change"),
        "no change within 2 seconds"
    );
}

#[test]
fn a_change_that_git_makes_reaches_open_pages() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let mut stream = TcpStream::connect(("127.0.0.1", served.port)).unwrap();
    stream
        .write_all(
            format!(
                "GET /api/stream HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n\r\n",
                served.port
            )
            .as_bytes(),
        )
        .unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    assert!(stream_until(&mut reader, "hello"));
    std::thread::sleep(Duration::from_millis(600));
    // A checkout of another branch takes the branch's Yard Log out of the tree.
    fs::remove_dir_all(yard.folder(BRANCH)).unwrap();
    assert!(
        stream_until(&mut reader, "change"),
        "no change within 2 seconds"
    );
}

#[test]
fn what_waits_on_the_person_is_listed_oldest_first_across_the_trees_served() {
    let mut one = Yard::new();
    let mut two = Yard::new();
    // The trees are listed in the order of their paths; the one listed first is sent to later.
    let (first, second) = if tree(&one.scratch) < tree(&two.scratch) {
        (&mut one, &mut two)
    } else {
        (&mut two, &mut one)
    };
    first.clock = Clock::fixed(EPOCH + 60);
    first.write("s1", ASK, &["--send"]);
    second.write("s1", ASK, &["--send"]);
    let served = served_both(first, second);
    let listing = served.get("/api/trees").json();
    let waits: Vec<&str> = listing["waiting"]
        .as_array()
        .unwrap()
        .iter()
        .map(|wait| wait["tree"].as_str().unwrap())
        .collect();
    assert_eq!(
        waits,
        [tree(&second.scratch), tree(&first.scratch)],
        "{listing}"
    );
}

/// A page server serving `first`'s tree and `second`'s.
fn served_both(first: &Yard, second: &Yard) -> Served {
    let served = Served::start(first, &["--tree", &first.scratch.0.display().to_string()]);
    let other = json!({ "tree": second.scratch.0.display().to_string() });
    assert_eq!(served.post_as_mod("/api/register", &other).status, 200);
    served
}

#[test]
fn a_checkout_that_rewrites_the_tree_s_head_reaches_open_pages() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let head = yard.scratch.0.join(".git/HEAD");
    fs::create_dir_all(head.parent().unwrap()).unwrap();
    fs::write(&head, format!("ref: refs/heads/{BRANCH}\n")).unwrap();
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let mut stream = TcpStream::connect(("127.0.0.1", served.port)).unwrap();
    let request = format!(
        "GET /api/stream HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n\r\n",
        served.port
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    assert!(stream_until(&mut reader, "hello"));
    std::thread::sleep(Duration::from_millis(600));
    fs::write(&head, "ref: refs/heads/another-branch\n").unwrap();
    assert!(
        stream_until(&mut reader, "change"),
        "no change within 2 seconds"
    );
}

#[test]
fn a_change_names_the_branch_whose_yard_log_changed_so_a_page_reads_only_that() {
    let yard = Yard::new();
    yard.write("s1", ASK, &[]);
    let served = Served::start(&yard, &["--tree", &yard.scratch.0.display().to_string()]);
    let mut stream = TcpStream::connect(("127.0.0.1", served.port)).unwrap();
    let request = format!(
        "GET /api/stream HTTP/1.1\r\nhost: 127.0.0.1:{}\r\n\r\n",
        served.port
    );
    stream.write_all(request.as_bytes()).unwrap();
    stream
        .set_read_timeout(Some(Duration::from_secs(2)))
        .unwrap();
    let mut reader = BufReader::new(stream);
    assert!(stream_until(&mut reader, "hello"));
    std::thread::sleep(Duration::from_millis(600));
    let note = r#"[{"id": "n1", "kind": "note", "stage": "shape", "markdown": "Seen."}]"#;
    yard.write("s1", note, &[]);
    assert!(
        stream_until(&mut reader, "change"),
        "no change within 2 seconds"
    );
    let mut line = String::new();
    reader.read_line(&mut line).unwrap();
    let change: Value = serde_json::from_str(line.trim_start_matches("data: ").trim()).unwrap();
    assert_eq!(change["trees"], json!(false), "{change}");
    let logs = change["logs"].as_array().unwrap();
    assert!(
        logs.iter()
            .any(|log| log["branch"] == json!(BRANCH) && log["log"] == json!(true)),
        "{change}"
    );
}
