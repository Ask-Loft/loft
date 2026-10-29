//! The page in a real browser: `loft yard serve` serves two trees' Yard Logs, and the Playwright
//! suite in `crates/loft-cli/yard-page-tests/browser/` drives a headless Chromium against it,
//! writing to Yard Log as an agent does through the same `loft` while the page is open. It is
//! ignored by default, since it needs Node.js and the browser Playwright installs; run it with
//! `npm ci` and `npx playwright install chromium` done in `yard-page-tests/`, then
//! `cargo nextest run -p loft-cli --run-ignored only -E 'test(/yard_page_browser/)'`.

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::{Map, Value, json};
use software_plant::{Clock, GitScenario};

use crate::served::Served;
use crate::yard::Yard;

/// The Yard Log the page opens on: an active stage with a note, a planned one, and a question
/// sent to the person.
const OPENING: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape the shelf", "status": "active",
   "goal": "The questions that decide the shelf are answered"},
  {"id": "build", "kind": "stage", "title": "Build the shelf", "status": "planned"},
  {"id": "intro", "kind": "note", "stage": "shape", "title": "Where it stands",
   "markdown": "The shelf holds **two** kinds of reader."},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top", "recommended": true, "detail": "Within reach"},
               {"label": "Bottom", "detail": "Out of the way"}]}
]"#;

/// How the suite runs `loft` as `yard`'s agent: the program, its environment and its directory,
/// as the harness starts it, the git simulator and the clock included.
fn driver(yard: &Yard) -> Value {
    let command = yard.scratch.command(Some(&yard.git), &yard.clock);
    let env: Map<String, Value> = command
        .get_envs()
        .map(|(name, value)| {
            let value = value.map_or(Value::Null, |value| json!(value.to_string_lossy()));
            (name.to_string_lossy().into_owned(), value)
        })
        .collect();
    json!({
        "program": command.get_program().to_string_lossy(),
        "cwd": yard.scratch.0.canonicalize().unwrap(),
        "env": env,
    })
}

/// A Yard at the system's time, so that the times the page draws along are the browser's own.
fn at_the_system_time(mut yard: Yard) -> Yard {
    yard.clock = Clock::system();
    yard
}

#[test]
#[ignore = "drives the page in a headless browser through Playwright, which needs Node.js"]
fn the_page_meets_its_criteria_in_a_headless_browser() {
    let mut yard = at_the_system_time(Yard::new());
    // Two Yard Logs carried from other branches, which the tree holds as inherited.
    let own = yard.git.clone();
    for branch in ["old/lamp", "old/desk"] {
        yard.git = GitScenario::clean_on(branch);
        yard.write(
            "s0",
            r#"[{"id": "done", "kind": "stage", "title": "Done", "status": "done"}]"#,
            &[],
        );
    }
    yard.git = own;
    yard.write("s1", OPENING, &["--send"]);
    let other = at_the_system_time(Yard::new());
    // A tree no session registers, whose `loft yard` shares the page server's home, as a session
    // without the mod in another worktree of the machine does.
    let unserved = at_the_system_time(Yard::new());
    let mut unserved_driver = driver(&unserved);
    unserved_driver["env"]["HOME"] = json!(yard.scratch.home());
    let tree = yard.scratch.0.display().to_string();
    let served = Served::start(&yard, &["--tree", &tree]);
    let setup = json!({
        "url": format!("http://127.0.0.1:{}/", served.port),
        "token": served.token(),
        "tree": yard.scratch.0.canonicalize().unwrap(),
        "branch": crate::yard::BRANCH,
        "driver": driver(&yard),
        "other": driver(&other),
        "unserved": unserved_driver,
    });
    let file = yard.scratch.home().join("browser-setup.json");
    fs::write(&file, setup.to_string()).unwrap();
    let tests = Path::new(env!("CARGO_MANIFEST_DIR")).join("yard-page-tests");
    let status = Command::new("npm")
        .args(["run", "browser"])
        .current_dir(&tests)
        .env("YARD_BROWSER_SETUP", &file)
        .status()
        .expect("npm runs the Playwright suite");
    assert!(status.success(), "the Playwright suite failed: {status}");
}
