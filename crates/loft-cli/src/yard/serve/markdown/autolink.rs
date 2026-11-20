//! Bare addresses made links, as GitHub's extended autolink makes them. `pulldown-cmark` reads
//! `CommonMark` alone and has no such extension, and linking the parsed text is too late: by then
//! an `__` or `*` within an address has become emphasis. GitHub reads an address from the raw
//! text as it parses; with no hook into `pulldown-cmark`'s parse, each address is read from the
//! raw source of a first parse instead, stood in for by one character, and the Markdown is parsed
//! again.

use std::borrow::Cow;
use std::mem::{self, Discriminant};
use std::ops::Range;

use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

use super::{SCHEMES, parser};

// Supplementary private use area B: the icon fonts a terminal draws from use the other private use
// areas, so no Markdown is expected to hold one. The parser reads it as a letter, as it reads the
// letter an address begins with.
const FIRST_STAND_IN: u32 = 0x10_0000;

// The area's last two code points are noncharacters.
const STAND_INS: usize = 0xFFFE;

/// The events of `markdown` with its bare addresses made links, or `None` where it has none to
/// make.
pub fn linked(markdown: &str) -> Option<Vec<Event<'static>>> {
    let autolinks = Autolinks::new(markdown)?;
    let all: Vec<usize> = (0..autolinks.found.len()).collect();
    autolinks
        .linked(&all)
        .or_else(|| autolinks.linked(&autolinks.keepable(&all)))
}

struct Autolinks<'a> {
    markdown: &'a str,
    found: Vec<Found<'a>>,
    shape: Vec<Shape>,
}

impl<'a> Autolinks<'a> {
    fn new(markdown: &'a str) -> Option<Self> {
        // Most Markdown holds no address, and the first parse is spent only on what does.
        next_scheme(markdown, 0, markdown.len())?;
        let events: Vec<_> = parser(markdown).into_offset_iter().collect();
        // A stand-in the Markdown holds already, written out or as an entity, could take an
        // address's place where the second parse reads the address into something else, such as
        // a link's definition.
        if holds_a_stand_in(markdown)
            || events
                .iter()
                .any(|(event, _)| matches!(event, Event::Text(text) if holds_a_stand_in(text)))
        {
            return None;
        }
        let mut found = addresses(markdown, &events);
        found.truncate(STAND_INS);
        if found.is_empty() {
            return None;
        }
        Some(Autolinks {
            markdown,
            found,
            shape: events
                .iter()
                .filter_map(|(event, _)| shape(event))
                .collect(),
        })
    }

    // A stand-in can complete what an address's characters had broken, such as a link whose
    // destination held `(`, or an HTML tag whose attribute held `=`. The second parse is used only
    // where it keeps the first one's shape, with one stand-in for each address, each in text
    // outside a link.
    fn linked(&self, kept: &[usize]) -> Option<Vec<Event<'static>>> {
        let mut source = String::with_capacity(self.markdown.len());
        let mut from = 0;
        for &index in kept {
            let range = &self.found[index].range;
            source.push_str(&self.markdown[from..range.start]);
            source.push(char::from_u32(FIRST_STAND_IN + u32::try_from(index).ok()?)?);
            from = range.end;
        }
        source.push_str(&self.markdown[from..]);
        let events: Vec<_> = parser(&source).collect();
        if events
            .iter()
            .filter_map(shape)
            .ne(self.shape.iter().copied())
            || stand_ins_outside_links(&events)? != kept.len()
        {
            return None;
        }
        let mut linked = Vec::with_capacity(events.len() + 2 * kept.len());
        for event in events {
            match event {
                Event::Text(text) if holds_a_stand_in(&text) => self.split(&text, &mut linked),
                other => linked.push(other.into_static()),
            }
        }
        Some(linked)
    }

    // The addresses of `failing`, which together change the parse, that can be kept: halves are
    // tried in turn, so that one address that changes the parse costs a few parses rather than
    // one for each address, and leaves the others linked.
    fn keepable(&self, failing: &[usize]) -> Vec<usize> {
        if failing.len() < 2 {
            return Vec::new();
        }
        let (left, right) = failing.split_at(failing.len() / 2);
        [left, right]
            .into_iter()
            .flat_map(|half| {
                if self.linked(half).is_some() {
                    half.to_vec()
                } else {
                    self.keepable(half)
                }
            })
            .collect()
    }

    fn split(&self, text: &str, linked: &mut Vec<Event<'static>>) {
        let mut plain = 0;
        for (at, character) in text.char_indices() {
            let Some(index) = stand_in(character) else {
                continue;
            };
            if plain < at {
                linked.push(Event::Text(text[plain..at].to_owned().into()));
            }
            let address = &self.found[index].address;
            linked.push(Event::Start(Tag::Link {
                link_type: LinkType::Autolink,
                dest_url: address.to_string().into(),
                title: CowStr::Borrowed(""),
                id: CowStr::Borrowed(""),
            }));
            linked.push(Event::Text(address.to_string().into()));
            linked.push(Event::End(TagEnd::Link));
            plain = at + character.len_utf8();
        }
        if plain < text.len() {
            linked.push(Event::Text(text[plain..].to_owned().into()));
        }
    }
}

