//! The files of this repository that the guards over its workflows read, and a workflow's jobs
//! as those guards read them: one trimmed line each, comments and blank lines left out.

use std::fs;
use std::ops::Deref;
use std::path::Path;

use crate::common::workspace;

/// The file at `path`, from the root of the repository unless it is absolute.
pub fn read(path: impl AsRef<Path>) -> String {
    let path = workspace().join(path);
    fs::read_to_string(&path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))
}

/// The trimmed lines below the line `start` of `text`, while each is blank or opens with
/// `indent`, leaving out comments.
pub fn block<'a>(text: &'a str, start: &str, indent: &str) -> Vec<&'a str> {
    text.lines()
        .skip_while(|line| *line != start)
        .skip(1)
        .take_while(|line| line.is_empty() || line.starts_with(indent))
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

/// One job of a workflow: its name and its trimmed lines, which it dereferences to.
#[derive(Debug)]
pub struct Job<'a> {
    pub name: String,
    lines: Vec<&'a str>,
}

/// The job `name` of `workflow`.
pub fn job<'a>(workflow: &'a str, name: &str) -> Job<'a> {
    let lines = block(workflow, &format!("  {name}:"), "   ");
    assert!(!lines.is_empty(), "the workflow has no job `{name}`");
    Job {
        name: name.to_owned(),
        lines,
    }
}

impl<'a> Deref for Job<'a> {
    type Target = [&'a str];

    fn deref(&self) -> &Self::Target {
        &self.lines
    }
}

impl Job<'_> {
    /// Where `line` stands among the job's lines.
    pub fn position(&self, line: &str) -> usize {
        self.iter()
            .position(|each| *each == line)
            .unwrap_or_else(|| panic!("the job `{}` lacks `{line}`", self.name))
    }

    /// Whether the job runs in an environment.
    pub fn has_environment(&self) -> bool {
        self.iter().any(|line| line.starts_with("environment:"))
    }

    /// What each of the job's steps runs, a block of several lines joined by newlines.
    pub fn runs(&self) -> Vec<String> {
        let mut runs = Vec::new();
        let mut lines = self.iter().peekable();
        while let Some(line) = lines.next() {
            let Some(run) = line
                .strip_prefix("- run: ")
                .or_else(|| line.strip_prefix("run: "))
            else {
                continue;
            };
            if run != "|" {
                runs.push(run.to_owned());
                continue;
            }
            let mut script = Vec::new();
            while let Some(next) = lines.next_if(|next| !next.starts_with("- ")) {
                script.push(*next);
            }
            runs.push(script.join("\n"));
        }
        runs
    }
}
