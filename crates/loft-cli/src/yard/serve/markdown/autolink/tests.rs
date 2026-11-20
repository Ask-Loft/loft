use super::super::to_html;

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
        "https://example.com/src/**/*.rs",
    ] {
        assert_eq!(
            to_html(&format!("See {address} now")),
            paragraph("See ", address, " now")
        );
    }
    assert_eq!(
        to_html("**See https://example.com/src/**/*.rs**"),
        format!(
            "<p><strong>See {}</strong></p>\n",
            link("https://example.com/src/**/*.rs")
        )
    );
}

#[test]
fn an_address_runs_past_the_end_of_an_emphasis_it_begins_in_as_on_github() {
    assert_eq!(
        to_html("**see https://example.com/a**b"),
        paragraph("**see ", "https://example.com/a**b", "")
    );
}

#[test]
fn an_address_whose_stand_in_would_change_the_parse_is_drawn_as_text() {
    for markdown in [
        "Use <img src=https://example.com/a?b=c width=1> here",
        "[a](https://example.com/a( )",
        "```https://example.com/a`b\nmore\n",
        "a | https://example.com/b|c\n-|-\n",
    ] {
        let html = to_html(markdown);
        assert!(!html.contains("<a "), "{markdown}: {html}");
        assert!(html.contains("https://example.com/"), "{markdown}: {html}");
        assert!(!html.contains('\u{10_0000}'), "{markdown}: {html}");
    }
}

#[test]
fn an_address_whose_stand_in_would_change_the_parse_leaves_the_others_linked() {
    let html = to_html(
        "See https://example.com/a now.\n\na | https://example.com/b|c\n-|-\n\nAnd https://example.com/d now.",
    );
    assert!(html.contains(&link("https://example.com/a")), "{html}");
    assert!(html.contains(&link("https://example.com/d")), "{html}");
    assert!(html.contains("a | https://example.com/b|c"), "{html}");
    assert_eq!(html.matches("<a ").count(), 2, "{html}");
}

#[test]
fn a_markdown_that_holds_a_stand_in_is_drawn_without_links() {
    for markdown in [
        "\u{10_0000} https://example.com/a",
        "See https://example.com/a and &#x100000;",
        "[foo]: https://example.com/a(b\nbar \u{10_0000}",
        "[foo]: https://example.com/a(b\nbar &#x100000;",
    ] {
        let html = to_html(markdown);
        assert!(!html.contains("<a "), "{markdown}: {html}");
        assert!(html.contains("https://example.com/a"), "{markdown}: {html}");
    }
}

#[test]
fn an_escaped_pipe_in_a_table_cell_is_part_of_the_address() {
    let html = to_html("| h |\n|---|\n| https://example.com/a\\|b |");
    assert!(
        html.contains("<a href=\"https://example.com/a%7Cb\">https://example.com/a|b</a>"),
        "{html}"
    );
}

#[test]
fn an_address_before_a_line_break_is_drawn_as_a_link() {
    assert_eq!(
        to_html("line https://example.com/a\\\nnext https://example.com/b  \nthird"),
        format!(
            "<p>line {}\nnext {}<br />\nthird</p>\n",
            "<a href=\"https://example.com/a%5C\">https://example.com/a\\</a>",
            link("https://example.com/b")
        )
    );
}

#[test]
fn a_scheme_that_a_letter_precedes_begins_no_address() {
    assert_eq!(
        to_html("xhttps://example.com and 9https://example.com"),
        paragraph("xhttps://example.com and 9", "https://example.com", "")
    );
}

#[test]
fn an_address_ends_at_a_space_a_tab_or_a_line_end_alone() {
    let html = to_html("See https://example.com/a\u{a0}b\u{2003}c\td now");
    assert!(
        html.contains(">https://example.com/a\u{a0}b\u{2003}c</a>\td now"),
        "{html}"
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

#[test]
fn an_http_scheme_that_punctuation_follows_is_drawn_as_text() {
    for markdown in ["https:///etc", "See https://-x now", "http://.example.com"] {
        let html = to_html(markdown);
        assert!(!html.contains("<a "), "{markdown}: {html}");
    }
}

#[test]
fn a_host_name_with_an_underscore_in_either_of_its_last_two_parts_is_drawn_as_text() {
    for markdown in ["https://a.b_c.com/x", "https://a.b.c_d/x"] {
        let html = to_html(markdown);
        assert!(!html.contains("<a "), "{markdown}: {html}");
    }
    for address in [
        "https://a_b.c.com/x",
        "https://example.com/a_b",
        "https://a.b.c.d.e.f.g.h.i.j.k_l.m",
    ] {
        assert_eq!(to_html(address), paragraph("", address, ""));
    }
}

#[test]
fn a_mailto_that_no_email_address_follows_is_drawn_as_text() {
    for markdown in [
        "mailto:someone",
        "mailto:a@b",
        "mailto:a@b.c1",
        "mailto:a b@c.com",
        "MAILTO:a@b.com",
        "9mailto:a@b.com",
    ] {
        let html = to_html(markdown);
        assert!(!html.contains("<a "), "{markdown}: {html}");
    }
}

#[test]
fn the_link_of_a_mailto_address_ends_where_its_email_address_ends() {
    assert_eq!(
        to_html("Write mailto:a.b+c@d-e.example.com?subject=x now"),
        paragraph("Write ", "mailto:a.b+c@d-e.example.com", "?subject=x now")
    );
    assert_eq!(
        to_html("Write mailto:a@b.com."),
        paragraph("Write ", "mailto:a@b.com", ".")
    );
}