// How many stand-ins lie in text outside a link or an image, or `None` where one lies within one.
fn stand_ins_outside_links(events: &[Event<'_>]) -> Option<usize> {
    let mut nested = 0usize;
    let mut outside = 0;
    for event in events {
        match event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => nested += 1,
            Event::End(TagEnd::Link | TagEnd::Image) => nested -= 1,
            Event::Text(text) if nested > 0 && holds_a_stand_in(text) => return None,
            Event::Text(text) => {
                outside += text
                    .chars()
                    .filter(|&each| stand_in(each).is_some())
                    .count();
            }
            _ => {}
        }
    }
    Some(outside)
}

fn holds_a_stand_in(text: &str) -> bool {
    text.chars().any(|each| stand_in(each).is_some())
}

fn stand_in(character: char) -> Option<usize> {
    let index = usize::try_from(u32::from(character).checked_sub(FIRST_STAND_IN)?).ok()?;
    (index < STAND_INS).then_some(index)
}

// What two parses of one Markdown share where a stand-in changed nothing but its address: how
// their elements close, and every event but text and line breaks. An emphasis may close
// elsewhere, as an address takes in the marks within it, and a hard break becomes a soft one
// where an address takes in its backslash, as on GitHub.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Shape {
    End(TagEnd),
    Event(Discriminant<Event<'static>>),
}

fn shape(event: &Event<'_>) -> Option<Shape> {
    match event {
        Event::Text(_)
        | Event::SoftBreak
        | Event::HardBreak
        | Event::Start(_)
        | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => None,
        Event::End(end) => Some(Shape::End(*end)),
        other => Some(Shape::Event(mem::discriminant(
            &other.clone().into_static(),
        ))),
    }
}

struct Found<'a> {
    range: Range<usize>,
    address: Cow<'a, str>,
}

#[derive(Clone, Copy)]
struct Open {
    end: usize,
    guarded: bool,
    in_cell: bool,
}

