//! The setup report (The setup report): what `loft init` and `loft skill install` end with. For
//! each piece the agents' hosts need beside `spec/` and the skill, its state and the step that
//! remains: the git repository, Loft's merge driver, the `loft` the hosts' hooks run and the
//! Claude Code plugin. It asks git and writes nothing.

use std::env;
use std::ffi::{OsStr, OsString};
use std::path::Path;

use loftspec_store::StoreError;
use loftspec_store::merge_driver::DRIVER;
use loftspec_store::repository::GitCommand;
use serde_json::{Value, json};

use crate::merge_driver;
use crate::out::outln;
use crate::version::VERSION;

/// Where a check stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    /// The check passed.
    Ok,
    /// A step remains.
    Action,
    /// `loft` cannot tell.
    Unchecked,
}

impl State {
    fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Action => "action",
            Self::Unchecked => "unchecked",
        }
    }
}

/// One check of the report: its name, its state and the step that remains, if any.
#[derive(Debug)]
pub struct Entry {
    check: &'static str,
    state: State,
    step: Option<String>,
}

impl Entry {
    fn ok(check: &'static str) -> Self {
        Self {
            check,
            state: State::Ok,
            step: None,
        }
    }

    fn unchecked(check: &'static str) -> Self {
        Self {
            check,
            state: State::Unchecked,
            step: None,
        }
    }

    fn with(check: &'static str, state: State, step: String) -> Self {
        Self {
            check,
            state,
            step: Some(step),
        }
    }
}

/// The report for the project in `root`, with `plugin` where the command enabled the Claude Code
/// plugin.
pub fn report(root: &Path, plugin: bool) -> Vec<Entry> {
    let git = GitCommand::default();
    let inside = match git.prefix(root) {
        Ok(_) => Some(Entry::ok("git")),
        Err(StoreError::NotARepository(_)) => Some(Entry::with(
            "git",
            State::Action,
            "the merge driver and Yard Log need a git repository; put the project under git, \
             then run `loft merge-driver --install` once in each clone"
                .to_owned(),
        )),
        Err(_) => None,
    };
    let repository = inside
        .as_ref()
        .is_some_and(|entry| entry.state == State::Ok);
    let mut entries = vec![inside.unwrap_or_else(|| Entry::unchecked("git"))];
    if repository {
        entries.push(merge_driver_entry(&git, root));
        entries.push(hooks_loft(root));
    } else {
        entries.push(Entry::unchecked("merge-driver"));
        entries.push(Entry::unchecked("hooks-loft"));
    }
    if plugin {
        entries.push(Entry::with("plugin", State::Unchecked, plugin_step()));
    }
    entries
}

fn merge_driver_entry(git: &GitCommand, root: &Path) -> Entry {
    match git.config_get(root, DRIVER) {
        Ok(Some(_)) => Entry::ok("merge-driver"),
        Ok(None) => Entry::with("merge-driver", State::Action, merge_driver::install_step()),
        Err(_) => Entry::unchecked("merge-driver"),
    }
}

/// The `loft` the hooks run: the first on the search path without the package runners'
/// directories, asked through git as git asks a merge driver's launcher.
fn hooks_loft(root: &Path) -> Entry {
    let path = hooks_path(&env::var_os("PATH").unwrap_or_default());
    let own = format!("loft {VERSION}");
    let found = match GitCommand::on_search_path(path).launcher_version(root, "loft") {
        Ok(Some(version)) if version == own => return Entry::ok("hooks-loft"),
        Ok(Some(version)) => format!("starts {version}"),
        Ok(None) => "has no loft".to_owned(),
        Err(_) => return Entry::unchecked("hooks-loft"),
    };
    let runner = runner(root).map_or_else(String::new, |runner| {
        format!(", or set the Claude Code plugin's loftCommand to `{runner}`")
    });
    Entry::with(
        "hooks-loft",
        State::Action,
        format!(
            "the hooks of the agents' hosts run `loft` from the search path, which {found}, not \
             {own}: install loft {VERSION} there with `npm install --global \
             @ask-loft/loft@{VERSION}` or `cargo install --locked loft-cli --version \
             {VERSION}`{runner}; Codex's hooks run `loft` from the search path alone"
        ),
    )
}

/// `path` without any `node_modules/.bin` directory, where a package runner puts the project's
/// own `loft` and a host started outside the runner does not find it.
fn hooks_path(path: &OsStr) -> OsString {
    let kept = env::split_paths(path).filter(|dir| !dir.ends_with("node_modules/.bin"));
    env::join_paths(kept).unwrap_or_default()
}

/// The command that runs the project's own `loft`, by the lockfile of its package manager.
fn runner(root: &Path) -> Option<&'static str> {
    [
        ("pnpm-lock.yaml", "pnpm exec loft"),
        ("yarn.lock", "yarn loft"),
        ("package-lock.json", merge_driver::NPM_COMMAND),
    ]
    .into_iter()
    .find(|(lockfile, _)| root.join(lockfile).is_file())
    .map(|(_, runner)| runner)
}

fn plugin_step() -> String {
    format!(
        "the project pins the Claude Code plugin loft@ask-loft at v{VERSION}; a machine that added \
         the marketplace ask-loft before keeps its own entry until it is added again: run `claude \
         plugin marketplace remove ask-loft`, `claude plugin marketplace add \
         Ask-Loft/loft@v{VERSION}` and `claude plugin install loft@ask-loft`; `claude plugin \
         list` shows the version installed"
    )
}

/// The report as the JSON document's `setup`.
pub fn json(entries: &[Entry]) -> Value {
    entries
        .iter()
        .map(|entry| {
            let mut value = json!({ "check": entry.check, "state": entry.state.as_str() });
            if let Some(step) = &entry.step {
                value["step"] = json!(step);
            }
            value
        })
        .collect()
}

/// The report in the text output: a line for each step that remains.
pub fn print(entries: &[Entry]) {
    for step in entries.iter().filter_map(|entry| entry.step.as_ref()) {
        outln!("next: {step}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_hooks_search_path_leaves_out_every_node_modules_bin() {
        let path = env::join_paths([
            "/project/node_modules/.bin",
            "/usr/local/bin",
            "/other/node_modules/.bin/",
            "/opt/.bin",
            "/usr/bin",
        ])
        .unwrap();
        let kept: Vec<_> = env::split_paths(&hooks_path(&path)).collect();
        assert_eq!(
            kept,
            ["/usr/local/bin", "/opt/.bin", "/usr/bin"].map(std::path::PathBuf::from)
        );
    }
}
