//! What the page server adds to a branch's Yard Log for the page to draw it: each block's
//! Markdown rendered to HTML by `loft`, with any raw HTML shown as text, the activity and the
//! token totals each session keeps in Yard Log's folder, and every submission sent; each reply
//! held to the read schema.

use std::fs;

use serde_json::{Value, json};

use crate::common::workspace;
use crate::schema;
use crate::served::Served;
use crate::yard::{BRANCH, Yard, block};

const NOTES: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "n1", "kind": "note", "stage": "shape", "markdown": "The shelf holds **two** kinds."},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "context": "See [the plan](https://example.com/plan).", "options": [{"label": "Top"}, {"label": "Bottom"}]},
  {"id": "p1", "kind": "approval", "stage": "shape", "title": "Ship it?",
   "markdown": "```mermaid\ngraph LR; A-->B\n```\n"}
]"#;

/// `yard`'s Yard Log as the page reads it from `served`, held to the read schema.
fn log(yard: &Yard, served: &Served) -> Value {
    log_of(yard, served, BRANCH)
}

/// `yard`'s Yard Log of `branch` as the page reads it from `served`, held to the read schema.
fn log_of(yard: &Yard, served: &Served, branch: &str) -> Value {
    let tree = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let reply = served.get(&format!("/api/log?tree={tree}&branch={branch}"));
    let schema = fs::read_to_string(workspace().join("schemas/yard-read.json")).unwrap();
    schema::conforming(&reply.body, &schema)
}

fn served(yard: &Yard) -> Served {
    Served::start(yard, &["--tree", &yard.scratch.0.display().to_string()])
}

#[test]
fn the_page_reads_each_block_s_markdown_rendered_to_html_by_loft() {
    let yard = Yard::new();
    yard.write("s1", NOTES, &[]);
    let served = served(&yard);
    let read = log(&yard, &served);
    let note = &block(&read, "n1")["version"]["body"];
    assert_eq!(
        note["html"],
        "<p>The shelf holds <strong>two</strong> kinds.</p>\n"
    );
    assert_eq!(note["markdown"], "The shelf holds **two** kinds.");
    let question = &block(&read, "q1")["version"]["body"];
    assert_eq!(
        question["contextHtml"],
        "<p>See <a href=\"https://example.com/plan\">the plan</a>.</p>\n"
    );
    let approval = &block(&read, "p1")["versions"][0]["body"];
    assert!(
        approval["html"]
            .as_str()
            .unwrap()
            .contains("<code class=\"language-mermaid\">"),
        "{approval}"
    );
}

#[test]
fn the_page_reads_every_markdown_field_of_a_stage_and_its_blocks_rendered_by_loft() {
    let yard = Yard::new();
    yard.write(
        "s1",
        r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active", "goal": "The *store* reads"},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top", "detail": "The **top** one", "recommended": true}, {"label": "Bottom"}]},
  {"id": "d1", "kind": "decision", "stage": "shape", "title": "Top", "consequence": "Ships `b84b656`"},
  {"id": "t1", "kind": "task", "stage": "shape", "title": "Fix", "state": "done", "detail": "Also `5ddbbfe`"}
]"#,
        &[],
    );
    let served = served(&yard);
    let read = log(&yard, &served);
    let body = |id: &str| block(&read, id)["version"]["body"].clone();
    assert_eq!(
        body("shape")["goalHtml"],
        "<p>The <em>store</em> reads</p>\n"
    );
    assert_eq!(
        body("q1")["options"][0]["detailHtml"],
        "<p>The <strong>top</strong> one</p>\n"
    );
    assert_eq!(
        body("d1")["consequenceHtml"],
        "<p>Ships <code>b84b656</code></p>\n"
    );
    assert_eq!(
        body("t1")["detailHtml"],
        "<p>Also <code>5ddbbfe</code></p>\n"
    );
}

#[test]
fn html_that_a_block_s_markdown_holds_reaches_the_page_as_text() {
    let yard = Yard::new();
    yard.write("s1", NOTES, &[]);
    // Raw HTML enters Yard Log only through its files, as by a hand edit or a merge.
    let folder = yard.folder(BRANCH);
    let stage = fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .find(|path| path.file_name().unwrap() != "yardlog.yaml" && path.is_file())
        .unwrap();
    let text = fs::read_to_string(&stage).unwrap();
    let edited = text.replacen("**two**", "<img src=x onerror=alert(1)>", 1);
    assert_ne!(edited, text);
    fs::write(&stage, edited).unwrap();
    let served = served(&yard);
    let html = block(&log(&yard, &served), "n1")["version"]["body"]["html"].clone();
    let html = html.as_str().unwrap();
    assert!(!html.contains("<img"), "{html}");
    assert!(
        html.contains("&lt;img src=x onerror=alert(1)&gt;"),
        "{html}"
    );
}

