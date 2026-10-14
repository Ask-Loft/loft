//! Composition for a host: the skill's host-neutral source becomes one host's package. The
//! Claude Code package takes the frontmatter fragment, the route bullet and the host reference
//! under `agents/claude/`, and holds no `agents/` directory; the Codex package keeps
//! `agents/openai.yaml` alone. No package carries another host's adaptation.

use std::fmt;

/// An agent host the skill is composed for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Host {
    ClaudeCode,
    Codex,
}

impl Host {
    pub const ALL: [Self; 2] = [Self::ClaudeCode, Self::Codex];

    /// The directory, from the project's root, that the host reads the skill from.
    pub const fn directory(self) -> &'static str {
        match self {
            Self::ClaudeCode => ".claude/skills/loft-spec",
            Self::Codex => ".agents/skills/loft-spec",
        }
    }
}

const CODEX_ADAPTATION: &str = "agents/openai.yaml";
const FRAGMENT: &str = "agents/claude/frontmatter.yaml";
const ROUTE: &str = "agents/claude/route.md";
const HOST_REFERENCE: &str = "agents/claude/claude-host.md";
const COMPOSED_REFERENCE: &str = "references/claude-host.md";
/// The route bullet the host's route follows.
const ANCHOR: &str = "references/the-loop.md";
/// The fields the host-neutral `SKILL.md` owns, which a fragment may not define.
const OWNED: [&str; 2] = ["name", "description"];
/// The width at which the host cuts `description` and `when_to_use` together in its listing.
const LISTING_BUDGET: usize = 1536;

/// Why a source does not compose.
#[derive(Debug, PartialEq, Eq)]
pub enum ComposeError {
    /// A file the composition needs is not in the source.
    Missing(&'static str),
    /// `SKILL.md` does not open with a frontmatter.
    NoFrontmatter,
    /// The fragment defines a field that `SKILL.md` owns.
    Defines(String),
    /// `description` and `when_to_use` together pass the listing budget: their combined length
    /// and the limit.
    Budget { length: usize, limit: usize },
    /// The router does not name the anchor bullet exactly once.
    Anchor(usize),
}

impl fmt::Display for ComposeError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing(path) => write!(formatter, "the source holds no `{path}`"),
            Self::NoFrontmatter => {
                formatter.write_str("`SKILL.md` does not open with a frontmatter")
            }
            Self::Defines(field) => {
                write!(
                    formatter,
                    "the fragment defines `{field}`, which `SKILL.md` owns"
                )
            }
            Self::Budget { length, limit } => write!(
                formatter,
                "`description` and `when_to_use` hold {length} characters together, past the \
                 host's listing limit of {limit}"
            ),
            Self::Anchor(count) => write!(
                formatter,
                "the router names `{ANCHOR}` {count} times, not once"
            ),
        }
    }
}

/// A package's files, by path, in the source's order.
pub type Files = Vec<(String, String)>;

/// The package of `host`, composed from `source`.
///
/// # Errors
///
/// A [`ComposeError`] for a source the host's adaptation does not fit.
pub fn compose(host: Host, source: &[(&str, &str)]) -> Result<Files, ComposeError> {
    let find = |path: &'static str| {
        source
            .iter()
            .find(|(each, _)| *each == path)
            .map(|(_, text)| *text)
            .ok_or(ComposeError::Missing(path))
    };
    // Every host drops `agents/`, but for the adaptation it reads in place.
    let (kept, skill_md, added) = match host {
        Host::ClaudeCode => {
            let skill_md = claude_skill_md(find("SKILL.md")?, find(FRAGMENT)?, find(ROUTE)?)?;
            let reference = (
                COMPOSED_REFERENCE.to_owned(),
                find(HOST_REFERENCE)?.to_owned(),
            );
            (None, Some(skill_md), Some(reference))
        }
        Host::Codex => (Some(CODEX_ADAPTATION), None, None),
    };
    let mut files: Files = source
        .iter()
        .filter(|(path, _)| !path.starts_with("agents/") || Some(*path) == kept)
        .map(|(path, text)| {
            let text = match (&skill_md, *path) {
                (Some(composed), "SKILL.md") => composed.clone(),
                _ => (*text).to_owned(),
            };
            ((*path).to_owned(), text)
        })
        .collect();
    files.extend(added);
    Ok(files)
}

/// `SKILL.md` with the fragment's fields after its own and the route after the anchor bullet.
fn claude_skill_md(skill_md: &str, fragment: &str, route: &str) -> Result<String, ComposeError> {
    let lines: Vec<&str> = skill_md.split('\n').collect();
    if lines.first() != Some(&"---") {
        return Err(ComposeError::NoFrontmatter);
    }
    let close = lines[1..]
        .iter()
        .position(|line| *line == "---")
        .ok_or(ComposeError::NoFrontmatter)?
        + 1;
    let added = fields(fragment)?;
    budget(&lines[1..close], &added)?;
    let anchors: Vec<usize> = (close..lines.len())
        .filter(|at| lines[*at].contains(ANCHOR))
        .collect();
    let [anchor] = anchors[..] else {
        return Err(ComposeError::Anchor(anchors.len()));
    };
    let mut after = anchor + 1;
    while lines.get(after).is_some_and(|line| line.starts_with("  ")) {
        after += 1;
    }
    let mut composed: Vec<&str> = lines[..close].to_vec();
    composed.extend(added.iter().copied());
    composed.extend(&lines[close..after]);
    composed.extend(route.trim_end_matches('\n').split('\n'));
    composed.extend(&lines[after..]);
    Ok(composed.join("\n"))
}

