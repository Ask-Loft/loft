//! The git simulator: the calls Loft makes of git, answered from a scripted repository the way
//! git answers them, byte for byte, and every other call refused.

use std::path::Path;
use std::{env, fs};

use serde::{Deserialize, Serialize};

/// The environment variable that names the scenario file the simulator reads.
pub const SCENARIO_ENV: &str = "SOFTWARE_PLANT_GIT_SCENARIO";

/// What git prints, with exit code 128, in a directory outside any repository.
pub const NOT_A_REPOSITORY: &str =
    "fatal: not a git repository (or any of the parent directories): .git\n";

/// The exit code of a call the simulator does not answer.
pub const UNSCRIPTED: i32 = 2;

/// The environment variable that, set to `0`, stops git taking the locks a call needs only for a
/// side effect, such as the index lock `git status` takes to refresh the index.
pub const OPTIONAL_LOCKS_ENV: &str = "GIT_OPTIONAL_LOCKS";

/// The file, beside the scenario, where the simulator records each lock a call took, one per
/// line.
pub const LOCKS_FILE: &str = "git-locks";

/// The lock `git status` takes in a repository to refresh the index.
pub const INDEX_LOCK: &str = "index.lock";

/// A scripted repository, and the calls that fail instead of answering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitScenario {
    /// The repository the working directory lies in, or none.
    pub repository: Option<Repository>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub failures: Vec<Failure>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Repository {
    /// The working directory's path from the repository's top level: empty at the top level,
    /// `sub/` one directory below it.
    pub prefix: String,
    /// The checked-out branch, or none on a detached HEAD.
    pub branch: Option<String>,
    /// What `git status` reports, each path from the top level.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub status: Vec<Entry>,
}

/// One entry of the porcelain v1 status: its two-letter code and its path, and for a rename or
/// a copy the path it came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Entry {
    pub code: String,
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<String>,
}

/// A call Loft makes of git.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Call {
    /// `git rev-parse --show-prefix`
    ShowPrefix,
    /// `git symbolic-ref --quiet --short HEAD`
    CurrentBranch,
    /// `git status --porcelain=v1 -z --untracked-files=all -- <pathspec>…`
    Status,
}

/// A call that fails with a scripted exit code and message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Failure {
    pub call: Call,
    pub code: i32,
    pub message: String,
}

/// What the simulator prints and how it exits, and the lock the call takes unless optional
/// locks are off.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reply {
    pub code: i32,
    pub stdout: Vec<u8>,
    pub stderr: String,
    pub optional_lock: Option<&'static str>,
}

impl Reply {
    fn ok(stdout: impl Into<Vec<u8>>) -> Self {
        Self {
            code: 0,
            stdout: stdout.into(),
            stderr: String::new(),
            optional_lock: None,
        }
    }

    fn fail(code: i32, stderr: impl Into<String>) -> Self {
        Self {
            code,
            stdout: Vec::new(),
            stderr: stderr.into(),
            optional_lock: None,
        }
    }
}

impl GitScenario {
    /// A clean repository, at its top level, on `branch`.
    #[must_use]
    pub fn clean_on(branch: &str) -> Self {
        Self {
            repository: Some(Repository {
                prefix: String::new(),
                branch: Some(branch.to_owned()),
                status: Vec::new(),
            }),
            failures: Vec::new(),
        }
    }

    /// A directory outside any repository.
    #[must_use]
    pub fn outside_repository() -> Self {
        Self {
            repository: None,
            failures: Vec::new(),
        }
    }

    /// The same repository on a detached HEAD.
    #[must_use]
    pub fn detached(mut self) -> Self {
        self.repository_mut().branch = None;
        self
    }

    /// The same repository with the working directory at `prefix` below its top level.
    #[must_use]
    pub fn below(mut self, prefix: &str) -> Self {
        prefix.clone_into(&mut self.repository_mut().prefix);
        self
    }

    /// The same repository with one more status entry, its path from the top level.
    #[must_use]
    pub fn with(mut self, code: &str, path: &str) -> Self {
        self.repository_mut().status.push(Entry {
            code: code.to_owned(),
            path: path.to_owned(),
            from: None,
        });
        self
    }

    /// The same repository with a staged rename of `from` to `to`.
    #[must_use]
    pub fn renamed(mut self, from: &str, to: &str) -> Self {
        self.repository_mut().status.push(Entry {
            code: "R ".to_owned(),
            path: to.to_owned(),
            from: Some(from.to_owned()),
        });
        self
    }

    /// The same scenario with `call` failing with `code` and `message`.
    #[must_use]
    pub fn failing(mut self, call: Call, code: i32, message: &str) -> Self {
        self.failures.push(Failure {
            call,
            code,
            message: message.to_owned(),
        });
        self
    }

