//! The `loft-spec` skill (the Agent Skill Product): its host-neutral source, embedded in `loft`,
//! composed for each agent host and written into a project by `loft skill install` and
//! `loft init`, which also enable the Claude Code plugin `loft@ask-loft` beside it.

mod compose;
mod settings;
mod write;

use std::fmt;
use std::io;
use std::path::Path;

use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;

use compose::Host;
pub use settings::Enabled;
use settings::SETTINGS;

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
        if *directory == SETTINGS {
            write!(formatter, "{directory}: cannot enable the plugin: {error}")
        } else {
            write!(formatter, "{directory}: cannot write the skill: {error}")
        }
    }
}

/// What writing the skill wrote: the skill directories, and what it did to the settings.
pub struct Installed {
    pub skills: Vec<&'static str>,
    pub settings: Enabled,
}

/// Writes each host's package into its directory under `root`, one after the other, then
/// enables the plugin in `.claude/settings.json`, and names what it wrote.
///
/// # Errors
///
/// A directory or the settings file that cannot be written; one not yet reached is left as it
/// was.
pub fn install(root: &Path) -> Result<Installed, SkillError> {
    let mut written = Vec::new();
    for host in Host::ALL {
        // The source is embedded, and its tests compose it for every host.
        let files = compose::compose(host, SOURCE).expect("the embedded skill composes");
        let directory = host.directory();
        write::replace(&root.join(directory), &files)
            .map_err(|error| SkillError { directory, error })?;
        written.push(directory);
    }
    let settings = settings::enable(root).map_err(|error| SkillError {
        directory: SETTINGS,
        error,
    })?;
    Ok(Installed {
        skills: written,
        settings,
    })
}

/// Whether each host's package of the skill lies under `root`.
pub fn installed(root: &Path) -> bool {
    Host::ALL
        .iter()
        .all(|host| root.join(host.directory()).join("SKILL.md").is_file())
}

/// An error of the skill as JSON, of kind `install`.
pub fn error_json(error: &SkillError) -> serde_json::Value {
    json::error("install", &error.to_string(), &[])
}

/// Prints the line for each skill directory written and for the settings, then asks for what
/// was written to be committed (Writing the skill).
pub fn report(installed: &Installed) {
    for directory in &installed.skills {
        outln!("installed {directory}");
    }
    let mut written: Vec<&str> = installed.skills.clone();
    match installed.settings {
        Enabled::Written => {
            outln!(
                "enabled the Claude Code plugin {} in {SETTINGS}",
                settings::PLUGIN
            );
            written.push(SETTINGS);
        }
        Enabled::Unchanged => {}
        Enabled::NotAnObject => outln!("{}", settings::by_hand()),
    }
    if !installed.skills.is_empty() {
        let last = written.pop().expect("a skill directory was written");
        let listed = if written.is_empty() {
            last.to_owned()
        } else {
            format!("{} and {last}", written.join(", "))
        };
        outln!("commit {listed}, so that every clone's coding agents have the skill");
    }
}

/// `loft skill install`: writes the skill into the current directory and names the directories
/// it wrote.
pub fn run(root: &Path, as_json: bool) -> Finished {
    match install(root) {
        Ok(installed) => {
            if as_json {
                let fields = json!({ "skills": installed.skills });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                report(&installed);
            }
            Finished::success("installed")
        }
        Err(error) => json::stop(SCHEMA, vec![error_json(&error)], &error, as_json),
    }
}
