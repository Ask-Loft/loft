//! The Markdown of Yard Log's blocks as the page shows it: rendered to HTML here, by `loft`, so
//! that the page never renders Markdown itself. Any HTML the Markdown holds is shown as text,
//! a link keeps only an address of the web, of mail or within the page, and an image is shown
//! as a link to it, so that no element a block names is made in the page and nothing is loaded
//! from elsewhere. A fenced code block marked `mermaid` stays a code block of that language,
//! which the page draws as a diagram. The names of the nodes a `loft` view reports are rendered
//! the same way, each on one line.

use pulldown_cmark::{Event, Options, Parser, Tag, TagEnd, html};
use serde_json::Value;

/// The Markdown fields of a stage's or a block's body, each with the field its HTML is given in:
/// a note's, an approval's and a plan's Markdown, a question's context, a stage's goal, a
/// decision's consequence, a task's detail and a `loft` block's note.
const FIELDS: [(&str, &str); 6] = [
    ("markdown", "html"),
    ("context", "contextHtml"),
    ("goal", "goalHtml"),
    ("consequence", "consequenceHtml"),
    ("detail", "detailHtml"),
    ("note", "noteHtml"),
];

/// `markdown` as HTML.
pub fn to_html(markdown: &str) -> String {
    let options = Options::ENABLE_TABLES
        | Options::ENABLE_STRIKETHROUGH
        | Options::ENABLE_TASKLISTS
        | Options::ENABLE_FOOTNOTES;
    let mut out = String::new();
    html::push_html(&mut out, Safe::new(Parser::new_ext(markdown, options)));
    out
}

/// `markdown` as HTML for one line, such as a node's name: the inside of the one paragraph it
/// makes, or none where it makes anything else.
pub fn to_inline_html(markdown: &str) -> Option<String> {
    let html = to_html(markdown);
    let inner = html.strip_prefix("<p>")?.strip_suffix("</p>\n")?;
    (!inner.contains("</p>")).then(|| inner.to_owned())
}

/// Gives each Markdown field of every version of every block and stage in a read document its
/// HTML beside it.
pub fn render_document(document: &mut Value) {
    let Some(stages) = document.get_mut("stages").and_then(Value::as_array_mut) else {
        return;
    };
    for stage in stages {
        render_record(stage);
        if let Some(blocks) = stage.get_mut("blocks").and_then(Value::as_array_mut) {
            blocks.iter_mut().for_each(render_record);
        }
    }
}

/// Renders the bodies of a record's current version and of each of its versions.
fn render_record(record: &mut Value) {
    if let Some(body) = record.pointer_mut("/version/body") {
        render_body(body);
    }
    if let Some(versions) = record.get_mut("versions").and_then(Value::as_array_mut) {
        for version in versions {
            if let Some(body) = version.get_mut("body") {
                render_body(body);
            }
        }
    }
}

/// Renders each Markdown field of a body, each option's detail of a question and the names of a
/// `loft` report.
fn render_body(body: &mut Value) {
    render_fields(body);
    let Some(fields) = body.as_object_mut() else {
        return;
    };
    if let Some(options) = fields.get_mut("options").and_then(Value::as_array_mut) {
        options.iter_mut().for_each(render_fields);
    }
    if let Some(report) = fields.get_mut("report") {
        render_names(report);
    }
}

/// Gives each Markdown field of `value` its HTML beside it.
fn render_fields(value: &mut Value) {
    let Some(fields) = value.as_object_mut() else {
        return;
    };
    for (markdown, html) in FIELDS {
        if let Some(text) = fields.get(markdown).and_then(Value::as_str) {
            let rendered = to_html(text);
            fields.insert(html.to_owned(), Value::String(rendered));
        }
    }
}

/// The one-line fields of a `loft` report, each with the field its HTML is given in: a node's
/// name and the slots of a Requirement's statement.
const LINES: [(&str, &str); 4] = [
    ("name", "nameHtml"),
    ("condition", "conditionHtml"),
    ("system", "systemHtml"),
    ("shall", "shallHtml"),
];

/// Gives each node a `loft` report names, at any depth, the HTML of its name beside it, and each
/// statement the HTML of its slots, where each is one line of Markdown.
fn render_names(value: &mut Value) {
    match value {
        Value::Object(fields) => {
            for (line, html) in LINES {
                let rendered = fields
                    .get(line)
                    .and_then(Value::as_str)
                    .and_then(to_inline_html);
                if let Some(rendered) = rendered {
                    fields.insert(html.to_owned(), Value::String(rendered));
                }
            }
            fields.values_mut().for_each(render_names);
        }
        Value::Array(items) => items.iter_mut().for_each(render_names),
        _ => {}
    }
}