    fn repository_mut(&mut self) -> &mut Repository {
        self.repository
            .as_mut()
            .expect("a scenario inside a repository")
    }

    /// Reads the scenario from the file [`SCENARIO_ENV`] names.
    ///
    /// # Errors
    ///
    /// The variable is unset, or the file cannot be read or parsed.
    pub fn from_env() -> Result<Self, String> {
        let path = env::var_os(SCENARIO_ENV).ok_or_else(|| format!("{SCENARIO_ENV} is not set"))?;
        Self::read(Path::new(&path))
    }

    /// Reads a scenario file.
    ///
    /// # Errors
    ///
    /// The file cannot be read or parsed.
    pub fn read(path: &Path) -> Result<Self, String> {
        let text =
            fs::read_to_string(path).map_err(|error| format!("{}: {error}", path.display()))?;
        serde_json::from_str(&text).map_err(|error| format!("{}: {error}", path.display()))
    }

    /// Writes the scenario to `path`.
    ///
    /// # Panics
    ///
    /// The file cannot be written.
    pub fn write(&self, path: &Path) {
        let text = serde_json::to_string_pretty(self).expect("a scenario serialises");
        fs::write(path, text).expect("the scenario file is written");
    }

    /// Answers one call, given its arguments after `git`.
    #[must_use]
    pub fn answer(&self, args: &[String]) -> Reply {
        let Some((call, pathspecs)) = parse(args) else {
            return Reply::fail(
                UNSCRIPTED,
                format!(
                    "software-plant: unscripted git call: git {}\n",
                    args.join(" ")
                ),
            );
        };
        if let Some(failure) = self.failures.iter().find(|failure| failure.call == call) {
            return Reply::fail(failure.code, failure.message.clone());
        }
        let Some(repository) = &self.repository else {
            return Reply::fail(128, NOT_A_REPOSITORY);
        };
        match call {
            Call::ShowPrefix => Reply::ok(format!("{}\n", repository.prefix)),
            Call::CurrentBranch => match &repository.branch {
                Some(branch) => Reply::ok(format!("{branch}\n")),
                None => Reply::fail(1, ""),
            },
            Call::Status => Reply {
                optional_lock: Some(INDEX_LOCK),
                ..Reply::ok(repository.porcelain(&pathspecs))
            },
        }
    }
}

impl Repository {
    /// The porcelain v1 `-z` records of the entries under `pathspecs`: tracked changes first,
    /// then untracked files, each group sorted by path, as git writes them.
    fn porcelain(&self, pathspecs: &[&str]) -> Vec<u8> {
        let scopes: Vec<String> = pathspecs
            .iter()
            .map(|pathspec| scope(&self.prefix, pathspec))
            .collect();
        let mut entries: Vec<&Entry> = self
            .status
            .iter()
            .filter(|entry| scopes.iter().any(|scope| within(&entry.path, scope)))
            .collect();
        entries.sort_by_key(|entry| (entry.code == "??", entry.path.as_str()));
        let mut out = Vec::new();
        for entry in entries {
            out.extend_from_slice(format!("{} {}\0", entry.code, entry.path).as_bytes());
            if let Some(from) = &entry.from {
                out.extend_from_slice(format!("{from}\0").as_bytes());
            }
        }
        out
    }
}

/// A pathspec relative to the working directory, as a path from the top level.
fn scope(prefix: &str, pathspec: &str) -> String {
    let pathspec = pathspec.trim_end_matches('/');
    if pathspec == "." {
        prefix.trim_end_matches('/').to_owned()
    } else {
        format!("{prefix}{pathspec}")
    }
}

fn within(path: &str, scope: &str) -> bool {
    scope.is_empty()
        || path == scope
        || path
            .strip_prefix(scope)
            .is_some_and(|rest| rest.starts_with('/'))
}

/// The call the arguments make and its pathspecs, after any leading `-C <directory>` pairs, or
/// `None` for a call the simulator does not answer.
fn parse(args: &[String]) -> Option<(Call, Vec<&str>)> {
    let mut rest: Vec<&str> = args.iter().map(String::as_str).collect();
    while rest.first() == Some(&"-C") && rest.len() >= 2 {
        rest.drain(..2);
    }
    match rest.as_slice() {
        ["rev-parse", "--show-prefix"] => Some((Call::ShowPrefix, Vec::new())),
        ["symbolic-ref", "--quiet", "--short", "HEAD"] => Some((Call::CurrentBranch, Vec::new())),
        [
            "status",
            "--porcelain=v1",
            "-z",
            "--untracked-files=all",
            "--",
            pathspecs @ ..,
        ] if !pathspecs.is_empty() => Some((Call::Status, pathspecs.to_vec())),
        _ => None,
    }
}
