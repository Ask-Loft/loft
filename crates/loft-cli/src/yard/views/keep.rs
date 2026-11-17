//! How a report is kept to [`MOST`] items of each list (the Record a Loft view Feature): a list
//! of the report keeps its first [`MOST`] items, and so does each list that lies inside the items
//! of another, such as a decision's subjects in the diff or a Feature's criteria, across all of
//! that list's items together. The report counts each list in full, before any is shortened, and
//! the number of its items left out.

use serde_json::{Map, Value, json};
use yardlog::body::View;

use super::MOST;

/// The lists of each view's report that [`keep_most`] shortens, by their path in the report.
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

/// The lists inside the items of another list that [`keep_most`] shortens across all of that
/// list's items, by their path in the report, `*` standing for each item; a list comes before
/// the lists inside its own items, so that what a shorter list left out is not counted as kept.
fn nested(view: View) -> &'static [&'static [&'static str]] {
    match view {
        View::SpecChange => &[&["graph", "diff", "*", "subjects"]],
        View::Trace => &[
            &["features", "*", "requirements"],
            &["features", "*", "criteria"],
            &["features", "*", "criteria", "*", "tests"],
            &["features", "*", "criteria", "*", "rules"],
        ],
        View::Publish | View::Findings | View::Decision => &[],
    }
}

/// Keeps the first [`MOST`] items of each long list of a report, and of each list inside the
/// items of another across all of them, and records each list's whole count and the number of
/// its items left out, under `counts` and `leftOut` by the list's path, `*` left out of it.
pub fn keep_most(report: &mut Value, view: View) {
    let mut counts = Map::new();
    let mut left_out = Map::new();
    // Each nested list counted whole, before the list that holds it is shortened.
    let wholes: Vec<usize> = nested(view)
        .iter()
        .map(|path| arrays(report, path).iter().map(|items| items.len()).sum())
        .collect();
    for path in lists(view) {
        let Some(Value::Array(items)) = path
            .iter()
            .try_fold(&mut *report, |at, name| at.get_mut(*name))
        else {
            continue;
        };
        let name = path.join(".");
        counts.insert(name.clone(), json!(items.len()));
        if items.len() > MOST {
            left_out.insert(name, json!(items.len() - MOST));
            items.truncate(MOST);
        }
    }
    for (path, whole) in nested(view).iter().zip(wholes) {
        let mut room = MOST;
        for items in arrays(report, path) {
            items.truncate(room);
            room -= items.len();
        }
        let kept = MOST - room;
        let name = named(path);
        counts.insert(name.clone(), json!(whole));
        if whole > kept {
            left_out.insert(name, json!(whole - kept));
        }
    }
    report["counts"] = Value::Object(counts);
    report["leftOut"] = Value::Object(left_out);
}

/// The name a nested list is counted under: its path without its `*`.
fn named(path: &[&str]) -> String {
    let names: Vec<&str> = path.iter().copied().filter(|name| *name != "*").collect();
    names.join(".")
}

/// Each list at `path` in `at`, in the report's order.
fn arrays<'a>(at: &'a mut Value, path: &[&str]) -> Vec<&'a mut Vec<Value>> {
    let mut found = Vec::new();
    gather(at, path, &mut found);
    found
}

fn gather<'a>(at: &'a mut Value, path: &[&str], found: &mut Vec<&'a mut Vec<Value>>) {
    match (path.split_first(), at) {
        (None, Value::Array(items)) => found.push(items),
        (Some((&"*", rest)), Value::Array(items)) => {
            for item in items {
                gather(item, rest, found);
            }
        }
        (Some((name, rest)), Value::Object(fields)) => {
            if let Some(next) = fields.get_mut(*name) {
                gather(next, rest, found);
            }
        }
        _ => {}
    }
}
