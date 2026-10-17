//! The format reference against the code it describes: `references/format.md` describes each
//! record kind's fields in the schema's order with whether each is required and the values of
//! each closed set, shows each statement form as `loft` renders it, the edges and the references
//! by name as `loft fmt` resolves them, a lint configuration the reader accepts, and the trace
//! file's fields as its reader takes them.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;

use crate::common::skill_source;
use crate::markdown::{Described, field_tables, rows_by_span, section, spans, table};
use loftspec::codec::schema::{Field, Item, Kind, Scalar, Target, field};
use loftspec::identity::Key;
use loftspec::lint::{LintConfig, RuleId, Severity};
use loftspec::statement::{EarsForm, Form};
use loftspec::trace::{FILE_FIELDS, FORMAT, LINK_FIELDS, TEST_FIELDS, TRACE};

/// `references/format.md`.
pub fn format() -> String {
    fs::read_to_string(skill_source().join("references/format.md")).unwrap()
}

/// The record that the field table of `kind` names; one table describes both Components.
fn title(kind: Kind) -> &'static str {
    match kind {
        Kind::Manifest => "Manifest",
        Kind::DerivedFrom => "`derivedFrom`",
        Kind::Labels => "`spec/labels.yaml`",
        Kind::Counters => "`counters`",
        Kind::Binding => "Binding",
        Kind::TagEntry => "Tag",
        Kind::CategoryEntry => "Category",
        Kind::TermEntry => "Term",
        Kind::Product => "Product",
        Kind::FeatureGroup => "Feature Group",
        Kind::Feature => "Feature",
        Kind::Requirement => "Requirement",
        Kind::Statement => "Statement",
        Kind::Criterion => "Criterion",
        Kind::RootComponent | Kind::Component => "Component",
        Kind::Constraint => "Constraint",
        Kind::ConstraintBinding => "`constraintBinding`",
        Kind::QualityAttribute => "Quality Attribute",
        Kind::QualityTarget => "`qualityTarget`",
        Kind::Band => "`band`",
        Kind::Section => "Section",
        Kind::Decision => "Decision",
        Kind::State => "`state`",
        Kind::Driver => "Driver",
        Kind::Option => "Option",
        Kind::Assessment => "Assessment",
        Kind::Consequence => "Consequence",
        Kind::Subject => "Subject",
        Kind::FormerLabel => "Former label",
        other => panic!("no field table is named for {other:?}"),
    }
}

/// The records the trace section's field tables describe.
const TRACE_TABLES: [&str; 3] = ["Trace file", "Link", "Test"];

/// Every record kind the reader accepts: a reference by name is read by `loft fmt` alone.
fn read_kinds() -> impl Iterator<Item = Kind> {
    Kind::ALL.into_iter().filter(|kind| *kind != Kind::ByName)
}

/// The fields of `kind` as its table describes them: the Component table describes a root
/// Component, with the `slug` a nested one lacks.
fn described(tables: &BTreeMap<String, Described>, kind: Kind) -> Described {
    let table = tables
        .get(title(kind))
        .unwrap_or_else(|| panic!("no table describes {kind:?}"));
    let nested = kind == Kind::Component;
    let fields = table
        .iter()
        .filter(|(name, _)| !nested || name != field::SLUG);
    fields.cloned().collect()
}

#[test]
fn each_record_kind_the_reader_accepts_is_described_with_its_fields_in_order() {
    let tables = field_tables(&format());
    for kind in read_kinds() {
        let names: Vec<String> = described(&tables, kind)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        let fields: Vec<&str> = kind.fields().iter().map(|field| field.name).collect();
        assert_eq!(names, fields, "{kind:?}");
    }
    let named: BTreeSet<&str> = read_kinds().map(title).chain(TRACE_TABLES).collect();
    let found: BTreeSet<&str> = tables.keys().map(String::as_str).collect();
    assert_eq!(found, named, "a table describes no record of the format");
}

#[test]
fn each_field_is_marked_required_or_optional_as_the_schema_holds_it() {
    let tables = field_tables(&format());
    for (record, fields) in &tables {
        for (name, mark) in fields {
            let marked = mark.starts_with("required") || mark.starts_with("optional");
            assert!(marked, "{record} `{name}`: {mark}");
        }
    }
    for kind in read_kinds() {
        for (name, mark) in described(&tables, kind) {
            let (_, field) = kind.field(&name).unwrap();
            let required = mark.starts_with("required");
            assert_eq!(required, field.required, "{kind:?} `{name}`: {mark}");
        }
    }
}

