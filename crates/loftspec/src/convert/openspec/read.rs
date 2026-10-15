//! The reader of one main spec, line by line as `OpenSpec`'s archive command reads it: ATX headings
//! outside fenced code, front matter skipped, and requirements only from `### Requirement:`
//! headings inside the section `Requirements`.

use std::borrow::Borrow;
use std::ops::Range;

/// A main spec's lines, with a byte order mark removed and every line ending read as LF.
#[derive(Debug)]
pub(super) struct Source {
    lines: Vec<String>,
    fenced: Vec<bool>,
    /// The 1-based line that closes the front matter, if the spec opens with front matter.
    pub front_matter: Option<usize>,
    start: usize,
}

/// A heading outside fenced code: its 0-based line, its level and its text.
#[derive(Debug, Clone)]
struct Heading {
    index: usize,
    level: usize,
    text: String,
}

/// A heading with the text up to its first heading and the headings nested in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Part {
    pub name: String,
    pub line: usize,
    pub text: Range<usize>,
    pub children: Vec<Self>,
}

/// A requirement: its name, the line of its heading, its text and its level-4 children.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Requirement {
    pub name: String,
    pub line: usize,
    pub text: Range<usize>,
    pub children: Vec<Child>,
}

/// A level-4 child of a requirement, as a part without children; `scenario` holds the
/// scenario's name when it is one.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Child {
    pub scenario: Option<String>,
    pub part: Part,
}

/// A main spec as read.
#[derive(Debug, Default)]
pub(super) struct Spec {
    /// The first level-1 heading before any level-2 heading, with its line.
    pub title: Option<(String, usize)>,
    /// Text before the first level-2 heading, other than the title, as the part
    /// `Introduction` that begins at its first line.
    pub introduction: Option<Part>,
    pub purpose: Option<Part>,
    /// The level-2 sections other than `Purpose` and `Requirements`, each with whether it is a
    /// delta section.
    pub sections: Vec<(Part, bool)>,
    /// Text of the section `Requirements` that no requirement holds.
    pub stray: Vec<Part>,
    pub requirements: Vec<Requirement>,
}

const DELTA: [&str; 4] = ["added", "modified", "removed", "renamed"];

impl Source {
    pub fn new(text: &str) -> Self {
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let text = text.replace("\r\n", "\n").replace('\r', "\n");
        let lines: Vec<String> = text.split('\n').map(str::to_owned).collect();
        let fenced = fence_mask(&lines);
        let front_matter = (lines.first().map(|line| line.trim_end()) == Some("---"))
            .then(|| {
                lines
                    .iter()
                    .skip(1)
                    .position(|line| line.trim_end() == "---")
            })
            .flatten()
            .map(|position| position + 2);
        Self {
            start: front_matter.unwrap_or(0),
            lines,
            fenced,
            front_matter,
        }
    }

    /// The lines of `range` as written, without leading and trailing blank lines; `None` when
    /// nothing else is left.
    pub fn verbatim(&self, range: Range<usize>) -> Option<String> {
        verbatim(&self.lines[range])
    }

    /// Every line of the source.
    pub fn whole(&self) -> Range<usize> {
        0..self.lines.len()
    }

    /// The 1-based line where the text of `range` begins, or `None` when it is blank.
    pub fn first_line(&self, range: Range<usize>) -> Option<usize> {
        let start = range.start;
        self.lines[range]
            .iter()
            .position(|line| !line.trim().is_empty())
            .map(|position| start + position + 1)
    }

    /// Each line of `range` with its 0-based index and whether fenced code holds it.
    pub fn lines(&self, range: Range<usize>) -> impl Iterator<Item = (usize, &str, bool)> {
        range.map(|index| (index, self.lines[index].as_str(), self.fenced[index]))
    }

    fn headings(&self) -> Vec<Heading> {
        (self.start..self.lines.len())
            .filter(|&index| !self.fenced[index])
            .filter_map(|index| {
                heading(&self.lines[index]).map(|(level, text)| Heading { index, level, text })
            })
            .collect()
    }

