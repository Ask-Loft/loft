//! What the edit generator draws from: the texts, tags and keys it writes, the records it
//! inserts, and the candidates each edit picks among.

use loftspec::codec::schema::{Kind, field};

use super::edit_tree::{At, ENTRIES, Top, Y, list_kind, node, records};
use super::edits::{Pool, Side};

/// The texts an edit writes; one spans lines, which the writer writes as a literal block.
pub const TEXTS: [&str; 4] = ["One.", "Two.", "Three.", "A first line.\nA second line.\n"];

/// The tags the registry of the test document defines.
pub const TAG_NAMES: [&str; 2] = ["identity", "portable"];

/// The key an inserted record carries: `n` of the side's own pool, or of the pool both sides
/// share.
pub fn pool_key(side: Side, pool: Pool, n: usize) -> String {
    let start = match (pool, side) {
        (Pool::Shared, _) => 0x500,
        (Pool::Own, Side::Base) => 0x200,
        (Pool::Own, Side::Ours) => 0x300,
        (Pool::Own, Side::Theirs) => 0x400,
    };
    format!("00000000-0000-7000-8000-{:012x}", start + n)
}

fn text(n: usize) -> Y {
    Y::Text(TEXTS[n % TEXTS.len()].to_owned())
}

fn plain(text: &str) -> Y {
    Y::Plain(text.to_owned())
}

fn quality_target() -> Y {
    Y::Map(vec![
        (field::METRIC, Y::Text("op.wall-time".to_owned())),
        (
            field::BAND,
            Y::Map(vec![
                (field::KIND, plain("at_most")),
                (field::UPPER, Y::Text("1".to_owned())),
            ]),
        ),
        (field::UNIT, Y::Text("ms".to_owned())),
        (field::AGGREGATION, plain("p99")),
        (field::SCENARIO, Y::Text("one operation".to_owned())),
        (field::ENVIRONMENT, Y::Text("a machine".to_owned())),
    ])
}

/// A new record of `kind` keyed `key`, with only the fields its kind requires.
pub fn record_of(kind: Kind, key: &str, n: usize) -> Y {
    let suffix = &key[key.len() - 3..];
    let mut fields = vec![(field::KEY, plain(key))];
    match kind {
        Kind::Section => fields.extend([(field::NAME, text(n)), (field::BODY, text(n + 1))]),
        Kind::Criterion | Kind::Constraint | Kind::Component => {
            fields.push((field::NAME, text(n)));
        }
        Kind::Requirement => fields.push((field::STATEMENT, Y::Map(vec![(field::TEXT, text(n))]))),
        Kind::QualityAttribute => fields.extend([
            (field::NAME, text(n)),
            (field::QA_KIND, plain("latency")),
            (field::QUALITY_TARGET, quality_target()),
        ]),
        Kind::FeatureGroup => fields.extend([
            (field::SLUG, plain(&format!("group-{suffix}"))),
            (field::NAME, text(n)),
        ]),
        Kind::TagEntry => fields.extend([
            (field::NAME, plain(&format!("tag-{suffix}"))),
            (field::DEFINITION, text(n)),
        ]),
        other => panic!("no record of kind {other:?} is inserted"),
    }
    Y::Map(fields)
}

/// A registry's entries in canonical order: by name, then key.
pub fn sort_registry(root: &mut Y) {
    let entries = root.get_mut(ENTRIES).expect("the entries").items_mut();
    let order = |entry: &Y| {
        (
            entry
                .get(field::NAME)
                .and_then(Y::text)
                .unwrap_or_default()
                .to_owned(),
            entry.key().unwrap_or_default().to_owned(),
        )
    };
    entries.sort_by_key(order);
}

/// Each text field of each keyed record, as the record and the path to the field.
pub fn text_targets(top: Top, root: &Y) -> Vec<(At, Vec<&'static str>)> {
    let is_text = |value: Option<&Y>| matches!(value, Some(Y::Text(_)));
    let mut targets = Vec::new();
    for at in records(top, root) {
        let record = node(root, &at.path);
        for name in [field::NAME, field::BODY, field::DEFINITION] {
            if is_text(record.get(name)) {
                targets.push((at.clone(), vec![name]));
            }
        }
        let statement = record.get(field::STATEMENT);
        for name in [field::TEXT, field::SHALL] {
            if is_text(statement.and_then(|statement| statement.get(name))) {
                targets.push((at.clone(), vec![field::STATEMENT, name]));
            }
        }
    }
    targets
}

/// Each text field a conflict record could be carried over: the record, the field's dotted
/// path and its current value. A field that already has a conflict is left out.
pub fn conflict_targets(top: Top, root: &Y) -> Vec<(At, String, String)> {
    text_targets(top, root)
        .into_iter()
        .filter_map(|(at, names)| {
            let record = node(root, &at.path);
            let path = names.join(".");
            let current = names
                .iter()
                .try_fold(record, |value, name| value.get(name))?
                .text()?
                .to_owned();
            let carried = record.get(field::CONFLICTS).is_some_and(|list| {
                list.items()
                    .iter()
                    .any(|conflict| conflict.get(field::FIELD).and_then(Y::text) == Some(&path))
            });
            (!carried).then_some((at, path, current))
        })
        .collect()
}

/// Each list a new record could go into: its host's key (none for a registry), the field, and
/// the kind the list holds.
pub fn insert_slots(top: Top, root: &Y) -> Vec<(Option<String>, &'static str, Kind)> {
    if top == Top::Registry {
        return vec![(None, ENTRIES, Kind::TagEntry)];
    }
    records(top, root)
        .into_iter()
        .flat_map(|at| {
            at.kind.fields().iter().filter_map(move |spec| {
                list_kind(Some(at.kind), spec.name)
                    .map(|kind| (Some(at.key.clone()), spec.name, kind))
            })
        })
        .collect()
}

/// The key of each record that could take `moved` into its list of the same field: never the
/// record itself or a record inside it.
pub fn move_slots(top: Top, root: &Y, moved: &At) -> Vec<String> {
    let name = moved.path.last().expect("a listed record").0;
    records(top, root)
        .into_iter()
        .filter(|at| !at.path.starts_with(&moved.path))
        .filter(|at| list_kind(Some(at.kind), name) == Some(moved.kind))
        .map(|at| at.key)
        .collect()
}
