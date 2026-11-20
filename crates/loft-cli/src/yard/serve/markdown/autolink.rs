//! Bare addresses made links, as GitHub's extended autolink makes them. `pulldown-cmark` reads
//! `CommonMark` alone and has no such extension, and linking the parsed text is too late: by then
//! an `__` or `*` within an address has become emphasis. So, as GitHub does, each address is read
//! from the raw source of a first parse, stood in for by one character, and the Markdown is
//! parsed again.

use std::collections::VecDeque;
use std::mem;
use std::ops::Range;

use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

const SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];

// A private use character: no Markdown is expected to hold one, and the parser reads it as it
// reads a letter, so the emphasis around an address parses as it did around the address.
const FIRST_STAND_IN: u32 = 0xF_0000;

const STAND_INS: usize = 0xFFFE;

pub fn stood_in<'a>(
    markdown: &'a str,
    events: impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> Option<(String, Vec<&'a str>)> {
    // A Markdown that holds a stand-in already is left unlinked rather than misread.
    if !markdown.contains(':') || markdown.chars().any(|each| stand_in(each).is_some()) {
        return None;
    }
    let mut found = addresses(markdown, events);
    found.truncate(STAND_INS);
    if found.is_empty() {
        return None;
    }
    let mut source = String::with_capacity(markdown.len());
    let mut from = 0;
    for (index, address) in found.iter().enumerate() {
        source.push_str(&markdown[from..address.start]);
        source.extend(char::from_u32(FIRST_STAND_IN + u32::try_from(index).ok()?));
        from = address.end;
    }
    source.push_str(&markdown[from..]);
    let addresses = found
        .into_iter()
        .map(|address| &markdown[address])
        .collect();
    Some((source, addresses))
}

fn stand_in(character: char) -> Option<usize> {
    let index = usize::try_from(u32::from(character).checked_sub(FIRST_STAND_IN)?).ok()?;
    (index < STAND_INS).then_some(index)
}

pub struct Links<'s, 'r, I: Iterator<Item = Event<'s>>> {
    events: I,
    addresses: &'r [&'r str],
    pending: VecDeque<Event<'s>>,
    nested: usize,
}

impl<'s, 'r, I: Iterator<Item = Event<'s>>> Links<'s, 'r, I> {
    pub const fn new(events: I, addresses: &'r [&'r str]) -> Self {
        Self {
            events,
            addresses,
            pending: VecDeque::new(),
            nested: 0,
        }
    }

    fn split(&self, text: &str) -> VecDeque<Event<'s>> {
        let mut events = VecDeque::new();
        let mut plain = String::new();
        for character in text.chars() {
            let Some(address) = stand_in(character).and_then(|index| self.addresses.get(index))
            else {
                plain.push(character);
                continue;
            };
            // The parse may put an address in a link's text; a link within a link is no link.
            if self.nested > 0 {
                plain.push_str(address);
                continue;
            }
            if !plain.is_empty() {
                events.push_back(Event::Text(mem::take(&mut plain).into()));
            }
            events.push_back(Event::Start(Tag::Link {
                link_type: LinkType::Autolink,
                dest_url: (*address).to_owned().into(),
                title: CowStr::Borrowed(""),
                id: CowStr::Borrowed(""),
            }));
            events.push_back(Event::Text((*address).to_owned().into()));
            events.push_back(Event::End(TagEnd::Link));
        }
        if !plain.is_empty() {
            events.push_back(Event::Text(plain.into()));
        }
        events
    }
}

impl<'s, I: Iterator<Item = Event<'s>>> Iterator for Links<'s, '_, I> {
    type Item = Event<'s>;

    fn next(&mut self) -> Option<Event<'s>> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        let event = self.events.next()?;
        match &event {
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => self.nested += 1,
            Event::End(TagEnd::Link | TagEnd::Image) => {
                self.nested = self.nested.saturating_sub(1);
            }
            Event::Text(text) if text.chars().any(|each| stand_in(each).is_some()) => {
                self.pending = self.split(text);
                return self.pending.pop_front();
            }
            _ => {}
        }
        Some(event)
    }
}

