//! Citations of withdrawn labels: every publish rewrites a label withdrawn from a key to the
//! key's live label, in the text of each element whose `mentions` include the key. The text of
//! a record without `mentions` of its own, such as a statement or a decision's option, belongs
//! to the nearest record around it that has them. A citation qualified by a document, one in an
//! element that also mentions the label's live holder, and one of a label withdrawn from two
//! keys the element mentions are left as they are.

use crate::bindings::{Bindings, Citation};
use crate::codec::schema::{Item, Scalar, Shape, field};
use crate::codec::{Files, LABELS, Mode, Rec, SpecPath, Val, read_file, write_content};
use crate::identity::Key;
use crate::prose::label_spans;

use super::Relabel;

/// The files a publish rewrites, in canonical layout, and each citation it rewrote: the
/// element whose text held it, the withdrawn label and the label that replaced it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewrites {
    pub files: Files,
    pub rewritten: Vec<Relabel>,
}

/// The element whose `mentions` govern a text: its key and the keys it mentions.
struct Element {
    key: Key,
    mentions: Vec<Key>,
}

/// The bindings after allocation, which decide each citation.
struct Withdrawals<'a> {
    bindings: &'a Bindings,
}

/// Rewrites the citations of withdrawn labels in `files`, given the bindings after allocation.
/// A file whose text cites no withdrawn label is not read.
#[must_use]
pub fn rewrite(files: &Files, bindings: &Bindings) -> Rewrites {
    let mut rewrites = Rewrites::default();
    if !bindings.has_withdrawn() {
        return rewrites;
    }
    let withdrawals = Withdrawals { bindings };
    for (path, bytes) in files {
        // The bindings list withdrawn labels but hold no text that cites them.
        if *path == SpecPath::new(LABELS) {
            continue;
        }
        let cites = std::str::from_utf8(bytes).is_ok_and(|text| {
            label_spans(text)
                .iter()
                .any(|span| bindings.is_withdrawn(span.label))
        });
        let Some((role, mut content)) = cites
            .then(|| read_file(path, bytes, Mode::Strict).ok())
            .flatten()
        else {
            continue;
        };
        let before = rewrites.rewritten.len();
        for record in content.records_mut() {
            withdrawals.record(record, None, &mut rewrites.rewritten);
        }
        if rewrites.rewritten.len() > before {
            rewrites
                .files
                .insert(path.clone(), write_content(path, role, content));
        }
    }
    rewrites
}

impl Withdrawals<'_> {
    /// Rewrites the text of `record` and of the records inside it, by the `mentions` of the
    /// record itself when its kind has them, or else of `around`.
    fn record(&self, record: &mut Rec, around: Option<&Element>, out: &mut Vec<Relabel>) {
        let kind = record.kind;
        let own = kind.field(field::MENTIONS).and_then(|_| element(record));
        let governing = own.as_ref().or(around);
        for (name, value) in &mut record.fields {
            let (_, each) = kind.field(name).expect("a field the schema defines");
            self.value(value, each.shape, governing, out);
        }
    }

    /// Rewrites the text of one field's value, or of the records it holds.
    fn value(
        &self,
        value: &mut Val,
        shape: Shape,
        governing: Option<&Element>,
        out: &mut Vec<Relabel>,
    ) {
        let text = Item::Scalar(Scalar::Text);
        match (shape, value) {
            (Shape::One(item), Val::Scalar { text: prose, .. }) if item == text => {
                self.text(prose, governing, out);
            }
            (Shape::List(item), Val::List(items)) if item == text => {
                let prose = items.iter_mut().filter_map(|item| match item {
                    Val::Scalar { text, .. } => Some(text),
                    _ => None,
                });
                prose.for_each(|text| self.text(text, governing, out));
            }
            (_, value) => value
                .records_mut()
                .for_each(|child| self.record(child, governing, out)),
        }
    }

    /// Rewrites each citation in `text` that the governing element's `mentions` decide.
    fn text(&self, text: &mut String, element: Option<&Element>, out: &mut Vec<Relabel>) {
        let Some(element) = element else {
            return;
        };
        let mut found = Vec::new();
        for span in label_spans(text).into_iter().rev() {
            let mentioned = |key| element.mentions.contains(&key);
            let citation = if span.qualified {
                Citation::NotWithdrawn
            } else {
                self.bindings.citation(span.label, mentioned)
            };
            if let Citation::Rewritten(live) = citation {
                text.replace_range(span.range, &live.to_string());
                found.push(Relabel {
                    element: element.key,
                    withdrawn: span.label,
                    label: live,
                });
            }
        }
        out.extend(found.into_iter().rev());
    }
}

/// The element a record with `mentions` is: its key and the keys it mentions.
fn element(record: &Rec) -> Option<Element> {
    let key = Key::parse(record.text(field::KEY)?).ok()?;
    let mentions = record
        .list(field::MENTIONS)
        .iter()
        .filter_map(|mention| Key::parse(mention.text()?).ok())
        .collect();
    Some(Element { key, mentions })
}
