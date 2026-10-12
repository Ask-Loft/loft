//! `loft merge-driver <base> <ours> <theirs> <path>` (RQ-150, RQ-151): git's merge driver for
//! the files of a `spec/` tree. It merges the three versions git hands it with the merge engine
//! and writes the result to `<ours>`; every message names `<path>`, never git's temporary files.

use std::path::{Component, Path, PathBuf};
use std::process::ExitCode;
use std::{fs, io};

use loftspec::codec::{ReadError, SpecPath};
use loftspec::merge::{Input, MergeError, merge_file};
use loftspec_store::fs::{SPEC, write_outputs};

/// The three files git hands the driver, and the path of the file being merged as git names it.
pub struct Inputs<'a> {
    pub base: &'a Path,
    pub ours: &'a Path,
    pub theirs: &'a Path,
    pub path: &'a str,
}

pub fn run(root: &Path, inputs: &Inputs<'_>) -> ExitCode {
    match merge(root, inputs) {
        Ok(0) => ExitCode::SUCCESS,
        Ok(conflicts) => {
            let plural = if conflicts == 1 { "" } else { "s" };
            eprintln!(
                "{}: {conflicts} merge conflict{plural} written",
                inputs.path
            );
            ExitCode::FAILURE
        }
        Err(messages) => {
            for message in messages {
                eprintln!("{}{message}", inputs.path);
            }
            ExitCode::FAILURE
        }
    }
}

/// Merges the file and writes it to `<ours>`, giving the number of conflicts written; or the
/// messages, each to follow `<path>`, and nothing written.
fn merge(root: &Path, inputs: &Inputs<'_>) -> Result<usize, Vec<String>> {
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
    Ok(merged.conflicts)
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
