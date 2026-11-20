//! A bare address of the web or of mail in a block's Markdown made a link, as GitHub's extended
//! autolink makes one: `pulldown-cmark` reads `CommonMark` alone, which links only an address
//! the Markdown writes out.

use std::collections::VecDeque;

use pulldown_cmark::{CowStr, Event, LinkType, Tag, TagEnd};

/// The schemes a bare address may begin with.
const SCHEMES: [&str; 3] = ["https://", "http://", "mailto:"];

/// The marks that end a sentence rather than an address.
const TRAILING: [char; 6] = ['.', ',', ':', ';', '!', '?'];

/// The events of a parser with each bare address in their text made a link, outside code and
/// outside a link the Markdown writes out. Its text is to come merged, as `TextMergeStream`
/// gives it, so that no address is split between two events.
pub struct Autolink<'a, I: Iterator<Item = Event<'a>>> {
    events: I,
    /// The events a text was split into, still to be given.
    pending: VecDeque<Event<'a>>,
    /// Whether a code block is open.
    code: bool,
    /// How many links and images are open.
    links: usize,
}

impl<'a, I: Iterator<Item = Event<'a>>> Autolink<'a, I> {
    pub const fn new(events: I) -> Self {
        Self {
            events,
            pending: VecDeque::new(),
            code: false,
            links: 0,
        }
    }

    /// Notes the code block or the link that `event` opens or ends.
    fn track(&mut self, event: &Event<'a>) {
        match event {
            Event::Start(Tag::CodeBlock(_)) => self.code = true,
            Event::End(TagEnd::CodeBlock) => self.code = false,
            Event::Start(Tag::Link { .. } | Tag::Image { .. }) => self.links += 1,
            Event::End(TagEnd::Link | TagEnd::Image) => {
                self.links = self.links.saturating_sub(1);
            }
            _ => {}
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for Autolink<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        if let Some(event) = self.pending.pop_front() {
            return Some(event);
        }
        let event = self.events.next()?;
        self.track(&event);
        match event {
            Event::Text(text) if !self.code && self.links == 0 => {
                self.pending = linked(text);
                self.pending.pop_front()
            }
            other => Some(other),
        }
    }
}

