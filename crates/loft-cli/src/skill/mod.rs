//! The `loft-spec` skill (the Agent Skill Product): its host-neutral source, embedded in `loft`,
//! composed for each agent host and written into a project by `loft skill install` and
//! `loft init`.

mod compose;
mod write;

use std::fmt;
use std::io;
use std::path::Path;

use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;

use compose::Host;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-skill/1";

/// The skill's source, each file by its path under `skill/loft-spec/`, embedded in the binary.
macro_rules! source {
    ($($path:literal),* $(,)?) => {
        &[$(($path, include_str!(concat!("../../skill/loft-spec/", $path)))),*]
    };
}

const SOURCE: &[(&str, &str)] = source![
    "SKILL.md",
    "agents/claude/claude-host.md",
    "agents/claude/frontmatter.yaml",
    "agents/claude/route.md",
    "agents/openai.yaml",
    "references/adopting-an-openspec-project.md",
    "references/changing-a-specified-product.md",
    "references/format.md",
    "references/lint-findings.md",
    "references/new-document.md",
    "references/publishing-and-merging.md",
    "references/reconciling.md",
    "references/reviewing.md",
    "references/the-loop.md",
    "references/writing-rules.md",
];

/// A skill directory that could not be written.
#[derive(Debug)]
pub struct SkillError {
    directory: &'static str,
    error: io::Error,
}

impl fmt::Display for SkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let Self { directory, error } = self;
        write!(formatter, "{directory}: cannot write the skill: {error}")
    }
}

/// Writes each host's package into its directory under `root`, one after the other, and names
/// the directories written.
///
/// # Errors
///
/// A directory that cannot be written; a directory not yet reached is left as it was.
pub fn install(root: &Path) -> Result<Vec<&'static str>, SkillError> {
    let mut written = Vec::new();
    for host in Host::ALL {
        // The source is embedded, and its tests compose it for every host.
        let files = compose::compose(host, SOURCE).expect("the embedded skill composes");
        let directory = host.directory();
        write::replace(&root.join(directory), &files)
            .map_err(|error| SkillError { directory, error })?;
        written.push(directory);
    }
    Ok(written)
}

/// An error of the skill as JSON, of kind `install`.
pub fn error_json(error: &SkillError) -> serde_json::Value {
    json::error("install", &error.to_string(), &[])
}

/// Prints the line for each skill directory written, then asks for them to be committed
/// (Writing the skill).
pub fn report(written: &[&str]) {
    for directory in written {
        outln!("installed {directory}");
    }
    if !written.is_empty() {
        outln!(
            "commit {}, so that every clone's coding agents have the skill",
            written.join(" and ")
        );
    }
}

/// `loft skill install`: writes the skill into the current directory and names the directories
/// it wrote.
pub fn run(root: &Path, as_json: bool) -> Finished {
    match install(root) {
        Ok(written) => {
            if as_json {
                let fields = json!({ "skills": written });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                report(&written);
            }
            Finished::success("installed")
        }
        Err(error) => json::stop(SCHEMA, vec![error_json(&error)], &error, as_json),
    }
}