#[test]
fn the_page_reads_the_activity_and_token_totals_each_session_keeps() {
    let yard = Yard::new();
    yard.write("s1", NOTES, &[]);
    let served = served(&yard);
    let dir = yard.scratch.0.canonicalize().unwrap().display().to_string();
    let event = |kind: &str, second: u32, more: Value| {
        let mut event = json!({
            "session": "s1",
            "dir": dir,
            "at": format!("2026-01-01T10:00:{second:02}Z"),
            "kind": kind,
        });
        for (name, value) in more.as_object().unwrap() {
            event[name] = value.clone();
        }
        event
    };
    let tokens = json!({ "input": 10, "cacheRead": 100, "cacheWrite": 5, "output": 20 });
    let events = json!([
        event("turn.start", 1, json!({})),
        event("request", 5, json!({ "model": "opus", "tokens": tokens })),
        event("turn.end", 9, json!({})),
    ]);
    assert_eq!(served.post_as_mod("/api/events", &events).status, 200);
    let read = log(&yard, &served);
    let activity = read["activity"].as_array().unwrap();
    assert_eq!(activity.len(), 1, "{read}");
    assert_eq!(activity[0]["session"], "s1");
    assert_eq!(activity[0]["turns"][0]["started"], "2026-01-01T10:00:01Z");
    assert_eq!(activity[0]["turns"][0]["ended"], "2026-01-01T10:00:09Z");
    let totals = read["tokens"].as_array().unwrap();
    assert_eq!(totals.len(), 1, "{read}");
    assert_eq!(totals[0]["tokens"]["input"], 10);
    assert_eq!(totals[0]["tokens"]["cacheRead"], 100);
    assert_eq!(totals[0]["agents"][0]["model"], "opus");
}

#[test]
fn a_yard_log_with_no_session_files_reads_with_no_activity_and_no_totals() {
    let yard = Yard::new();
    yard.write("s1", NOTES, &[]);
    let served = served(&yard);
    let read = log(&yard, &served);
    assert_eq!(read["activity"], json!([]));
    assert_eq!(read["tokens"], json!([]));
}

#[test]
fn the_page_reads_every_submission_sent_with_its_changes() {
    let yard = Yard::new();
    yard.write("s1", NOTES, &["--send"]);
    yard.answer("s1", r#"[{"block": "q1", "labels": ["Top"]}]"#);
    let served = served(&yard);
    let read = log(&yard, &served);
    let sent = read["sent"].as_array().unwrap();
    assert_eq!(sent.len(), 1, "{read}");
    assert_eq!(sent[0]["state"], "open", "the approval still waits");
    assert_eq!(sent[0]["agent"]["session"], "s1");
    assert_eq!(sent[0]["changes"][0]["what"], "sent");
    let blocks: Vec<&str> = sent[0]["blocks"]
        .as_array()
        .unwrap()
        .iter()
        .map(|key| key.as_str().unwrap())
        .collect();
    assert!(
        blocks.contains(&block(&read, "q1")["key"].as_str().unwrap()),
        "{read}"
    );
    assert!(
        blocks.contains(&block(&read, "p1")["key"].as_str().unwrap()),
        "{read}"
    );
}

#[test]
fn the_listing_marks_a_repository_s_main_worktree_and_no_linked_one() {
    let main = Yard::new();
    main.write("s1", NOTES, &[]);
    let listing = served(&main).get("/api/trees").json();
    let tree = &listing["repositories"][0]["trees"][0];
    assert_eq!(tree["main"], json!(true), "{listing}");

    let mut linked = Yard::new();
    let common = linked.scratch.0.join("repository/.git");
    linked.git = linked.git.clone().common_dir(
        &common.to_string_lossy(),
        &common.join("worktrees/wt").to_string_lossy(),
    );
    linked.write("s1", NOTES, &[]);
    let listing = served(&linked).get("/api/trees").json();
    let tree = &listing["repositories"][0]["trees"][0];
    assert_eq!(tree["main"], json!(false), "{listing}");
}

#[test]
fn the_page_reads_each_loft_block_with_its_report() {
    let yard = Yard::with_spec();
    let view = r#"[{"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
        {"id": "v1", "kind": "loft", "stage": "shape", "view": "spec-change"}]"#;
    yard.write("s1", view, &[]);
    let served = served(&yard);
    let read = log_of(&yard, &served, "feature");
    let body = &block(&read, "v1")["version"]["body"];
    assert!(body["report"]["status"].is_object(), "{body}");
    assert!(body.get("reportBytes").is_none(), "{body}");
}