/// The fragment's field lines, its comments and blank lines left out.
fn fields(fragment: &str) -> Result<Vec<&str>, ComposeError> {
    let lines: Vec<&str> = fragment
        .lines()
        .filter(|line| !line.trim().is_empty() && !line.starts_with('#'))
        .collect();
    for line in &lines {
        let field = line.split_once(':').map_or(*line, |(field, _)| field);
        if OWNED.contains(&field) {
            return Err(ComposeError::Defines(field.to_owned()));
        }
    }
    Ok(lines)
}

/// Refuses a `description` and `when_to_use` that together pass the listing budget.
fn budget(frontmatter: &[&str], added: &[&str]) -> Result<(), ComposeError> {
    let value = |lines: &[&str], field: &str| {
        let prefix = format!("{field}: ");
        lines
            .iter()
            .find_map(|line| line.strip_prefix(&prefix))
            .map_or(0, |value| value.chars().count())
    };
    let length = value(frontmatter, "description") + value(added, "when_to_use");
    if length > LISTING_BUDGET {
        return Err(ComposeError::Budget {
            length,
            limit: LISTING_BUDGET,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ComposeError, Host, LISTING_BUDGET, compose};

    const SKILL_MD: &str = "---\nname: x\ndescription: d\n---\n\n- Read `references/the-loop.md`\n  first.\n- Read `references/b.md`.\n";

    fn source(fragment: &'static str) -> Vec<(&'static str, &'static str)> {
        vec![
            ("SKILL.md", SKILL_MD),
            ("agents/claude/claude-host.md", "host"),
            ("agents/claude/frontmatter.yaml", fragment),
            (
                "agents/claude/route.md",
                "- Read `references/claude-host.md`.\n",
            ),
            ("agents/openai.yaml", "interface: {}"),
            ("agents/another/host.md", "another host"),
            ("references/the-loop.md", "loop"),
        ]
    }

    #[test]
    fn the_claude_code_package_takes_the_fragment_the_route_and_the_host_reference() {
        let files = compose(Host::ClaudeCode, &source("# note\nwhen_to_use: w\n")).unwrap();
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            [
                "SKILL.md",
                "references/the-loop.md",
                "references/claude-host.md"
            ]
        );
        assert_eq!(
            files[0].1,
            "---\nname: x\ndescription: d\nwhen_to_use: w\n---\n\n- Read `references/the-loop.md`\n  first.\n- Read `references/claude-host.md`.\n- Read `references/b.md`.\n"
        );
    }

    #[test]
    fn the_codex_package_keeps_its_own_adaptation_alone() {
        let files = compose(Host::Codex, &source("when_to_use: w\n")).unwrap();
        let paths: Vec<&str> = files.iter().map(|(path, _)| path.as_str()).collect();
        assert_eq!(
            paths,
            ["SKILL.md", "agents/openai.yaml", "references/the-loop.md"]
        );
        assert_eq!(files[0].1, SKILL_MD);
    }

    #[test]
    fn a_fragment_that_defines_name_or_description_is_refused() {
        for (fragment, field) in [("name: y\n", "name"), ("description: y\n", "description")] {
            assert_eq!(
                compose(Host::ClaudeCode, &source(fragment)),
                Err(ComposeError::Defines(field.to_owned()))
            );
        }
    }

    #[test]
    fn a_listing_past_the_budget_is_refused() {
        let long = format!("when_to_use: {}\n", "w".repeat(LISTING_BUDGET));
        let long: &'static str = Box::leak(long.into());
        assert_eq!(
            compose(Host::ClaudeCode, &source(long)),
            Err(ComposeError::Budget {
                length: LISTING_BUDGET + 1,
                limit: LISTING_BUDGET,
            })
        );
    }

    #[test]
    fn the_refusal_names_the_combined_length_and_the_limit() {
        let error = ComposeError::Budget {
            length: 1537,
            limit: 1536,
        };
        assert_eq!(
            error.to_string(),
            "`description` and `when_to_use` hold 1537 characters together, past the \
             host's listing limit of 1536"
        );
    }

    #[test]
    fn the_refusal_of_a_defined_field_names_the_field() {
        for (fragment, field) in [("name: y\n", "name"), ("description: y\n", "description")] {
            let error = compose(Host::ClaudeCode, &source(fragment)).unwrap_err();
            assert_eq!(
                error.to_string(),
                format!("the fragment defines `{field}`, which `SKILL.md` owns")
            );
        }
    }
}
