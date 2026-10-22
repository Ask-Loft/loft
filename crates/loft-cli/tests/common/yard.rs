//! Running `loft yard` commands in a scratch tree, in front of the git simulator and a clock:
//! a repository on a branch, with the person git names, each command's JSON held to its
//! schema, and its input written beside the tree so that the tree holds only what the command
//! writes.

use std::fs;
use std::path::PathBuf;
use std::process::Output;

use serde_json::Value;
use software_plant::{Clock, GitScenario};

use crate::common::{EPOCH, Scratch, workspace};
use crate::schema;

/// The branch the scratch repository is on.
pub const BRANCH: &str = "feat/shelf";

/// A scratch repository for the yard commands.
pub struct Yard {
    pub scratch: Scratch,
    pub git: GitScenario,
    pub clock: Clock,
}

impl Yard {
    /// An empty repository on [`BRANCH`] whose person is named, at [`EPOCH`].
    pub fn new() -> Self {
        Self::on(BRANCH)
    }

    /// An empty repository on `branch` whose person is named, at [`EPOCH`].
    pub fn on(branch: &str) -> Self {
        Self {
            scratch: Scratch::empty(),
            git: GitScenario::clean_on(branch)
                .with_config("user.name", "Ada Reader")
                .with_config("user.email", "ada@example.com"),
            clock: Clock::fixed(EPOCH),
        }
    }

    /// Runs `loft yard` with `args`, printing JSON.
    pub fn run(&self, args: &[&str]) -> Output {
        let mut all = vec!["yard"];
        all.extend_from_slice(args);
        all.extend_from_slice(&["--format", "json"]);
        self.scratch.loft_with(&all, Some(&self.git), &self.clock)
    }

    /// The file beside the tree that holds a command's input.
    fn input(&self, text: &str) -> PathBuf {
        let path = self.scratch.home().join("input.json");
        fs::write(&path, text).unwrap();
        path
    }

    /// Runs `loft yard` with `args` and `--input` naming a file that holds `input`.
    pub fn run_with(&self, args: &[&str], input: &str) -> Output {
        let path = self.input(input);
        let mut all = args.to_vec();
        all.extend_from_slice(&["--input", path.to_str().unwrap()]);
        self.run(&all)
    }

    /// Writes `batch` as session `session`'s main agent, with `extra` arguments.
    pub fn write(&self, session: &str, batch: &str, extra: &[&str]) -> Value {
        let mut args = vec!["write", "--session", session];
        args.extend_from_slice(extra);
        let output = self.run_with(&args, batch);
        document("write", &output)
    }

    /// Reads Yard Log as session `session`'s main agent, with `extra` arguments.
    pub fn read(&self, session: &str, extra: &[&str]) -> Value {
        let mut args = vec!["read", "--session", session];
        args.extend_from_slice(extra);
        document("read", &self.run(&args))
    }

    /// Records the person's answers given in the terminal, for session `session`.
    pub fn answer(&self, session: &str, answers: &str) -> Value {
        document(
            "answer",
            &self.run_with(&["answer", "--session", session], answers),
        )
    }

    /// The folder of the branch's Yard Log.
    pub fn folder(&self, branch: &str) -> PathBuf {
        self.scratch.0.join(".yard").join(branch)
    }

    /// The file `name` of `~/.yard/`, as JSON.
    pub fn home_file(&self, name: &str) -> Value {
        let path = self.scratch.home().join(".yard").join(name);
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }
}

/// The JSON document `output` holds, after asserting that the schema of `loft yard <command>`
/// holds it.
pub fn document(command: &str, output: &Output) -> Value {
    let path = workspace().join(format!("schemas/yard-{command}.json"));
    let schema = fs::read_to_string(path).unwrap();
    schema::conforming(&output.stdout, &schema)
}

/// The block whose id is `id` in a read's result, from any stage.
pub fn block<'a>(read: &'a Value, id: &str) -> &'a Value {
    read["stages"]
        .as_array()
        .unwrap()
        .iter()
        .flat_map(|stage| std::iter::once(stage).chain(stage["blocks"].as_array().unwrap().iter()))
        .find(|block| block["id"] == id)
        .unwrap_or_else(|| panic!("no block `{id}` in {read}"))
}

/// A batch of a stage and one question, `q1`, sent or not.
pub const ASK: &str = r#"[
  {"id": "shape", "kind": "stage", "title": "Shape it", "status": "active"},
  {"id": "q1", "kind": "question", "stage": "shape", "prompt": "Which shelf?", "header": "Shelf",
   "options": [{"label": "Top", "recommended": true}, {"label": "Bottom"}], "ownWords": true}
]"#;

/// [`EPOCH`] as Yard Log writes it.
pub const EPOCH_TEXT: &str = "2026-01-01T00:00:00Z";