    /// Reads the spec.
    pub fn read(&self) -> Spec {
        let headings = self.headings();
        let end = self.lines.len();
        let mut spec = Spec::default();
        let title = headings
            .iter()
            .find(|heading| heading.level <= 2)
            .filter(|heading| heading.level == 1);
        spec.title = title.map(|heading| (heading.text.clone(), heading.index + 1));
        let top: Vec<&Heading> = headings
            .iter()
            .filter(|heading| heading.level <= 2)
            .filter(|heading| title.is_none_or(|title| heading.index != title.index))
            .collect();
        // Text before the title keeps the title line in the introduction, so none is lost.
        let lead = title
            .filter(|title| self.first_line(self.start..title.index).is_none())
            .map_or(self.start, |title| title.index + 1);
        let lead_end = top.first().map_or(end, |heading| heading.index);
        spec.introduction = self.first_line(lead..lead_end).map(|line| Part {
            name: "Introduction".to_owned(),
            line,
            text: lead..lead_end,
            children: Vec::new(),
        });
        let mut requirements_seen = false;
        for (position, heading) in top.iter().enumerate() {
            let segment_end = top.get(position + 1).map_or(end, |next| next.index);
            let inner = between(&headings, heading.index, segment_end);
            let name = heading.text.to_lowercase();
            if heading.level == 2 && name == "purpose" && spec.purpose.is_none() {
                let text = heading.index + 1..segment_end;
                spec.purpose = Some(part(heading, text, Vec::new()));
            } else if heading.level == 2 && name == "requirements" && !requirements_seen {
                requirements_seen = true;
                self.requirements(&mut spec, heading.index + 1..segment_end, inner);
            } else {
                let delta = heading.level == 2 && is_delta(&name);
                let text_end = inner.first().map_or(segment_end, |first| first.index);
                let text = heading.index + 1..text_end;
                spec.sections
                    .push((part(heading, text, tree(inner, segment_end)), delta));
            }
        }
        spec
    }

    /// Reads the section `Requirements`, whose text is `range` and whose headings are `inner`.
    fn requirements(&self, spec: &mut Spec, range: Range<usize>, inner: &[Heading]) {
        let threes: Vec<&Heading> = inner.iter().filter(|heading| heading.level <= 3).collect();
        let preamble_end = threes.first().map_or(range.end, |first| first.index);
        if let Some(line) = self.first_line(range.start..preamble_end) {
            spec.stray.push(Part {
                name: "Requirements".to_owned(),
                line,
                text: range.start..preamble_end,
                children: Vec::new(),
            });
        }
        for (position, heading) in threes.iter().enumerate() {
            let block_end = threes
                .get(position + 1)
                .map_or(range.end, |next| next.index);
            let nested = between(inner, heading.index, block_end);
            let text_end = nested.first().map_or(block_end, |first| first.index);
            if let Some(name) = requirement_name(&heading.text) {
                spec.requirements
                    .push(requirement(name, heading, nested, block_end));
            } else {
                let children = tree(nested, block_end);
                spec.stray
                    .push(part(heading, heading.index + 1..text_end, children));
            }
        }
    }
}

/// The headings of `headings`, which are in line order, that lie after the line `after` and
/// before the line `before`.
fn between(headings: &[Heading], after: usize, before: usize) -> &[Heading] {
    let start = headings.partition_point(|heading| heading.index <= after);
    let end = headings.partition_point(|heading| heading.index < before);
    &headings[start..end.max(start)]
}

/// Lines as written, without leading and trailing blank lines; `None` when nothing else is
/// left.
pub(super) fn verbatim<S: Borrow<str>>(lines: &[S]) -> Option<String> {
    let blank = |line: &S| line.borrow().trim().is_empty();
    let first = lines.iter().position(|line| !blank(line))?;
    let last = lines.iter().rposition(|line| !blank(line))?;
    Some(lines[first..=last].join("\n"))
}

fn part(heading: &Heading, text: Range<usize>, children: Vec<Part>) -> Part {
    Part {
        name: heading.text.clone(),
        line: heading.index + 1,
        text,
        children,
    }
}

