//! The Markdown constructs of a body outside the Format's subset (`V2-009`). The subset is
//! paragraphs, bullet and numbered lists, code spans, fenced code blocks, emphasis (strong
//! included) and links, with the line breaks between them; a heading is `V2-001`'s. The body is
//! read as `CommonMark` with the GitHub extensions a renderer shows as something other than
//! text: tables, footnotes, strikethrough and task lists. Extensions that would read ordinary
//! prose as a construct, such as `$` as math or `^` as superscript, are left off, so that text
//! stays text.

use pulldown_cmark::{CodeBlockKind, Event, Options, Parser, Tag};

/// The constructs of `body` outside the subset, in order, each as the body line it starts on,
/// counted from 1, and what it is. A construct inside another counts on its own; text in a fenced
/// code block or a code span is code, and yields nothing.
pub(super) fn outside_subset(body: &str) -> Vec<(usize, &'static str)> {
    let line_starts: Vec<usize> = std::iter::once(0)
        .chain(body.match_indices('\n').map(|(at, _)| at + 1))
        .collect();
    let line_of = |offset: usize| line_starts.partition_point(|start| *start <= offset);
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_FOOTNOTES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS;
    Parser::new_ext(body, options)
        .into_offset_iter()
        .filter_map(|(event, range)| construct(&event).map(|what| (line_of(range.start), what)))
        .collect()
}

/// What an event starts, where it starts a construct outside the subset.
fn construct(event: &Event<'_>) -> Option<&'static str> {
    Some(match event {
        Event::Start(tag) => match tag {
            Tag::HtmlBlock => "an HTML block",
            Tag::BlockQuote(_) => "a block quote",
            Tag::CodeBlock(CodeBlockKind::Indented) => "an indented code block",
            Tag::FootnoteDefinition(_) => "a footnote",
            Tag::Table(_) => "a table",
            Tag::Strikethrough => "struck-through text",
            Tag::Image { .. } => "an image",
            _ => return None,
        },
        Event::InlineHtml(_) => "inline HTML",
        Event::FootnoteReference(_) => "a footnote reference",
        Event::Rule => "a thematic break",
        Event::TaskListMarker(_) => "a task list item",
        _ => return None,
    })
}