/// Whether a link may keep `address`: one of the web or of mail, one within the page, or one
/// relative to it, never a script's or another scheme's.
fn is_safe(address: &str) -> bool {
    let lower = address.trim().to_ascii_lowercase();
    if ["http://", "https://", "mailto:"]
        .iter()
        .any(|scheme| lower.starts_with(scheme))
    {
        return true;
    }
    // An address with no scheme: nothing before the first `/`, `?` or `#` holds a `:`.
    let head = lower.split(['/', '?', '#']).next().unwrap_or_default();
    !head.contains(':')
}

/// The events of a parser with raw HTML turned to text, unsafe links undone and images turned
/// to links.
struct Safe<'a, I: Iterator<Item = Event<'a>>> {
    events: I,
    /// For each link open, whether it was kept.
    links: Vec<bool>,
}

impl<'a, I: Iterator<Item = Event<'a>>> Safe<'a, I> {
    const fn new(events: I) -> Self {
        Self {
            events,
            links: Vec::new(),
        }
    }

    /// The event `event` becomes, or none where it is dropped: a link or an image whose address
    /// is not kept is dropped with its end, its text staying.
    fn turn(&mut self, event: Event<'a>) -> Option<Event<'a>> {
        match event {
            Event::Html(text) | Event::InlineHtml(text) => Some(Event::Text(text)),
            Event::Start(
                Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }
                | Tag::Image {
                    link_type,
                    dest_url,
                    title,
                    id,
                },
            ) => {
                let kept = is_safe(&dest_url);
                self.links.push(kept);
                kept.then_some(Event::Start(Tag::Link {
                    link_type,
                    dest_url,
                    title,
                    id,
                }))
            }
            Event::End(TagEnd::Link | TagEnd::Image) => {
                (self.links.pop() == Some(true)).then_some(Event::End(TagEnd::Link))
            }
            other => Some(other),
        }
    }
}