/// The events `text` makes: its text, with each bare address in it a link.
fn linked(text: CowStr<'_>) -> VecDeque<Event<'_>> {
    let found = addresses(&text);
    if found.is_empty() {
        return VecDeque::from([Event::Text(text)]);
    }
    let mut events = VecDeque::new();
    let mut from = 0;
    for (start, end) in found {
        if start > from {
            events.push_back(Event::Text(text[from..start].to_owned().into()));
        }
        let address = &text[start..end];
        events.push_back(Event::Start(Tag::Link {
            link_type: LinkType::Autolink,
            dest_url: address.to_owned().into(),
            title: CowStr::Borrowed(""),
            id: CowStr::Borrowed(""),
        }));
        events.push_back(Event::Text(address.to_owned().into()));
        events.push_back(Event::End(TagEnd::Link));
        from = end;
    }
    if from < text.len() {
        events.push_back(Event::Text(text[from..].to_owned().into()));
    }
    events
}

/// Where each bare address in `text` begins and ends: at a scheme that no letter or digit
/// precedes, up to the first space or `<`, less the marks that end it, with something left after
/// its scheme.
fn addresses(text: &str) -> Vec<(usize, usize)> {
    // ASCII lowercase keeps every byte where it was.
    let lower = text.to_ascii_lowercase();
    let mut found = Vec::new();
    let mut from = 0;
    while let Some((start, scheme)) = next_scheme(&lower, from) {
        let preceded = text[..start]
            .chars()
            .next_back()
            .is_some_and(|before| before.is_ascii_alphanumeric());
        let rest = &text[start..];
        let length = rest
            .find(|next: char| next.is_whitespace() || next == '<')
            .unwrap_or(rest.len());
        let end = start + trimmed(&rest[..length]);
        if !preceded && end > start + scheme {
            found.push((start, end));
            from = end;
        } else {
            from = start + scheme;
        }
    }
    found
}

/// Where the first scheme at or after `from` in `lower` begins, and its length.
fn next_scheme(lower: &str, from: usize) -> Option<(usize, usize)> {
    SCHEMES
        .iter()
        .filter_map(|scheme| {
            lower[from..]
                .find(scheme)
                .map(|at| (from + at, scheme.len()))
        })
        .min_by_key(|&(at, _)| at)
}

/// The length of `address` less the marks that end it: a mark that ends a sentence, and a
/// closing parenthesis that the address opens none for.
fn trimmed(address: &str) -> usize {
    let mut end = address.len();
    loop {
        let kept = &address[..end];
        match kept.chars().next_back() {
            Some(mark) if TRAILING.contains(&mark) => end -= 1,
            Some(')') if kept.matches(')').count() > kept.matches('(').count() => end -= 1,
            _ => return end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::{render_document, to_html, to_inline_html};

    /// The link `to_html` draws for `address`.
    fn link(address: &str) -> String {
        format!("<a href=\"{address}\">{address}</a>")
    }

    #[test]
    fn a_bare_https_address_in_a_note_is_drawn_as_a_link_to_it() {
        assert_eq!(
            to_html("See https://example.com/plan for it."),
            format!("<p>See {} for it.</p>\n", link("https://example.com/plan"))
        );
    }

    #[test]
    fn a_bare_http_address_and_a_bare_mailto_address_are_each_drawn_as_a_link() {
        assert_eq!(
            to_html("Read http://example.com/a?b=1#c first"),
            format!("<p>Read {} first</p>\n", link("http://example.com/a?b=1#c"))
        );
        assert_eq!(
            to_html("Write to mailto:someone@example.com today"),
            format!(
                "<p>Write to {} today</p>\n",
                link("mailto:someone@example.com")
            )
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
                    "options": [{"label": "Top", "detail": "See https://example.com/option"}]}}}
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
        ] {
            let html = to_html(markdown);
            assert_eq!(html.matches("<a ").count(), 1, "{html}");
        }
    }

    #[test]
    fn punctuation_that_ends_an_address_is_drawn_after_the_link() {
        for mark in ['.', ',', ':', ';', '!', '?'] {
            assert_eq!(
                to_html(&format!("Go to https://example.com/x{mark}")),
                format!("<p>Go to {}{mark}</p>\n", link("https://example.com/x"))
            );
        }
        assert_eq!(
            to_html("https://example.com/a.b"),
            format!("<p>{}</p>\n", link("https://example.com/a.b"))
        );
    }

    #[test]
    fn a_closing_parenthesis_ends_the_link_unless_the_address_opens_one() {
        assert_eq!(
            to_html("(see https://example.com/x)"),
            format!("<p>(see {})</p>\n", link("https://example.com/x"))
        );
        let wiki = "https://en.wikipedia.org/wiki/Foo_(bar)";
        assert_eq!(to_html(wiki), format!("<p>{}</p>\n", link(wiki)));
        assert_eq!(
            to_html(&format!("({wiki}).")),
            format!("<p>({}).</p>\n", link(wiki))
        );
    }

    #[test]
    fn an_address_of_any_other_scheme_is_drawn_as_text() {
        let html = to_html("javascript:alert(1) and ftp://example.com/f and data:text/html,x");
        assert!(!html.contains("<a"), "{html}");
        assert!(html.contains("javascript:alert(1)"), "{html}");
    }

    #[test]
    fn a_scheme_with_nothing_after_it_stays_text() {
        assert_eq!(
            to_html("Begin with https:// or mailto: here"),
            "<p>Begin with https:// or mailto: here</p>\n"
        );
    }

    #[test]
    fn a_bare_address_in_a_name_stays_text() {
        assert_eq!(
            to_inline_html("See https://example.com/x").as_deref(),
            Some("See https://example.com/x")
        );
    }
}
