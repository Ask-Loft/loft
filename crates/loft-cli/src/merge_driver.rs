//! `loft merge-driver <base> <ours> <theirs> <path>` (RQ-150, RQ-151): git's merge driver for
//! the files of a `spec/` tree. It merges the three versions git hands it with the merge engine
//! and writes the result to `<ours>`; every message names `<path>`, never git's temporary files.
//! With `--install` it registers itself in the repository instead (RQ-152, RQ-153).

use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use loftspec::codec::{ReadError, SpecPath};
use loftspec::identity::Label;
use loftspec::merge::{Input, MergeError, merge_file};
use loftspec_store::StoreError;
use loftspec_store::fs::{SPEC, write_outputs};
use loftspec_store::merge_driver;
use loftspec_store::repository::GitCommand;
use serde_json::json;

use crate::json;
use crate::version::VERSION;

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-merge-driver/1";

/// The three files git hands the driver, and the path of the file being merged as git names it.
pub struct Inputs<'a> {
    pub base: &'a Path,
    pub ours: &'a Path,
    pub theirs: &'a Path,
    pub path: &'a str,
}

pub fn run(root: &Path, inputs: &Inputs<'_>, as_json: bool) -> ExitCode {
    match merge(root, inputs) {
        Ok((conflicts, disputed)) => {
            if as_json {
                let outcome = if conflicts == 0 {
                    "merged"
                } else {
                    "conflicted"
                };
                let fields = json!({
                    "path": inputs.path,
                    "conflicts": conflicts,
                    "disputed": disputed,
                });
                json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
            }
            if conflicts == 0 {
                return ExitCode::SUCCESS;
            }
            let plural = if conflicts == 1 { "" } else { "s" };
            eprintln!(
                "{}: {conflicts} merge conflict{plural} written",
                inputs.path
            );
            if !disputed.is_empty() {
                eprintln!("{}: {}", inputs.path, settle_with_publish(&disputed));
            }
            ExitCode::FAILURE
        }
        Err(messages) => {
            let mut errors = Vec::new();
            for message in messages {
                let message = format!("{}{message}", inputs.path);
                errors.push(json::error("merge", &message, &[]));
                eprintln!("{message}");
            }
            if as_json {
                json::print(&json::failure(SCHEMA, errors));
            }
            ExitCode::FAILURE
        }
    }
}

/// What to do about the labels two publishes gave apart.
fn settle_with_publish(disputed: &[Label]) -> String {
    let names = disputed
        .iter()
        .map(ToString::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    let (noun, verb, pronoun) = match disputed {
        [_] => ("label", "was", "it"),
        _ => ("labels", "were", "them"),
    };
    format!(
        "{noun} {names} {verb} given apart by two publishes; run `loft publish` on the publish \
         branch to settle {pronoun}"
    )
}

/// Registers the driver started by `launcher`, printing what it wrote, one name to a line, and
/// warning when the launcher starts another version of `loft`.
pub fn install(root: &Path, launcher: &str, as_json: bool) -> ExitCode {
    match merge_driver::install(&GitCommand::default(), root, launcher) {
        Ok(installed) => {
            let own = format!("loft {VERSION}");
            if installed.version != own {
                eprintln!(
                    "loft merge-driver --install: warning: `{launcher}` starts {}, not {own}",
                    installed.version
                );
            }
            if as_json {
                let fields = json!({ "written": installed.written, "version": installed.version });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                for name in installed.written {
                    println!("{name}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            let message = format!("loft merge-driver --install: {}", refusal(&error, launcher));
            if as_json {
                let errors = vec![json::error("install", &message, &[])];
                json::print(&json::failure(SCHEMA, errors));
            }
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}

/// Why the driver was not registered.
fn refusal(error: &StoreError, launcher: &str) -> String {
    match error {
        StoreError::Launcher(_) => format!(
            "git could not run `{launcher} --version` from the top of the working tree, where it \
             runs the driver; give the command that starts loft there with --command, such as \
             --command \"npx --no-install loft\""
        ),
        StoreError::NotARepository(_) => "a git repository is required, and the working \
                                          directory is not inside one"
            .to_owned(),
        error => error.to_string(),
    }
}

/// Merges the file and writes it to `<ours>`, giving the number of conflicts written and the
/// labels in dispute; or the messages, each to follow `<path>`, and nothing written.
fn merge(root: &Path, inputs: &Inputs<'_>) -> Result<(usize, Vec<Label>), Vec<String>> {
    let spec_path =
        spec_path(inputs.path).ok_or_else(|| vec![format!(": not under a `{SPEC}` directory")])?;
    let read = |input: Input, file: &Path| {
        fs::read(root.join(file))
            .map_err(|error: io::Error| format!(": cannot read {input}: {error}"))
    };
    let (base, ours, theirs) = (
        read(Input::Base, inputs.base),
        read(Input::Ours, inputs.ours),
        read(Input::Theirs, inputs.theirs),
    );
    let (base, ours, theirs) = match (base, ours, theirs) {
        (Ok(base), Ok(ours), Ok(theirs)) => (base, ours, theirs),
        (base, ours, theirs) => {
            return Err([base, ours, theirs]
                .into_iter()
                .filter_map(Result::err)
                .collect());
        }
    };
    let merged = merge_file(&spec_path, &base, &ours, &theirs).map_err(messages)?;
    write_outputs(root, &[(inputs.ours.to_path_buf(), merged.bytes)])
        .map_err(|error| vec![format!(": {error}")])?;
    Ok((merged.conflicts, merged.disputed))
}

/// The path of the file within its tree: what follows the first `spec` directory of `path`.
fn spec_path(path: &str) -> Option<SpecPath> {
    let mut parts = Path::new(path).components();
    parts.find(|part| *part == Component::Normal(SPEC.as_ref()))?;
    let rest: PathBuf = parts.collect();
    let rest = rest.to_str()?;
    (!rest.is_empty()).then(|| SpecPath::new(&rest.replace('\\', "/")))
}

fn messages(error: MergeError) -> Vec<String> {
    match error {
        MergeError::NotAFormatFile => vec![": not a file of the Format".to_owned()],
        MergeError::Unreadable(errors) => errors
            .iter()
            .map(|(input, error)| format!("{}: {input}: {}", location(error), error.problem))
            .collect(),
        MergeError::BothChanged => {
            vec![": both sides changed the lint configuration to different contents".to_owned()]
        }
        MergeError::Unwritable(error) => {
            vec![format!(
                ": the merged file does not read back: {}",
                error.problem
            )]
        }
    }
}

/// Where in the file a read error lies, as `:<line>:<column>`, or nothing.
fn location(error: &ReadError) -> String {
    error
        .sites
        .iter()
        .find_map(|site| site.location)
        .map_or_else(String::new, |location| format!(":{location}"))
}