impl<'a, I: Iterator<Item = Event<'a>>> Iterator for Safe<'a, I> {
    type Item = Event<'a>;

    fn next(&mut self) -> Option<Event<'a>> {
        while let Some(event) = self.events.next() {
            if let Some(turned) = self.turn(event) {
                return Some(turned);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_is_rendered_to_html() {
        assert_eq!(
            to_html("**two** kinds"),
            "<p><strong>two</strong> kinds</p>\n"
        );
    }

    #[test]
    fn html_a_block_holds_is_shown_as_text() {
        let html = to_html("<script>alert(1)</script>\n\nand <b>bold</b> inline");
        assert!(!html.contains("<script"), "{html}");
        assert!(!html.contains("<b>"), "{html}");
        assert!(html.contains("&lt;script&gt;"), "{html}");
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"), "{html}");
    }

    #[test]
    fn a_link_to_a_script_keeps_only_its_text() {
        let html = to_html("[click](javascript:alert(1)) and [site](https://example.com)");
        assert!(!html.contains("javascript"), "{html}");
        assert!(html.contains("click"), "{html}");
        assert!(
            html.contains("<a href=\"https://example.com\">site</a>"),
            "{html}"
        );
    }

    #[test]
    fn an_image_is_shown_as_a_link_to_it() {
        let html = to_html("![a shelf](https://example.com/shelf.png)");
        assert!(!html.contains("<img"), "{html}");
        assert!(html.contains("<a href=\"https://example.com/shelf.png\">a shelf</a>"));
    }

    #[test]
    fn a_code_span_in_a_name_is_rendered_as_code() {
        assert_eq!(
            to_inline_html("Written to `<ours>`.").as_deref(),
            Some("Written to <code>&lt;ours&gt;</code>.")
        );
    }

    #[test]
    fn html_a_name_holds_is_shown_as_text() {
        let html = to_inline_html("A <b>bold</b> name").unwrap_or_default();
        assert!(!html.contains("<b>"), "{html}");
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"), "{html}");
    }

    #[test]
    fn a_name_of_more_than_one_line_is_left_as_its_text() {
        assert_eq!(to_inline_html("- one\n- two"), None);
        assert_eq!(to_inline_html("One.\n\nTwo."), None);
        assert_eq!(to_inline_html("# A heading"), None);
    }

    #[test]
    fn each_name_in_a_loft_report_gets_its_html() {
        let mut document = serde_json::json!({"stages": [{"blocks": [{"version": {"body": {
            "kind": "loft",
            "report": {"features": [{"name": "`loft merge-driver`",
                "criteria": [{"name": "It exits `0`."}, {"name": "- one\n- two"}]}]}
        }}}]}]});
        render_document(&mut document);
        let feature =
            &document["stages"][0]["blocks"][0]["version"]["body"]["report"]["features"][0];
        assert_eq!(feature["nameHtml"], "<code>loft merge-driver</code>");
        assert_eq!(
            feature["criteria"][0]["nameHtml"],
            "It exits <code>0</code>."
        );
        assert!(feature["criteria"][1].get("nameHtml").is_none());
        assert_eq!(feature["name"], "`loft merge-driver`");
    }

    #[test]
    fn each_slot_of_a_statement_is_rendered_as_a_name_is() {
        let mut document = serde_json::json!({"stages": [{"blocks": [{"version": {"body": {
            "report": {"features": [{"requirements": [{"statement": {"form": "event_driven",
                "condition": "`loft yard write` runs", "system": "the `loft` command",
                "shall": "write `<ours>`."}}]}]}
        }}}]}]});
        render_document(&mut document);
        let statement = &document["stages"][0]["blocks"][0]["version"]["body"]["report"]["features"]
            [0]["requirements"][0]["statement"];
        assert_eq!(
            statement["conditionHtml"],
            "<code>loft yard write</code> runs"
        );
        assert_eq!(statement["systemHtml"], "the <code>loft</code> command");
        assert_eq!(statement["shallHtml"], "write <code>&lt;ours&gt;</code>.");
        assert_eq!(statement["form"], "event_driven");
        assert!(statement.get("formHtml").is_none());
    }

    #[test]
    fn every_markdown_field_of_a_stage_and_its_blocks_gets_its_html() {
        let mut document = serde_json::json!({"stages": [{
            "version": {"body": {"title": "Model", "goal": "The *store* reads"}},
            "blocks": [
                {"version": {"body": {"title": "Ship", "consequence": "Ships `b84b656`"}}},
                {"version": {"body": {"title": "Fix", "detail": "Also `5ddbbfe`", "state": "done"}}},
                {"version": {"body": {"view": "trace", "note": "Shows *what changed*"}}},
                {"version": {"body": {"prompt": "Which?", "options": [
                    {"label": "Top", "detail": "The **top** shelf"},
                    {"label": "Bottom"}
                ]}}}
            ]
        }]});
        render_document(&mut document);
        let stage = &document["stages"][0];
        let body = |index: usize| &stage["blocks"][index]["version"]["body"];
        assert_eq!(
            stage["version"]["body"]["goalHtml"],
            "<p>The <em>store</em> reads</p>\n"
        );
        assert_eq!(
            body(0)["consequenceHtml"],
            "<p>Ships <code>b84b656</code></p>\n"
        );
        assert_eq!(body(1)["detailHtml"], "<p>Also <code>5ddbbfe</code></p>\n");
        assert_eq!(body(2)["noteHtml"], "<p>Shows <em>what changed</em></p>\n");
        assert_eq!(
            body(3)["options"][0]["detailHtml"],
            "<p>The <strong>top</strong> shelf</p>\n"
        );
        assert!(body(3)["options"][1].get("detailHtml").is_none());
        assert_eq!(body(1)["detail"], "Also `5ddbbfe`");
    }

    #[test]
    fn html_in_a_task_s_detail_is_shown_as_text() {
        let mut document = serde_json::json!({"stages": [{"blocks": [
            {"version": {"body": {"title": "Fix", "detail": "a <b>bold</b> step", "state": "done"}}}
        ]}]});
        render_document(&mut document);
        let html = document["stages"][0]["blocks"][0]["version"]["body"]["detailHtml"]
            .as_str()
            .unwrap_or_default()
            .to_owned();
        assert!(!html.contains("<b>"), "{html}");
        assert!(html.contains("&lt;b&gt;bold&lt;/b&gt;"), "{html}");
    }

    #[test]
    fn a_mermaid_block_stays_a_code_block_of_its_language() {
        let html = to_html("```mermaid\ngraph LR; A-->B\n```\n");
        assert!(html.contains("<code class=\"language-mermaid\">"), "{html}");
    }
}
