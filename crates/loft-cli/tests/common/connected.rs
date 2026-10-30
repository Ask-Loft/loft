//! A connected repository for the tests of `loft publish` through a remote: its published
//! history, the remote that holds it, and a sign-in to that remote.

use std::fs;
use std::process::Output;

use serde_json::{Value, json};
use software_plant::http::Request;
use software_plant::remote::{Remote, RemoteScenario};
use software_plant::{BrowserScenario, Clock, GitScenario};

use super::remote::{History, history, with_criterion};
use super::signed_in::{self, document, granted, keep, name_remote, repository};
use super::{EPOCH, Scratch, tree};

/// The key of the criterion each change adds.
pub const ADDED: &str = "01a18e40-0000-7000-8000-000000000001";

/// A repository that published `count` states, connected to a remote that holds the first `held`
/// of them, with a sign-in to it kept.
pub struct Connected {
    pub scratch: Scratch,
    pub git: GitScenario,
    pub history: History,
    pub remote: Remote,
}

pub fn connected(
    count: u64,
    held: usize,
    scenario: impl FnOnce(&str) -> RemoteScenario,
) -> Connected {
    let history = history(count);
    let scratch = Scratch::with_reading_list();
    let sequences: Vec<usize> = (1..=history.states.len()).collect();
    let git = repository(&scratch, &history, &sequences);
    let remote = serve(
        &scratch,
        scenario(&history.key).holding("Reading List", history.states[..held].to_vec()),
    );
    Connected {
        scratch,
        git,
        history,
        remote,
    }
}

/// Starts a remote that answers as `scenario` and names it as the document's remote, with a
/// sign-in to it kept.
pub fn serve(scratch: &Scratch, scenario: RemoteScenario) -> Remote {
    let remote = Remote::start(scenario);
    name_remote(scratch, Some(&remote.address()), true);
    keep(
        scratch,
        &json!({ remote.address(): granted(&remote, 3600) }),
    );
    remote
}

impl Connected {
    /// The same repository, connected instead to a remote that answers as `scenario`.
    #[must_use]
    pub fn serving(self, scenario: RemoteScenario) -> Self {
        let remote = serve(&self.scratch, scenario);
        Self { remote, ..self }
    }

    /// Adds a criterion to the tree, the change the next publish sends.
    pub fn change(&self) {
        let files = tree(&self.scratch.0);
        for (path, bytes) in with_criterion(&files, ADDED) {
            if files.get(&path) != Some(&bytes) {
                fs::write(self.scratch.file(path.as_str()), bytes).unwrap();
            }
        }
    }

    pub fn publish(&self) -> Output {
        self.publish_with(&[])
    }

    /// Publishes with the variables `env`, in a run whose waits pass at once.
    pub fn publish_with(&self, env: &[(&str, &str)]) -> Output {
        let (mut command, _) = signed_in::command_at(
            &self.scratch,
            BrowserScenario::Absent,
            Some(&self.git),
            &Clock::fixed(EPOCH).passing(),
        );
        for (name, value) in env {
            command.env(name, value);
        }
        command
            .args(["publish", "--format", "json"])
            .output()
            .unwrap()
    }

    /// The method and path of each request to the documents, in order.
    pub fn calls(&self) -> Vec<String> {
        self.remote
            .requests()
            .into_iter()
            .filter(|request| request.path.starts_with("/documents/"))
            .map(|request| format!("{} {}", request.method, request.path))
            .collect()
    }

    /// Each publish request, in order.
    pub fn publish_requests(&self) -> Vec<Request> {
        self.remote
            .requests()
            .into_iter()
            .filter(|request| request.path.ends_with("/publish"))
            .collect()
    }

    /// The body of the first publish request.
    pub fn sent(&self) -> Value {
        let request = self
            .publish_requests()
            .into_iter()
            .next()
            .expect("a publish request");
        serde_json::from_slice(&request.body).unwrap()
    }

    pub fn lock(&self) -> Value {
        serde_json::from_slice(&fs::read(self.scratch.0.join("loft.lock.json")).unwrap()).unwrap()
    }

    pub fn message(output: &Output) -> String {
        document(output)["errors"][0]["message"]
            .as_str()
            .unwrap_or_default()
            .to_owned()
    }
}

/// The paths of a delta's list `name`.
pub fn paths(sent: &Value, name: &str) -> Vec<String> {
    sent[name]
        .as_array()
        .unwrap()
        .iter()
        .map(|file| file["path"].as_str().or(file.as_str()).unwrap().to_owned())
        .collect()
}
