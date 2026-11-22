//! The number of items of each list of a report (the Record a Loft view Feature), which a read
//! of Yard Log without its reports gives in place of the report. A report is recorded whole.

use serde_json::{Map, Value, json};
use yardlog::body::View;

/// The lists of each view's report that [`count`] counts, by their path in the report.
fn lists(view: View) -> &'static [&'static [&'static str]] {
    match view {
        View::SpecChange => &[
            &["status", "base", "added"],
            &["status", "base", "changed"],
            &["status", "base", "removed"],
            &["graph", "nodes"],
            &["graph", "decisions"],
            &["graph", "edges"],
            &["graph", "diff"],
        ],
        View::Publish => &[
            &["status", "base", "added"],
            &["status", "base", "changed"],
            &["status", "base", "removed"],
        ],
        View::Findings => &[
            &["check", "findings"],
            &["check", "resolved"],
            &["check", "errors"],
        ],
        View::Trace => &[&["features"]],
        View::Decision => &[],
    }
}

/// Records under `counts`, by each list's path, the number of items of each list of `report`.
pub fn count(report: &mut Value, view: View) {
    let mut counts = Map::new();
    for path in lists(view) {
        if let Some(Value::Array(items)) = path.iter().try_fold(&*report, |at, name| at.get(*name))
        {
            counts.insert(path.join("."), json!(items.len()));
        }
    }
    report["counts"] = Value::Object(counts);
}