fn addresses<'a>(
    markdown: &'a str,
    events: impl Iterator<Item = (Event<'a>, Range<usize>)>,
) -> Vec<Range<usize>> {
    let events: Vec<_> = events.collect();
    let mut stops: Vec<usize> = events
        .iter()
        .filter(|(event, _)| stops_an_address(event))
        .map(|(_, range)| range.start)
        .collect();
    stops.sort_unstable();
    let lower = markdown.to_ascii_lowercase();
    let mut scan = Scan {
        markdown,
        lower: &lower,
        stops: &stops,
        found: Vec::new(),
    };
    let mut open: Vec<(usize, bool)> = Vec::new();
    for (event, range) in events {
        match event {
            Event::Start(tag) => open.push((range.end, guards(&tag))),
            Event::End(_) => {
                open.pop();
            }
            Event::Text(_) if !open.iter().any(|&(_, guarded)| guarded) => {
                // An address ends within its element, such as a table cell or the emphasis it
                // begins in, so that standing in for it leaves the parse's structure as it was.
                let cap = open.last().map_or(markdown.len(), |&(end, _)| end);
                scan.text(range, cap);
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
    matches!(
        event,
        Event::Code(_)
            | Event::InlineHtml(_)
            | Event::Html(_)
            | Event::FootnoteReference(_)
            | Event::Start(
                Tag::Link { .. } | Tag::Image { .. } | Tag::CodeBlock(_) | Tag::HtmlBlock
            )
    )
}

struct Scan<'s> {
    markdown: &'s str,
    // ASCII lowercase keeps every byte where it was, so a place in it is a place in `markdown`.
    lower: &'s str,
    stops: &'s [usize],
    found: Vec<Range<usize>>,
}

impl Scan<'_> {
    fn text(&mut self, range: Range<usize>, cap: usize) {
        let mut from = range
            .start
            .max(self.found.last().map_or(0, |address| address.end));
        while let Some((start, scheme)) = next_scheme(&self.lower[..range.end], from) {
            let preceded = self.markdown[..start]
                .chars()
                .next_back()
                .is_some_and(|before| before.is_ascii_alphanumeric());
            let rest = &self.markdown[start..cap.min(self.next_stop(start))];
            let length = rest.find(ends_an_address).unwrap_or(rest.len());
            let end = start + trimmed(&rest[..length]);
            if !preceded && end > start + scheme {
                self.found.push(start..end);
                from = end;
            } else {
                from = start + scheme;
            }
            if from >= range.end {
                return;
            }
        }
    }

    fn next_stop(&self, start: usize) -> usize {
        let index = self.stops.partition_point(|&stop| stop <= start);
        self.stops.get(index).copied().unwrap_or(usize::MAX)
    }
}

fn ends_an_address(next: char) -> bool {
    next.is_whitespace() || next == '<'
}

fn next_scheme(lower: &str, from: usize) -> Option<(usize, usize)> {
    SCHEMES
        .iter()
        .filter_map(|scheme| {
            lower
                .get(from..)?
                .find(scheme)
                .map(|at| (from + at, scheme.len()))
        })
        .min_by_key(|&(at, _)| at)
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
mod tests {
    use super::super::{render_document, to_html};

    fn link(address: &str) -> String {
        format!("<a href=\"{address}\">{address}</a>")
    }

    fn paragraph(before: &str, address: &str, after: &str) -> String {
        format!("<p>{before}{}{after}</p>\n", link(address))
    }

    #[test]
    fn a_bare_https_address_in_a_note_is_drawn_as_a_link_to_it() {
        assert_eq!(
            to_html("See https://example.com/plan for it."),
            paragraph("See ", "https://example.com/plan", " for it.")
        );
    }

    #[test]
    fn an_underscore_an_asterisk_or_a_tilde_within_an_address_stays_in_its_link() {
        for address in [
            "https://github.com/o/r/tree/main/src/__tests__/a.ts",
            "https://example.com/a*b*c",
            "https://example.com/a~b~c",
            "https://example.com/a**b**c",
        ] {
            assert_eq!(
                to_html(&format!("See {address} now")),
                paragraph("See ", address, " now")
            );
        }
    }

    #[test]
    fn emphasis_around_an_address_parses_as_it_did_without_the_link() {
        assert_eq!(
            to_html("**see https://example.com/a**b"),
            format!(
                "<p><strong>see {}</strong>b</p>\n",
                link("https://example.com/a")
            )
        );
    }

    #[test]
    fn a_bare_http_address_and_a_bare_mailto_address_are_each_drawn_as_a_link() {
        assert_eq!(
            to_html("Read http://example.com/a?b=1#c first"),
            paragraph("Read ", "http://example.com/a?b=1#c", " first")
        );
        assert_eq!(
            to_html("Write to mailto:someone@example.com today"),
            paragraph("Write to ", "mailto:someone@example.com", " today")
        );
    }

    #[test]
    fn a_bare_address_in_every_markdown_field_is_drawn_as_a_link() {
        let mut document = serde_json::json!({"stages": [{
            "version": {"body": {"title": "Model", "goal": "Per https://example.com/goal"}},
            "blocks": [
                {"version": {"body": {"title": "Ship", "consequence": "See https://example.com/consequence"}}},
                {"version": {"body": {"title": "Fix", "detail": "See https://example.com/detail", "state": "done"}}},
                {"version": {"body": {"prompt": "Which?", "context": "See https://example.com/context",
                    "options": [{"label": "Top", "detail": "See https://example.com/option"}]}}},
                {"kind": "loft", "version": {"body": {"view": "trace", "note": "See https://example.com/note"}}},
                {"version": {"body": {"title": "Ship it?", "markdown": "See https://example.com/approval"}}},
                {"version": {"body": {"title": "The plan", "markdown": "See https://example.com/plan"}}}
            ]
        }]});
        render_document(&mut document);
        let stage = &document["stages"][0];
        let body = |index: usize| &stage["blocks"][index]["version"]["body"];
        let fields = [
            (&stage["version"]["body"]["goalHtml"], "goal"),
            (&body(0)["consequenceHtml"], "consequence"),
            (&body(1)["detailHtml"], "detail"),
            (&body(2)["contextHtml"], "context"),
            (&body(2)["options"][0]["detailHtml"], "option"),
            (&body(3)["noteHtml"], "note"),
            (&body(4)["html"], "approval"),
            (&body(5)["html"], "plan"),
        ];
        for (html, field) in fields {
            let html = html.as_str().unwrap_or_default();
            let expected = link(&format!("https://example.com/{field}"));
            assert!(html.contains(&expected), "{field}: {html}");
        }
    }

    #[test]
    fn an_address_within_a_code_span_is_drawn_as_text() {
        let html = to_html("Run `curl https://example.com/api` now");
        assert!(!html.contains("<a"), "{html}");
        assert!(
            html.contains("<code>curl https://example.com/api</code>"),
            "{html}"
        );
        let html = to_html("Run https://example.com/a`code` now");
        assert!(html.contains(&link("https://example.com/a")), "{html}");
        assert!(html.contains("<code>code</code>"), "{html}");
    }

    #[test]
    fn an_address_within_a_fenced_code_block_is_drawn_as_text() {
        let html = to_html("```\ncurl https://example.com/api\n```\n");
        assert!(!html.contains("<a"), "{html}");
        assert!(html.contains("curl https://example.com/api"), "{html}");
    }

    #[test]
    fn an_address_that_a_written_link_holds_is_not_linked_again() {
        for markdown in [
            "[https://example.com/a](https://example.com/a)",
            "<https://example.com/b>",
            "[see https://example.com/c](https://example.com/d)",
            "![https://example.com/e](https://example.com/e.png)",
        ] {
            let html = to_html(markdown);
            assert_eq!(html.matches("<a ").count(), 1, "{html}");
        }
    }

    #[test]
    fn punctuation_that_ends_an_address_is_drawn_after_the_link() {
        for mark in ['.', ',', ':', ';', '!', '?', '*', '_', '~', '\''] {
            assert_eq!(
                to_html(&format!("Go to https://example.com/x{mark}")),
                paragraph("Go to ", "https://example.com/x", &mark.to_string()),
                "{mark}"
            );
        }
        assert_eq!(
            to_html("Set it to \"https://example.com\"."),
            paragraph("Set it to \"", "https://example.com", "\".")
        );
        assert_eq!(
            to_html("See https://docs.python.org/3/x.html#object.__init__ there"),
            paragraph(
                "See ",
                "https://docs.python.org/3/x.html#object.__init",
                "__ there"
            )
        );
        assert_eq!(
            to_html("https://example.com/a.b"),
            paragraph("", "https://example.com/a.b", "")
        );
    }

    #[test]
    fn an_entity_that_ends_an_address_is_drawn_after_the_link() {
        assert_eq!(
            to_html("See https://example.com/a&amp; now"),
            paragraph("See ", "https://example.com/a", "&amp; now")
        );
    }

    #[test]
    fn a_closing_parenthesis_ends_the_link_unless_the_address_opens_one() {
        assert_eq!(
            to_html("(see https://example.com/x)"),
            paragraph("(see ", "https://example.com/x", ")")
        );
        let wiki = "https://en.wikipedia.org/wiki/Foo_(bar)";
        assert_eq!(to_html(wiki), paragraph("", wiki, ""));
        assert_eq!(to_html(&format!("({wiki}).")), paragraph("(", wiki, ")."));
    }

    #[test]
    fn a_closing_square_bracket_ends_the_link_unless_the_address_opens_one() {
        assert_eq!(
            to_html("[see https://example.com/x]"),
            paragraph("[see ", "https://example.com/x", "]")
        );
        assert_eq!(
            to_html("https://example.com/a[1]"),
            "<p><a href=\"https://example.com/a%5B1%5D\">https://example.com/a[1]</a></p>\n"
        );
    }

    #[test]
    fn no_link_is_drawn_to_an_address_of_any_other_scheme() {
        let html = to_html("javascript:alert(1) and ftp://example.com/f and data:text/html,x");
        assert!(!html.contains("<a"), "{html}");
        assert!(html.contains("javascript:alert(1)"), "{html}");
        let html = to_html("javascript:https://example.com/x");
        assert!(!html.contains("href=\"javascript"), "{html}");
    }

    #[test]
    fn a_scheme_with_nothing_after_it_stays_text() {
        assert_eq!(
            to_html("Begin with https:// or mailto: here"),
            "<p>Begin with https:// or mailto: here</p>\n"
        );
    }
}
