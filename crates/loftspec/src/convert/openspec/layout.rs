//! The names `OpenSpec` gives the parts of its directory, shared by the reader of a project, the
//! converter and the renderer.

/// The directory of the main specs.
pub const SPECS: &str = "specs";
/// The name of each main spec's file.
pub const SPEC_FILE: &str = "spec.md";
/// The directory of the changes.
pub const CHANGES: &str = "changes";
/// The directory of the archived changes, under [`CHANGES`] and, in older projects, at the top.
pub const ARCHIVE: &str = "archive";
/// The legacy project description.
pub const PROJECT_MD: &str = "project.md";
/// The project configuration.
pub const CONFIG: &str = "config.yaml";
