//! A document's files as the remote's wire carries them: a list of objects, each with its `path`
//! from the top of the tree, under `spec/`, and its `text`.

use loftspec::codec::{Files, SpecPath};
use serde_json::Value;

/// The files of `list`, by their paths under `spec/`. The error names a file that has no path or
/// text, or lies outside `spec/`, as one the remote `verb` (`gave`, `returned`).
///
/// # Errors
///
/// A file has no path or text, or its path does not name a file under `spec/`.
pub fn read(list: &Value, verb: &str) -> Result<Files, String> {
    let mut files = Files::new();
    for file in list.as_array().into_iter().flatten() {
        let (Some(path), Some(text)) = (file["path"].as_str(), file["text"].as_str()) else {
            return Err(format!("the remote {verb} a file without its path or text"));
        };
        let under = path
            .strip_prefix("spec/")
            .filter(|under| {
                under
                    .split('/')
                    .all(|segment| !matches!(segment, "" | "." | ".."))
            })
            .ok_or_else(|| format!("the remote {verb} a file outside spec/: {path}"))?;
        files.insert(SpecPath::new(under), text.as_bytes().to_vec());
    }
    Ok(files)
}
