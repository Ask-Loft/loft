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
fn a_bare_http_address_is_drawn_as_a_link() {
    assert_eq!(
        to_html("Read http://example.com/a?b=1#c first"),
        paragraph("Read ", "http://example.com/a?b=1#c", " first")
    );
}

#[test]
fn a_bare_mailto_address_is_drawn_as_a_link() {
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
        paragraph("Set it to &quot;", "https://example.com", "&quot;.")
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
fn an_address_after_a_square_bracket_that_opens_no_link_is_drawn_as_text() {
    for markdown in ["[see https://example.com/x]", "[see https://example.com/x"] {
        assert_eq!(to_html(markdown), format!("<p>{markdown}</p>\n"));
    }
    assert_eq!(
        to_html("[a] https://example.com/x"),
        paragraph("[a] ", "https://example.com/x", "")
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
fn an_address_that_begins_with_www_is_drawn_as_text() {
    assert_eq!(
        to_html("See www.example.com/a now"),
        "<p>See www.example.com/a now</p>\n"
    );
    assert_eq!(
        to_html("[www.example.com](http://www.example.com)"),
        "<p><a href=\"http://www.example.com\">www.example.com</a></p>\n"
    );
}

#[test]
fn an_email_address_that_no_mailto_begins_is_drawn_as_text() {
    assert_eq!(
        to_html("Write to someone@example.com today"),
        "<p>Write to someone@example.com today</p>\n"
    );
    assert_eq!(
        to_html("<someone@example.com>"),
        "<p><a href=\"mailto:someone@example.com\">someone@example.com</a></p>\n"
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

#[test]
fn a_link_written_to_an_address_that_begins_with_two_slashes_is_drawn_as_its_text() {
    for markdown in ["[docs](//example.com/x)", "[docs]( //example.com/x )"] {
        assert_eq!(to_html(markdown), "<p>docs</p>\n", "{markdown}");
    }
}

#[test]
fn a_link_written_to_a_file_s_address_is_drawn_as_its_text() {
    for markdown in [
        "[plan](docs/plan.md)",
        "[plan](/docs/plan.md)",
        "[plan](./plan.md?x#y)",
    ] {
        assert_eq!(to_html(markdown), "<p>plan</p>\n", "{markdown}");
    }
}

#[test]
fn a_link_written_to_a_place_within_the_page_is_drawn_as_a_link_to_it() {
    assert_eq!(
        to_html("[notes](#notes)"),
        "<p><a href=\"#notes\">notes</a></p>\n"
    );
}

#[test]
fn an_image_whose_address_is_a_javascript_one_is_drawn_as_its_description() {
    assert_eq!(
        to_html("![a shelf](javascript:alert(1))"),
        "<p>a shelf</p>\n"
    );
}

#[test]
fn an_image_within_a_link_is_drawn_as_its_description_within_that_link() {
    assert_eq!(
        to_html("[![a badge](https://example.com/b.png)](https://example.com/c)"),
        "<p><a href=\"https://example.com/c\">a badge</a></p>\n"
    );
}

#[test]
fn a_link_within_an_image_s_description_is_drawn_as_its_text_within_the_image_s_link() {
    assert_eq!(
        to_html("![see [docs](https://example.com/a)](https://example.com/b.png)"),
        "<p><a href=\"https://example.com/b.png\">see docs</a></p>\n"
    );
}

#[test]
fn a_footnote_s_reference_within_a_written_link_is_drawn_right_after_that_link() {
    let html = to_html("[see[^1]](https://example.com/x) now\n\n[^1]: Its source.");
    assert!(
        html.contains(
            "<a href=\"https://example.com/x\">see</a><sup class=\"footnote-ref\"><a href=\"#fn-1\""
        ),
        "{html}"
    );
    assert_eq!(
        html.matches("</a>").count(),
        html.matches("<a ").count(),
        "{html}"
    );
}
