//! The Markdown of the skill's references as its tests read it: tables, their rows and cells,
//! code spans, sections, and the field tables that describe a record's fields.

use std::collections::BTreeMap;

/// A Markdown table: its header's cells, then each row's.
pub struct Table {
    pub header: Vec<String>,
    pub rows: Vec<Vec<String>>,
}

fn cells(line: &str) -> Vec<String> {
    let line = line.trim();
    let line = line.strip_prefix('|').unwrap_or(line);
    let line = line.strip_suffix('|').unwrap_or(line);
    line.split('|').map(|cell| cell.trim().to_owned()).collect()
}

/// Every table of `markdown`, in order.
pub fn tables(markdown: &str) -> Vec<Table> {
    let mut found = Vec::new();
    let mut lines = markdown.lines().peekable();
    while let Some(line) = lines.next() {
        if !line.starts_with('|') {
            continue;
        }
        let delimiter = lines.next().unwrap_or_default();
        assert!(
            delimiter.starts_with("|---"),
            "a table without a delimiter: {line}"
        );
        let mut rows = Vec::new();
        while let Some(row) = lines.next_if(|line| line.starts_with('|')) {
            rows.push(cells(row));
        }
        found.push(Table {
            header: cells(line),
            rows,
        });
    }
    found
}

/// The table of `markdown` whose header opens with `first`.
pub fn table(markdown: &str, first: &str) -> Table {
    tables(markdown)
        .into_iter()
        .find(|table| table.header[0] == first)
        .unwrap_or_else(|| panic!("no table opens with {first}"))
}

/// The code spans of `text`, in order.
pub fn spans(text: &str) -> Vec<&str> {
    text.split('`').skip(1).step_by(2).collect()
}

/// The rows of `table` by the code span that opens each.
pub fn rows_by_span(table: &Table) -> BTreeMap<&str, &[String]> {
    let rows = table.rows.iter().map(|row| (spans(&row[0])[0], &row[..]));
    rows.collect()
}

/// The section of `markdown` under the heading `## <heading>`.
pub fn section<'a>(markdown: &'a str, heading: &str) -> &'a str {
    let start = markdown
        .find(&format!("\n## {heading}\n"))
        .unwrap_or_else(|| panic!("no section {heading}"))
        + 1;
    let rest = &markdown[start..];
    let end = rest[3..].find("\n## ").map_or(rest.len(), |end| end + 3);
    &rest[..end]
}

/// The fields a table describes, each beside its mark.
pub type Described = Vec<(String, String)>;

/// The fields each field table describes, by the record it names: a header cell `<record>
/// field` heads a column of fields, each beside its mark in the next column.
pub fn field_tables(markdown: &str) -> BTreeMap<String, Described> {
    let mut found = BTreeMap::new();
    for table in tables(markdown) {
        for (column, title) in table.header.iter().enumerate() {
            let Some(record) = title.strip_suffix(" field") else {
                continue;
            };
            assert!(!found.contains_key(record), "two tables describe {record}");
            let described = table.rows.iter().flat_map(|row| {
                let mark = &row[column + 1];
                spans(&row[column])
                    .into_iter()
                    .map(|name| (name.to_owned(), mark.clone()))
            });
            found.insert(record.to_owned(), described.collect());
        }
    }
    found
}