fn addresses<'a>(markdown: &'a str, events: &[(Event<'_>, Range<usize>)]) -> Vec<Found<'a>> {
    let mut stops: Vec<usize> = events
        .iter()
        .filter(|(event, _)| stops_an_address(event))
        .map(|(_, range)| range.start)
        .collect();
    stops.sort_unstable();
    let mut scan = Scan {
        markdown,
        stops: &stops,
        found: Vec::new(),
    };
    let mut open: Vec<Open> = Vec::new();
    for (event, range) in events {
        match event {
            // An address takes in the emphasis marks within it, as on GitHub, so an emphasis
            // neither ends an address nor is closed by one.
            Event::Start(Tag::Emphasis | Tag::Strong | Tag::Strikethrough)
            | Event::End(TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough) => {}
            Event::Start(tag) => {
                let outer = open.last().copied();
                open.push(Open {
                    end: range.end,
                    guarded: guards(tag) || outer.is_some_and(|outer| outer.guarded),
                    in_cell: matches!(tag, Tag::TableCell)
                        || outer.is_some_and(|outer| outer.in_cell),
                });
            }
            Event::End(_) => {
                open.pop();
            }
            Event::Text(_) if !open.last().is_some_and(|open| open.guarded) => {
                // An address ends within its element, such as a table cell, so that standing in
                // for it leaves the parse's structure as it was.
                let (cap, in_cell) = open
                    .last()
                    .map_or((markdown.len(), false), |open| (open.end, open.in_cell));
                scan.text(range.clone(), cap, in_cell);
            }
            _ => {}
        }
    }
    scan.found
}

const fn guards(tag: &Tag<'_>) -> bool {
    matches!(
        tag,
        Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_) | Tag::HtmlBlock
    )
}

const fn stops_an_address(event: &Event<'_>) -> bool {
    match event {
        Event::Code(_) | Event::InlineHtml(_) | Event::Html(_) | Event::FootnoteReference(_) => {
            true
        }
        Event::Start(tag) => guards(tag),
        _ => false,
    }
}

struct Scan<'a, 's> {
    markdown: &'a str,
    stops: &'s [usize],
    found: Vec<Found<'a>>,
}

impl Scan<'_, '_> {
    fn text(&mut self, range: Range<usize>, cap: usize, in_cell: bool) {
        let mut from = range
            .start
            .max(self.found.last().map_or(0, |each| each.range.end));
        while let Some((start, scheme)) = next_scheme(self.markdown, from, range.end) {
            from = start + scheme.len();
            let limit = cap.min(self.next_stop(start));
            let end = if scheme == "mailto:" {
                mailto_end(self.markdown, start, limit)
            } else {
                url_end(self.markdown, start, scheme, limit)
            };
            if let Some(end) = end {
                self.found.push(Found {
                    range: start..end,
                    address: cell_unescaped(&self.markdown[start..end], in_cell),
                });
                from = end;
            }
        }
    }

    fn next_stop(&self, start: usize) -> usize {
        let index = self.stops.partition_point(|&stop| stop <= start);
        self.stops.get(index).copied().unwrap_or(usize::MAX)
    }
}

// A table's cells are split at `|` before they are parsed, as GitHub splits them.
fn cell_unescaped(address: &str, in_cell: bool) -> Cow<'_, str> {
    if in_cell && address.contains("\\|") {
        Cow::Owned(address.replace("\\|", "|"))
    } else {
        Cow::Borrowed(address)
    }
}

// `cmark-gfm`'s `url_match`, so that an address is linked where GitHub links it.
fn url_end(markdown: &str, start: usize, scheme: &str, limit: usize) -> Option<usize> {
    // GitHub reads back over the letters before `:` for the scheme, so a letter before it makes
    // another scheme.
    if preceded_by(markdown, start, u8::is_ascii_alphabetic) {
        return None;
    }
    let host = &markdown[start + scheme.len()..limit];
    // Non-ASCII punctuation begins a host here: the standard library has no class for it.
    let begins = host
        .chars()
        .next()
        .is_some_and(|first| !first.is_whitespace() && !first.is_ascii_punctuation());
    if !begins || underscored(host.as_bytes()) {
        return None;
    }
    let rest = &markdown[start..limit];
    let length = rest.find(ends_an_address).unwrap_or(rest.len());
    let end = start + trimmed(&rest[..length]);
    (end > start + scheme.len()).then_some(end)
}

// `cmark-gfm`'s `check_domain`, which takes host names apart from domain names, where an
// underscore is allowed, and lets a long one through rather than read it again and again.
fn underscored(host: &[u8]) -> bool {
    let (mut dots, mut before_last, mut last) = (0, 0, 0);
    let mut index = 1;
    while index + 1 < host.len() {
        if host[index] == b'\\' && index + 2 < host.len() {
            index += 1;
        }
        match host[index] {
            b'_' => last += 1,
            b'.' => {
                before_last = last;
                last = 0;
                dots += 1;
            }
            b'-' => {}
            byte if byte.is_ascii()
                && !byte.is_ascii_punctuation()
                && !matches!(byte, b'\t' | b'\n' | 0x0C | b'\r' | b' ') => {}
            _ => break,
        }
        index += 1;
    }
    (before_last > 0 || last > 0) && dots <= 10
}

