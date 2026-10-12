//! Citations of withdrawn labels: every publish rewrites a label withdrawn from a key to the
//! key's live label, in the text of each element whose `mentions` include the key. The text of
//! a record without `mentions` of its own, such as a statement or a decision's option, belongs
//! to the nearest record around it that has them. A citation qualified by a document, one in an
//! element that also mentions the label's live holder, and one of a label withdrawn from two
//! keys the element mentions are left as they are.

use std::collections::BTreeMap;

use crate::bindings::Bindings;
use crate::codec::schema::{Item, Scalar, Shape, field};
use crate::codec::{Files, Mode, Rec, Val, read_file, write_content};
use crate::identity::{Key, Label};
use crate::prose::label_spans;

/// One rewritten citation: the element whose text held it, the withdrawn label and the label
/// that replaced it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rewritten {
    pub element: Key,
    pub withdrawn: Label,
    pub label: Label,
}

/// The files a publish rewrites, in canonical layout, and each citation it rewrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Rewrites {
    pub files: Files,
    pub rewritten: Vec<Rewritten>,
}

/// The element whose `mentions` govern a text: its key and the keys it mentions.
struct Element {
    key: Key,
    mentions: Vec<Key>,
}

/// Each withdrawn label, with the keys it was withdrawn from and their live labels.
struct Withdrawals<'a> {
    from: BTreeMap<Label, Vec<(Key, Label)>>,
    bindings: &'a Bindings,
}

/// Rewrites the citations of withdrawn labels in `files`, given the bindings after allocation.
/// A tree whose bindings withdraw no label is not read.
#[must_use]
pub fn rewrite(files: &Files, bindings: &Bindings) -> Rewrites {
    let mut from: BTreeMap<Label, Vec<(Key, Label)>> = BTreeMap::new();
    for binding in bindings.iter() {
        for withdrawn in &binding.withdrawn {
            let holder = (binding.element.key(), binding.label);
            from.entry(*withdrawn).or_default().push(holder);
        }
    }
    let mut rewrites = Rewrites::default();
    if from.is_empty() {
        return rewrites;
    }
    let withdrawals = Withdrawals { from, bindings };
    for (path, bytes) in files {
        let Ok((role, mut content)) = read_file(path, bytes, Mode::Strict) else {
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
    fn record(&self, record: &mut Rec, around: Option<&Element>, out: &mut Vec<Rewritten>) {
        let kind = record.kind;
        let own = kind.field(field::MENTIONS).and_then(|_| element(record));
        let governing = own.as_ref().or(around);
        for (name, value) in &mut record.fields {
            if *name != field::CONFLICTS {
                let shape = kind.field(name).map(|(_, each)| each.shape);
                self.value(value, shape, governing, out);
            }
        }
    }

    /// Rewrites the text of one field's value, or of the records it holds.
    fn value(
        &self,
        value: &mut Val,
        shape: Option<Shape>,
        governing: Option<&Element>,
        out: &mut Vec<Rewritten>,
    ) {
        let text = Item::Scalar(Scalar::Text);
        match (shape, value) {
            (Some(Shape::One(item)), Val::Scalar { text: prose, .. }) if item == text => {
                self.text(prose, governing, out);
            }
            (Some(Shape::List(item)), Val::List(items)) if item == text => {
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
    fn text(&self, text: &mut String, element: Option<&Element>, out: &mut Vec<Rewritten>) {
        let Some(element) = element else {
            return;
        };
        let mut found = Vec::new();
        for span in label_spans(text).into_iter().rev() {
            if let Some(live) = self
                .live_label(span.label, element)
                .filter(|_| !span.qualified)
            {
                text.replace_range(span.range, &live.to_string());
                found.push(Rewritten {
                    element: element.key,
                    withdrawn: span.label,
                    label: live,
                });
            }
        }
        out.extend(found.into_iter().rev());
    }

    /// The live label a citation of `withdrawn` becomes in `element`: the label of the one key
    /// it was withdrawn from that the element mentions, unless the element mentions its holder.
    fn live_label(&self, withdrawn: Label, element: &Element) -> Option<Label> {
        let candidates = self.from.get(&withdrawn)?;
        let mentioned: Vec<&(Key, Label)> = candidates
            .iter()
            .filter(|(key, _)| element.mentions.contains(key))
            .collect();
        let [(_, live)] = mentioned.as_slice() else {
            return None;
        };
        let holder = self.bindings.resolve(withdrawn);
        let holder_mentioned =
            holder.is_some_and(|holder| element.mentions.contains(&holder.key()));
        (!holder_mentioned).then_some(*live)
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