#[test]
fn each_field_of_a_closed_set_lists_its_values() {
    let markdown = format();
    let tables = field_tables(&markdown);
    let mut sets = 0;
    for kind in read_kinds() {
        let marks: BTreeMap<String, String> = described(&tables, kind).into_iter().collect();
        for field in kind.fields() {
            let Item::Scalar(Scalar::Enum(values)) = field.shape.item() else {
                continue;
            };
            sets += 1;
            let mark = &marks[field.name];
            for value in values {
                let listed = mark.contains(&format!("`{value}`"));
                assert!(listed, "{kind:?} `{}` lacks `{value}`: {mark}", field.name);
            }
        }
    }
    assert!(sets >= 12, "the scan found {sets} closed sets");
    let lint = section(&markdown, "Lint configuration");
    for severity in Severity::NAMES {
        assert!(lint.contains(&format!("`{severity}`")), "{severity}");
    }
}

/// The statement table's rows, by form.
fn statement_rows(markdown: &str) -> BTreeMap<String, Vec<String>> {
    let table = table(section(markdown, "Statements"), "`form`");
    let rows = rows_by_span(&table);
    rows.into_iter()
        .map(|(form, row)| (form.to_owned(), row.to_vec()))
        .collect()
}

#[test]
fn each_statement_form_is_shown_with_its_slots_in_order() {
    let rows = statement_rows(&format());
    let forms: Vec<&str> = rows.keys().map(String::as_str).collect();
    let mut names = Form::NAMES.to_vec();
    names.sort_unstable();
    assert_eq!(forms, names);
    for form in Form::ALL {
        let row = &rows[form.as_str()];
        assert_eq!(spans(&row[1]), form.slots(), "{}", form.as_str());
    }
}

#[test]
fn each_statement_form_is_shown_with_the_sentence_loft_renders() {
    let rows = statement_rows(&format());
    let slot = |name: &str| format!("<{name}>");
    for form in Form::ALL {
        let condition = slot(form.condition().unwrap_or_default());
        let rendered = EarsForm::from_slots(*form, condition, slot("system"), slot("shall"));
        assert_eq!(
            rows[form.as_str()][2],
            rendered.render(),
            "{}",
            form.as_str()
        );
    }
}

/// The words, in lowercase, that name the elements of `target`.
fn target_words(target: Target) -> &'static [&'static str] {
    match target {
        Target::Node => &["node"],
        Target::Component => &["component"],
        Target::Product => &["product"],
        Target::Decision => &["decision"],
        Target::NodeOrDecision => &["node", "decision"],
        Target::Option => &["option"],
        Target::Driver => &["driver"],
        other => panic!("no words name {other:?}"),
    }
}

/// The words, in lowercase, that name the records holding a field: "node" for every kind of
/// node at once, else each kind's own name, and "decision".
fn holder_words(name: &str) -> BTreeSet<String> {
    let holders: Vec<Kind> = read_kinds()
        .filter(|kind| kind.field(name).is_some())
        .collect();
    let mut nodes = read_kinds().filter_map(Kind::node_kind);
    let mut words = BTreeSet::new();
    if nodes.all(|node| holders.iter().any(|kind| kind.node_kind() == Some(node))) {
        words.insert("node".to_owned());
    } else {
        let held = holders.iter().filter_map(|kind| kind.node_kind());
        words.extend(held.map(|node| node.as_str().replace('_', " ")));
    }
    if holders.contains(&Kind::Decision) {
        words.insert("decision".to_owned());
    }
    words
}

/// What an edge field reaches: its own target, or the one its records' reference reaches.
fn edge_target(field: &Field) -> Target {
    let target = match field.shape.item() {
        Item::Record(kind) => kind.fields().iter().find_map(|f| f.shape.item().target()),
        item => item.target(),
    };
    target.unwrap_or_else(|| panic!("`{}` reaches nothing", field.name))
}

#[test]
fn each_edge_is_described_with_the_kinds_at_its_ends() {
    let markdown = format();
    let table = table(section(&markdown, "Edges"), "Field");
    let rows = rows_by_span(&table);
    let edges = [
        field::REALIZES,
        field::SCOPED_TO,
        field::SUBJECTS,
        field::MENTIONS,
    ];
    let mut sorted = edges.to_vec();
    sorted.sort_unstable();
    assert_eq!(rows.keys().copied().collect::<Vec<_>>(), sorted);
    for edge in edges {
        let holder = read_kinds().find_map(|kind| kind.field(edge)).unwrap().1;
        let (from, to) = (rows[edge][1].to_lowercase(), rows[edge][2].to_lowercase());
        for word in holder_words(edge) {
            assert!(from.contains(&word), "`{edge}` from {word}: {from}");
        }
        for word in target_words(edge_target(holder)) {
            assert!(to.contains(word), "`{edge}` to {word}: {to}");
        }
    }
}