/// The headings of `headings`, nested by level, each holding its text up to the next heading;
/// the last runs to `end`.
fn tree(headings: &[Heading], end: usize) -> Vec<Part> {
    let mut parts = Vec::new();
    let mut position = 0;
    while let Some(heading) = headings.get(position) {
        let inner_end = headings[position + 1..]
            .iter()
            .position(|next| next.level <= heading.level)
            .map_or(headings.len(), |offset| position + 1 + offset);
        let block_end = headings.get(inner_end).map_or(end, |next| next.index);
        let text_end = headings
            .get(position + 1)
            .map_or(block_end, |next| next.index);
        let children = tree(&headings[position + 1..inner_end], block_end);
        parts.push(part(heading, heading.index + 1..text_end, children));
        position = inner_end;
    }
    parts
}

/// A requirement from its heading, the headings nested in its block and the block's end.
fn requirement(name: String, heading: &Heading, nested: &[Heading], end: usize) -> Requirement {
    let fours: Vec<&Heading> = nested.iter().filter(|other| other.level <= 4).collect();
    let text_end = fours.first().map_or(end, |first| first.index);
    let children = fours
        .iter()
        .enumerate()
        .map(|(position, child)| {
            let child_end = fours.get(position + 1).map_or(end, |next| next.index);
            Child {
                scenario: scenario_name(&child.text),
                part: part(child, child.index + 1..child_end, Vec::new()),
            }
        })
        .collect();
    Requirement {
        name,
        line: heading.index + 1,
        text: heading.index + 1..text_end,
        children,
    }
}

/// Which lines fenced code holds, its opening and closing lines included: a fence opens with a
/// run of three or more backticks or tildes and closes with a run of the same character at
/// least as long and nothing else; a fence left open runs to the end.
fn fence_mask(lines: &[String]) -> Vec<bool> {
    let mut mask = vec![false; lines.len()];
    let mut open: Option<(char, usize)> = None;
    for (index, line) in lines.iter().enumerate() {
        let run = fence_run(line);
        match open {
            None => {
                if let Some((marker, length, _)) = run {
                    open = Some((marker, length));
                    mask[index] = true;
                }
            }
            Some((marker, length)) => {
                mask[index] = true;
                if run.is_some_and(|(found, found_length, alone)| {
                    found == marker && found_length >= length && alone
                }) {
                    open = None;
                }
            }
        }
    }
    mask
}

/// The fence run a line opens with: its character, its length and whether nothing but
/// whitespace follows it.
fn fence_run(line: &str) -> Option<(char, usize, bool)> {
    let trimmed = line.trim_start();
    let marker = trimmed.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let length = trimmed.chars().take_while(|c| *c == marker).count();
    (length >= 3).then(|| {
        let rest = &trimmed[length * marker.len_utf8()..];
        (marker, length, rest.trim().is_empty())
    })
}

/// An ATX heading's level and text: one to six `#` at the start of the line, whitespace, and
/// text with a closing run of `#` after a space or tab removed.
fn heading(line: &str) -> Option<(usize, String)> {
    let level = line.chars().take_while(|c| *c == '#').count();
    if !(1..=6).contains(&level) {
        return None;
    }
    let rest = &line[level..];
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    let text = without_closing_run(rest).trim().to_owned();
    (!text.is_empty()).then_some((level, text))
}

fn without_closing_run(text: &str) -> &str {
    let trimmed = text.trim_end_matches([' ', '\t']);
    let without = trimmed.trim_end_matches('#');
    if without.len() < trimmed.len() && without.ends_with([' ', '\t']) {
        without
    } else {
        text
    }
}

/// The name a level-3 heading gives a requirement, if it is a requirement heading.
fn requirement_name(text: &str) -> Option<String> {
    let rest = strip_prefix_ignoring_case(text, "requirement:")?;
    let name = rest.trim();
    (!name.is_empty()).then(|| name.to_owned())
}

/// The scenario name of a level-4 heading that begins with `Scenario:`.
fn scenario_name(text: &str) -> Option<String> {
    strip_prefix_ignoring_case(text, "scenario:").map(|rest| rest.trim().to_owned())
}

fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    text.get(..prefix.len())
        .filter(|head| head.eq_ignore_ascii_case(prefix))
        .map(|_| &text[prefix.len()..])
}

/// Whether a level-2 heading, in lower case, names a delta section.
fn is_delta(name: &str) -> bool {
    name.split_once(char::is_whitespace)
        .is_some_and(|(operation, rest)| {
            DELTA.contains(&operation) && rest.trim() == "requirements"
        })
}
