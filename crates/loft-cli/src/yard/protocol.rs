//! `loft yard protocol` (the Report the yard protocol Feature): the version of the protocol the
//! `loft yard` commands speak, `loft`'s own version, and the yardlog formats it reads and writes,
//! so that a host checks them before it offers Yard Client's tools, and whether the working tree
//! of the directory it runs in uses Loft, so that a host offers them only there. It needs no
//! repository and writes nothing.

use std::path::Path;

use serde_json::{Value, json};
use yardlog::Format;
use yardlog_store::home::Home;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::version::VERSION;

use super::uses_loft::{self, Shown};

const SCHEMA: &str = "loft-yard-protocol/3";
const OUTCOME: &str = "reported";

/// The protocol's version, raised with each release that changes the input or the output of a
/// `loft yard` command in a way an earlier host cannot read, or that a host sends what an
/// earlier page server would refuse: protocol 3 lets an adapter declare `consults` and give its
/// tool calls the classes of what they consult, and protocol 4 gives `loft yard serve`
/// `--listen` and `--join`, which an earlier `loft` refuses.
pub const PROTOCOL: u32 = 4;

/// Every format this `loft` reads, oldest first: each up to the one it writes.
fn reads() -> Vec<String> {
    (1..=Format::CURRENT.version())
        .map(|version| Format::new(version).to_string())
        .collect()
}

pub fn report() -> Value {
    json!({
        "protocol": PROTOCOL,
        "loft": VERSION,
        "formats": {
            "reads": reads(),
            "writes": Format::CURRENT.to_string(),
        },
    })
}

/// The command's own report: [`report`], which the page server sends to every page too, and what
/// only the command reports: the directory that holds what belongs to the machine, and whether
/// the working tree of `dir` uses Loft.
fn own(mut fields: Value, dir: &Path) -> Value {
    if let Some(home) = Home::from_env() {
        fields["home"] = json!(home.dir().to_string_lossy());
    }
    fields["workingTree"] = uses_loft::report(dir);
    fields
}

/// The line that says whether the working tree of `dir` uses Loft, and what shows it.
fn working_tree_line(dir: &Path) -> String {
    let shown = match uses_loft::of_dir(dir) {
        Some(Shown::Spec) => "`spec/loftspec.yaml` shows it",
        Some(Shown::Lock) => "`loft.lock.json` shows it",
        Some(Shown::YardLog) => "a Yard Log under `.yard/` shows it",
        Some(Shown::Config) => "git's configuration sets `loft.yard`",
        None => {
            return format!(
                "This working tree does not use Loft, so Yard Client stays off in it; in a git repository, `git config {} true` turns it on.",
                uses_loft::SETTING
            );
        }
    };
    format!("This working tree uses Loft: {shown}.")
}

pub fn run(root: &Path, as_json: bool) -> Finished {
    let dir = std::path::absolute(root).unwrap_or_else(|_| root.to_owned());
    if as_json {
        json::print(&json::result(
            SCHEMA,
            OUTCOME,
            own(report(), &dir),
            Vec::new(),
        ));
    } else {
        outln!(
            "loft {VERSION} speaks the yard protocol {PROTOCOL}; it reads {} and writes {}",
            reads().join(", "),
            Format::CURRENT
        );
        outln!("{}", working_tree_line(&dir));
    }
    Finished::success(OUTCOME)
}
