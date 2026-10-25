//! `loft yard protocol` (the Report the yard protocol Feature): the version of the protocol the
//! `loft yard` commands speak, `loft`'s own version, and the yardlog formats it reads and writes,
//! so that a host checks them before it offers Yard Client's tools. It needs no repository and
//! writes nothing.

use serde_json::{Value, json};
use yardlog::Format;

use crate::finish::Finished;
use crate::json;
use crate::out::outln;
use crate::version::VERSION;

const SCHEMA: &str = "loft-yard-protocol/1";
const OUTCOME: &str = "reported";

/// The protocol's version, raised with each release that changes the input or the output of a
/// `loft yard` command in a way an earlier host cannot read.
pub const PROTOCOL: u32 = 1;

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

pub fn run(as_json: bool) -> Finished {
    if as_json {
        json::print(&json::result(SCHEMA, OUTCOME, report(), Vec::new()));
    } else {
        outln!(
            "loft {VERSION} speaks the yard protocol {PROTOCOL}; it reads {} and writes {}",
            reads().join(", "),
            Format::CURRENT
        );
    }
    Finished::success(OUTCOME)
}
