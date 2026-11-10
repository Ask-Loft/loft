//! What a tool call consults (the Feature "Classify what a tool call consults"): the class of
//! each path it reads or edits, `spec` under the `spec/` of the known tree that holds it, `code`
//! elsewhere in that tree and `other` outside every known tree or within its `.git/` or
//! `.yard/`; the classes a shell command's reading programs read; and the `loft` subcommands it
//! runs. The Codex adapter gives a tool call's start these, and nothing else of its input. The
//! Claude mod's `consult.ts` and `shell.ts` follow the same rules, and both are held to the cases
//! in `plugins/claude/loft/tests/consult-cases.ts`. Pure: no file is read.

mod patch;
mod paths;
mod readers;
mod words;

pub use patch::patched;
pub use paths::{Class, class_of, holds, ordered};
pub use readers::read_paths;
pub use words::{commands_of, loft_names, runs};

#[cfg(test)]
mod tests;