// GitHub links `mailto:` only with the email address it finds after it, in its postprocess of
// text: the scheme in lowercase after no letter or digit, and a domain with a dot that ends in a
// letter.
fn mailto_end(markdown: &str, start: usize, limit: usize) -> Option<usize> {
    if !markdown[start..].starts_with("mailto:")
        || preceded_by(markdown, start, u8::is_ascii_alphanumeric)
    {
        return None;
    }
    let after = start + "mailto:".len();
    let bytes = &markdown.as_bytes()[after..limit];
    let at = bytes
        .iter()
        .position(|byte| !byte.is_ascii_alphanumeric() && !b".+-_".contains(byte))?;
    let domain = bytes[at..].strip_prefix(b"@")?;
    let (mut length, mut dots) = (0, 0);
    while let Some(&byte) = domain.get(length) {
        let dot = byte == b'.'
            && domain
                .get(length + 1)
                .is_some_and(u8::is_ascii_alphanumeric);
        if byte == b'@' {
            return None;
        }
        if !(dot || byte.is_ascii_alphanumeric() || byte == b'-' || byte == b'_') {
            break;
        }
        dots += usize::from(dot);
        length += 1;
    }
    let ends_in_a_letter = length > 0 && domain[length - 1].is_ascii_alphabetic();
    (dots > 0 && ends_in_a_letter).then_some(after + at + 1 + length)
}

fn preceded_by(markdown: &str, start: usize, class: fn(&u8) -> bool) -> bool {
    start
        .checked_sub(1)
        .is_some_and(|before| class(&markdown.as_bytes()[before]))
}

// `cmark-gfm`'s `cmark_isspace`, which takes no other whitespace.
fn ends_an_address(next: char) -> bool {
    matches!(next, ' ' | '\t' | '\n' | '\r' | '<')
}

// Found from each `:` rather than by searching for each scheme in turn, so that a line of many
// addresses is read once.
fn next_scheme(markdown: &str, from: usize, end: usize) -> Option<(usize, &'static str)> {
    let bytes = markdown.as_bytes();
    markdown
        .get(from..end)?
        .match_indices(':')
        .find_map(|(at, _)| {
            let colon = from + at;
            SCHEMES.iter().find_map(|scheme| {
                let (name, after) = scheme.split_once(':')?;
                let start = colon
                    .checked_sub(name.len())
                    .filter(|&start| start >= from)?;
                let named = bytes[start..colon].eq_ignore_ascii_case(name.as_bytes());
                (named && bytes[colon + 1..].starts_with(after.as_bytes()))
                    .then_some((start, *scheme))
            })
        })
}

// `cmark-gfm`'s `autolink_delim`, so that an address ends where it ends on GitHub. Cutting an
// unbalanced `]` is ours: GitHub links no address within brackets at all.
fn trimmed(candidate: &str) -> usize {
    let bytes = candidate.as_bytes();
    let count = |wanted: char| candidate.matches(wanted).count();
    let (opened, mut closed) = (count('('), count(')'));
    let (opened_square, mut closed_square) = (count('['), count(']'));
    let mut end = bytes.len();
    while let Some(&last) = bytes[..end].last() {
        match last {
            b')' if closed > opened => closed -= 1,
            b']' if closed_square > opened_square => closed_square -= 1,
            b'?' | b'!' | b'.' | b',' | b':' | b'*' | b'_' | b'~' | b'\'' | b'"' => {}
            b';' => {
                end = entity_start(&bytes[..end]).unwrap_or(end - 1);
                continue;
            }
            _ => return end,
        }
        end -= 1;
    }
    end
}

fn entity_start(bytes: &[u8]) -> Option<usize> {
    let letters = bytes[..bytes.len() - 1]
        .iter()
        .rev()
        .take_while(|byte| byte.is_ascii_alphabetic())
        .count();
    let amp = bytes.len().checked_sub(letters + 2)?;
    (letters > 0 && bytes[amp] == b'&').then_some(amp)
}

#[cfg(test)]
mod tests;
