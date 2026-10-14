//! A written record's fields in the order its file holds them, and a file with one field of one
//! record taken out, for the field-order and required-field tests. A record is found by its
//! route from the top of its file: each step a field name, or an index into a list.

use loftspec::codec::{self, Files, Problem, Site};
use loftspec::yaml::{self, Location, Node, Position, Value};

use super::text::{path, text};

fn parsed(files: &Files, at: &str) -> Node {
    yaml::read(&files[&path(at)]).unwrap().unwrap()
}

/// The node at `route` under `root`.
fn node<'n>(root: &'n Node, route: &[&str]) -> &'n Node {
    route.iter().fold(root, |node, step| match &node.value {
        Value::Mapping(entries) => {
            &entries
                .iter()
                .find(|entry| entry.key == *step)
                .unwrap_or_else(|| panic!("no field {step}"))
                .value
        }
        Value::Sequence(items) => &items[step.parse::<usize>().unwrap()],
        other => panic!("{step} in {other:?}"),
    })
}

/// The names of the fields of the record at `route` in the file at `at`, in the file's order.
pub fn fields(files: &Files, at: &str, route: &[&str]) -> Vec<String> {
    match &node(&parsed(files, at), route).value {
        Value::Mapping(entries) => entries.iter().map(|entry| entry.key.clone()).collect(),
        other => panic!("{other:?}"),
    }
}

fn text_of(node: &Node) -> String {
    match &node.value {
        Value::Scalar(scalar) => scalar.text.clone(),
        other => panic!("{other:?}"),
    }
}

/// The scalar value at `route` in the file at `at`.
pub fn scalar(files: &Files, at: &str, route: &[&str]) -> String {
    text_of(node(&parsed(files, at), route))
}

/// The scalar values of the list at `route` in the file at `at`, in the file's order.
pub fn scalars(files: &Files, at: &str, route: &[&str]) -> Vec<String> {
    match &node(&parsed(files, at), route).value {
        Value::Sequence(items) => items.iter().map(text_of).collect(),
        other => panic!("{other:?}"),
    }
}

fn indent(line: &str) -> usize {
    line.len() - line.trim_start_matches(' ').len()
}

/// The lines of the entry at `index` of `entries`, as a range of zero-based line numbers.
fn span(lines: &[String], entries: &[yaml::Entry], index: usize) -> std::ops::Range<usize> {
    let first = entries[index].key_position.line - 1;
    let column = entries[index].key_position.column;
    let end = match entries.get(index + 1) {
        Some(next) => next.key_position.line - 1,
        None => (first + 1..lines.len())
            .find(|&line| !lines[line].is_empty() && indent(&lines[line]) < column - 1)
            .unwrap_or(lines.len()),
    };
    first..end
}

/// The files with the field `field` of the record at `route` in the file at `at` taken out, and
/// where that record then starts. A record's first field shares its line with the list entry's
/// `- `, which the next field then takes.
pub fn without(files: &Files, at: &str, route: &[&str], field: &str) -> (Files, Position) {
    let root = parsed(files, at);
    let Value::Mapping(entries) = &node(&root, route).value else {
        panic!("{route:?} is no record")
    };
    let index = entries
        .iter()
        .position(|entry| entry.key == field)
        .unwrap_or_else(|| panic!("no field {field}"));
    let mut lines: Vec<String> = text(files, at).lines().map(str::to_owned).collect();
    let span = span(&lines, entries, index);
    let prefix = lines[span.start][..entries[index].key_position.column - 1].to_owned();
    if index == 0 && span.end < lines.len() {
        let rest = lines[span.end][prefix.len()..].to_owned();
        lines[span.end] = format!("{prefix}{rest}");
    }
    lines.drain(span);
    let mut files = files.clone();
    files.insert(path(at), format!("{}\n", lines.join("\n")).into_bytes());
    let start = node(&parsed(&files, at), route).position;
    (files, start)
}

/// Asserts that taking `field` out of the record at `route` of the file at `at` in `files` is
/// refused at the record's start, naming the field.
pub fn assert_required(files: &Files, at: &str, route: &[&str], field: &'static str) {
    let (files, start) = without(files, at, route, field);
    let errors = codec::read(&files).expect_err(field);
    assert_eq!(errors.len(), 1, "{field}: {errors:?}");
    assert_eq!(errors[0].problem, Problem::MissingField(field), "{field}");
    let site = Site {
        path: path(at),
        location: Some(Location::At(start)),
    };
    assert_eq!(errors[0].sites, [site], "{field}");
}
