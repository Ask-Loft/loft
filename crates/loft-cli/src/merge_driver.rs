//! `loft merge-driver <base> <ours> <theirs> <path>` (RQ-150, RQ-151): git's merge driver for
//! the files of a `spec/` tree and of Yard Log under `.yard/`. It merges the three versions git
//! hands it, with the merge engine or as yardlog files, and writes the result to `<ours>`; every
//! message names `<path>`, never git's temporary files.
//! With `--install` it registers itself in the repository instead (RQ-152, RQ-153).

use std::path::{Component, Path, PathBuf};
use std::{fs, io};

use loftspec::codec::{ReadError, SpecPath};
use loftspec::identity::Label;
use loftspec::merge::{Input, MergeError, merge_file};
use loftspec_store::StoreError;
use loftspec_store::fs::{SPEC, write_outputs};
use loftspec_store::merge_driver::{self, ATTRIBUTES};
use loftspec_store::repository::GitCommand;
use serde_json::json;

use crate::finish::Finished;
use crate::json;
use crate::out::{errln, outln};
use crate::version::VERSION;
use crate::yard;

/// The command that starts `loft` where a project has it as an npm dev dependency.
pub const NPM_COMMAND: &str = "npx --no-install loft";

/// The step that registers the driver in a repository that has not.
pub fn install_step() -> String {
    format!(
        "run `loft merge-driver --install` once in each clone, so git merges spec/ by key; where \
         loft runs through npm, add --command \"{NPM_COMMAND}\""
    )
}

/// What to do about the merge driver after a command that writes `spec/`: register it where the repository has not, or put the
/// project under git first; nothing where it is registered or git cannot answer.
pub fn suggestion(git: &GitCommand, root: &Path) -> Option<String> {
    match git.config_get(root, merge_driver::DRIVER) {
        Ok(None) => Some(format!("next: {}", install_step())),
        Ok(Some(_)) => None,
        // Outside a repository git refuses the read of its local configuration.
        Err(_) => matches!(git.prefix(root), Err(StoreError::NotARepository(_))).then(|| {
            "next: the merge driver needs a git repository; put the project under git, then run \
             `loft merge-driver --install` once in each clone"
                .to_owned()
        }),
    }
}

/// The schema `--format json` writes to, by name and version (RQ-156).
const SCHEMA: &str = "loft-merge-driver/1";

/// The three files git hands the driver, and the path of the file being merged as git names it.
pub struct Inputs<'a> {
    pub base: &'a Path,
    pub ours: &'a Path,
    pub theirs: &'a Path,
    pub path: &'a str,
}

pub fn run(root: &Path, inputs: &Inputs<'_>, as_json: bool) -> Finished {
    match merge(root, inputs) {
        Ok((conflicts, disputed)) => {
            let merged = conflicts == 0;
            let outcome = if merged { "merged" } else { "conflicted" };
            if as_json {
                let fields = json!({
                    "path": inputs.path,
                    "conflicts": conflicts,
                    "disputed": disputed,
                });
                json::print(&json::result(SCHEMA, outcome, fields, Vec::new()));
            }
            if merged {
                return Finished::success(outcome);
            }
            let plural = if conflicts == 1 { "" } else { "s" };
            errln!(
                "{}: {conflicts} merge conflict{plural} written",
                inputs.path
            );
            if !disputed.is_empty() {
                errln!("{}: {}", inputs.path, settle_with_publish(&disputed));
            }
            Finished::failure(outcome)
        }
        Err(messages) => {
            let mut errors = Vec::new();
            for message in messages {
                let message = format!("{}{message}", inputs.path);
                errors.push(json::error("merge", &message, &[]));
                errln!("{message}");
            }
            let finished = Finished::stopped(&errors);
            if as_json {
                json::print(&json::failure(SCHEMA, errors));
            }
            finished
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
pub fn install(root: &Path, launcher: &str, as_json: bool) -> Finished {
    match merge_driver::install_routing(
        &GitCommand::default(),
        root,
        launcher,
        &[merge_driver::ATTRIBUTE, yardlog_store::ATTRIBUTE],
    ) {
        Ok(installed) => {
            let own = format!("loft {VERSION}");
            if installed.version != own {
                errln!(
                    "loft merge-driver --install: warning: `{launcher}` starts {}, not {own}",
                    installed.version
                );
            }
            if as_json {
                let fields = json!({ "written": installed.written, "version": installed.version });
                json::print(&json::result(SCHEMA, "installed", fields, Vec::new()));
            } else {
                for name in &installed.written {
                    outln!("{name}");
                }
                if installed.written.iter().any(|name| name == ATTRIBUTES) {
                    outln!(
                        "commit {ATTRIBUTES}, so that every clone routes spec/ and .yard/ to the driver"
                    );
                }
            }
            Finished::success("installed")
        }
        Err(error) => {
            let message = format!("loft merge-driver --install: {}", refusal(&error, launcher));
            let errors = vec![json::error("install", &message, &[])];
            let finished = Finished::stopped(&errors);
            if as_json {
                json::print(&json::failure(SCHEMA, errors));
            }
            errln!("{message}");
            finished
        }
    }
}

/// Why the driver was not registered.
fn refusal(error: &StoreError, launcher: &str) -> String {
    match error {
        StoreError::Launcher(_) => format!(
            "git could not run `{launcher} --version` from the top of the working tree, where it \
             runs the driver; give the command that starts loft there with --command, such as \
             --command \"{NPM_COMMAND}\""
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
    let spec_path = if yard::merge::holds(inputs.path) {
        None
    } else {
        Some(
            spec_path(inputs.path)
                .ok_or_else(|| vec![format!(": not under a `{SPEC}` directory")])?,
        )
    };
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
    let (bytes, conflicts, disputed) = match spec_path {
        Some(spec_path) => {
            let merged = merge_file(&spec_path, &base, &ours, &theirs).map_err(messages)?;
            (merged.bytes, merged.conflicts, merged.disputed)
        }
        None => (
            yard::merge::merge(root, inputs.path, &base, &ours, &theirs)?,
            0,
            Vec::new(),
        ),
    };
    write_outputs(root, &[(inputs.ours.to_path_buf(), bytes)])
        .map_err(|error| vec![format!(": {error}")])?;
    Ok((conflicts, disputed))
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
        MergeError::Unplaced(key) => {
            vec![format!(
                ": no list of the merged file can take the record {key}, as when a side changed \
                 the kind of the record or of its parent"
            )]
        }
    }
}

/// Where in the file a read error lies, as `:<line>:<column>` with every further site after a
/// comma, as `write_sites` writes sites in one file (AC-251); or nothing. Every site lies in the
/// one file merged, which the message names by `<path>` rather than by its path under `spec/`.
fn location(error: &ReadError) -> String {
    let locations: Vec<String> = error
        .sites
        .iter()
        .filter_map(|site| site.location)
        .map(|location| location.to_string())
        .collect();
    if locations.is_empty() {
        String::new()
    } else {
        format!(":{}", locations.join(", "))
    }
}
