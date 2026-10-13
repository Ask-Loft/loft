//! The `loft-spec` skill (the Agent Skill Product): its host-neutral source, embedded in `loft`,
//! composed for each agent host and written into a project by `loft skill install` and
//! `loft init`.

mod compose;
mod write;

use std::fmt;
use std::path::Path;
use std::process::ExitCode;

use serde_json::json;

use crate::json;

pub use compose::Host;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-skill/1";

/// The skill's source, each file by its path under `skill/loft-spec/`.
const SOURCE: [(&str, &str); 13] = [
    ("SKILL.md", include_str!("../../skill/loft-spec/SKILL.md")),
    (
        "agents/claude/claude-host.md",
        include_str!("../../skill/loft-spec/agents/claude/claude-host.md"),
    ),
    (
        "agents/claude/frontmatter.yaml",
        include_str!("../../skill/loft-spec/agents/claude/frontmatter.yaml"),
    ),
    (
        "agents/claude/route.md",
        include_str!("../../skill/loft-spec/agents/claude/route.md"),
    ),
    (
        "agents/openai.yaml",
        include_str!("../../skill/loft-spec/agents/openai.yaml"),
    ),
    (
        "references/changing-a-specified-product.md",
        include_str!("../../skill/loft-spec/references/changing-a-specified-product.md"),
    ),
    (
        "references/format.md",
        include_str!("../../skill/loft-spec/references/format.md"),
    ),
    (
        "references/lint-findings.md",
        include_str!("../../skill/loft-spec/references/lint-findings.md"),
    ),
    (
        "references/new-document.md",
        include_str!("../../skill/loft-spec/references/new-document.md"),
    ),
    (
        "references/publishing-and-merging.md",
        include_str!("../../skill/loft-spec/references/publishing-and-merging.md"),
    ),
    (
        "references/reviewing.md",
        include_str!("../../skill/loft-spec/references/reviewing.md"),
    ),
    (
        "references/the-loop.md",
        include_str!("../../skill/loft-spec/references/the-loop.md"),
    ),
    (
        "references/writing-rules.md",
        include_str!("../../skill/loft-spec/references/writing-rules.md"),
    ),
];

/// Why the skill could not be written: its source does not compose, or a directory cannot be
/// written.
#[derive(Debug)]
pub enum SkillError {
    Compose(compose::ComposeError),
    Write {
        directory: &'static str,
        error: std::io::Error,
    },
}

impl fmt::Display for SkillError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Compose(error) => write!(formatter, "the skill does not compose: {error}"),
            Self::Write { directory, error } => {
                write!(formatter, "{directory}: cannot write the skill: {error}")
            }
        }
    }
}

/// Writes each host's package into its directory under `root`, one after the other, and names
/// the directories written.
///
/// # Errors
///
/// The source does not compose, or a directory cannot be written; a directory not yet reached is
/// left as it was.
pub fn install(root: &Path) -> Result<Vec<&'static str>, SkillError> {
    let mut written = Vec::new();
    for host in Host::ALL {
        let files = compose::compose(host, &SOURCE).map_err(SkillError::Compose)?;
        let directory = host.directory();
        write::replace(&root.join(directory), &files)
            .map_err(|error| SkillError::Write { directory, error })?;
        written.push(directory);
    }
    Ok(written)
}

/// An error of the skill as JSON, of kind `skill`.
pub fn error_json(error: &SkillError) -> serde_json::Value {
    json::error("skill", &error.to_string(), &[])
}

/// `loft skill install`: writes the skill into the current directory and names the directories
/// it wrote.
pub fn run(root: &Path, as_json: bool) -> ExitCode {
    match install(root) {
        Ok(written) => {
            if as_json {
                let fields = json!({ "skills": written });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                for directory in written {
                    println!("installed {directory}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            if as_json {
                json::print(&json::failure(SCHEMA, vec![error_json(&error)]));
            }
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