#[test]
fn each_reference_field_is_shown_with_what_it_looks_for_by_name() {
    let markdown = format();
    let edges = section(&markdown, "Edges");
    let [by_name] = Kind::ByName.fields() else {
        panic!("a reference by name holds one field");
    };
    let record = format!("a record of one field, `{}`", by_name.name);
    assert!(edges.contains(&record), "{record}");
    let mut references: BTreeMap<&str, Target> = BTreeMap::new();
    for field in read_kinds().flat_map(Kind::fields) {
        if let Some(target) = field.shape.item().target() {
            let before = references.insert(field.name, target);
            assert!(
                before.is_none_or(|before| before == target),
                "{}",
                field.name
            );
        }
    }
    let table = table(edges, "Reference");
    let mut shown: BTreeMap<&str, String> = BTreeMap::new();
    for row in &table.rows {
        for name in spans(&row[0]) {
            shown.insert(name, row[1].to_lowercase());
        }
    }
    let names: Vec<&&str> = references.keys().collect();
    assert_eq!(shown.keys().collect::<Vec<_>>(), names);
    for (name, target) in references {
        for word in target_words(target) {
            assert!(
                shown[name].contains(word),
                "`{name}` looks for {word}: {}",
                shown[name]
            );
        }
    }
}

/// The `spec/lint.yaml` example, and the configuration the reader makes of it.
fn lint_example() -> (String, LintConfig) {
    let markdown = format();
    let start = markdown.find("```yaml path=spec/lint.yaml\n").unwrap();
    let body = markdown[start..].split_once('\n').unwrap().1;
    let text = body[..body.find("```").unwrap()].to_owned();
    let config = LintConfig::read(Some(text.as_bytes())).unwrap_or_else(|e| panic!("{e:?}"));
    (text, config)
}

/// The entries of the top-level list `list`, each as its scalar fields.
fn entries(text: &str, list: &str) -> Vec<BTreeMap<String, String>> {
    let mut entries: Vec<BTreeMap<String, String>> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if !line.starts_with(' ') {
            inside = line.strip_suffix(':') == Some(list);
            continue;
        }
        if !inside {
            continue;
        }
        let field = if let Some(first) = line.strip_prefix("  - ") {
            entries.push(BTreeMap::new());
            first
        } else if let Some(more) = line.strip_prefix("    ")
            && !more.starts_with([' ', '-'])
        {
            more
        } else {
            continue;
        };
        let (name, value) = field.split_once(':').unwrap();
        let value = value.trim().trim_matches('"').to_owned();
        entries.last_mut().unwrap().insert(name.to_owned(), value);
    }
    entries
}

#[test]
fn the_lint_configuration_shows_a_rule_entry_with_an_option() {
    let (text, config) = lint_example();
    assert_ne!(config, LintConfig::default());
    // Besides its rule, an entry holds `enabled`, `severity` or the options the rule takes.
    let with_option = entries(&text, "rules").iter().any(|entry| {
        RuleId::parse(&entry["rule"]).is_some()
            && entry
                .keys()
                .any(|name| !["rule", "enabled", "severity"].contains(&name.as_str()))
    });
    assert!(with_option, "{text}");
}

#[test]
fn the_lint_configuration_shows_a_suppression_with_its_reason() {
    let (text, config) = lint_example();
    let suppressions = entries(&text, "suppressions");
    assert!(!suppressions.is_empty(), "{text}");
    for entry in suppressions {
        let rule = RuleId::parse(&entry["rule"]).unwrap();
        let key = Key::parse(&entry["key"]).unwrap();
        assert!(config.suppresses(rule, &[key]), "{entry:?}");
        assert!(!entry["reason"].is_empty(), "{entry:?}");
    }
}

#[test]
fn the_trace_file_and_each_of_its_fields_are_described() {
    let markdown = format();
    let trace = section(&markdown, "The trace file");
    assert!(trace.contains(&format!("`{TRACE}`")));
    let tables = field_tables(trace);
    let records = [FILE_FIELDS, LINK_FIELDS, TEST_FIELDS];
    for (title, fields) in TRACE_TABLES.into_iter().zip(records) {
        let table = tables
            .get(title)
            .unwrap_or_else(|| panic!("no {title} table"));
        let names: Vec<&str> = table.iter().map(|(name, _)| name.as_str()).collect();
        assert_eq!(names, fields, "{title}");
    }
    let (_, format_mark) = &tables[TRACE_TABLES[0]][0];
    assert!(
        format_mark.contains(&format!("`{FORMAT}`")),
        "{format_mark}"
    );
}
