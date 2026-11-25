//! `V2-014` (F-21): what defines a decimal byte unit, and so settles the document, and what does
//! not; and the numbers that begin no quantity.

use crate::common::decisions::decision;
use crate::common::{key, section, term};
use crate::lint_byte_units::{FEATURE, beside_binary, byte_units, named, one, with_bodies};
use loftspec::document::Document;
use loftspec::lint::Finding;

/// The document stating `5 MB` beside `4 KiB`, with `text` once in a Component's body, once in a
/// decision's context, once in a glossary Section and once in a term's definition.
fn with_text(text: &str) -> [Document; 4] {
    let document = beside_binary("A page holds 5 MB.");
    let mut node = document.clone();
    node.components[0].component.details.body = Some(text.to_owned());
    let mut decided = document.clone();
    text.clone_into(&mut decision(&mut decided).context);
    let mut glossary = document.clone();
    glossary.products[0].details.sections = vec![section(66, "Glossary", text)];
    let mut termed = document;
    let mut entry = term(65, "page size", &[]);
    text.clone_into(&mut entry.definition);
    termed.terms.push(entry);
    [node, decided, glossary, termed]
}

#[test]
fn a_full_name_in_parentheses_is_no_count_of_bytes() {
    for text in [
        "Sizes are in kB (kilobytes).",
        "kB (kilobytes) is the unit of size.",
        "A cache holds 512 MB (megabytes).",
    ] {
        for document in with_text(text) {
            assert!(named(&document).contains(&one(FEATURE, "MB")[0]), "{text}");
        }
    }
}

#[test]
fn an_ordinary_sentence_with_a_quantity_and_a_count_of_bytes_defines_no_unit() {
    for text in [
        "The index overhead per 1 MB is 64 bytes.",
        "A 64 MB cache (4096 bytes per entry)",
        "Here 1 MB = 1,000,000 bytes.",
    ] {
        for document in with_text(text) {
            assert!(named(&document).contains(&one(FEATURE, "MB")[0]), "{text}");
        }
    }
}

#[test]
fn a_unit_that_opens_a_clause_alone_or_after_one_or_an_article_defines_it() {
    for text in [
        "1 MB = 1,000,000 bytes.",
        "MB (10^6 bytes) is the unit of size.",
        "Sizes: 1 MB = 1,000,000 bytes.",
        "For sizes, MB means 10^6 bytes.",
        "Pages are small; one MB is 1,000,000 bytes.",
        "Sizes vary. The megabyte is 1,000,000 bytes.",
        "A cache (1 MB = 10^6 bytes) is kept.",
        "Units\n\n- An MB is 1,000,000 bytes.",
    ] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}

#[test]
fn a_term_whose_alias_is_a_decimal_unit_settles_the_document() {
    for alias in ["MB", "megabytes"] {
        let mut document = beside_binary("A page holds 5 MB.");
        let mut entry = term(65, "Decimal megabyte", &[alias]);
        entry.definition = "1,000,000 bytes.".to_owned();
        document.terms.push(entry);
        assert!(byte_units(&document).is_empty(), "{alias}");
    }
}

#[test]
fn only_a_lowercase_s_makes_a_term_s_name_plural() {
    for name in ["KBS", "TBS", "MBS"] {
        let mut document = beside_binary("A page holds 5 MB.");
        document.terms.push(term(65, name, &[]));
        assert_eq!(named(&document), one(FEATURE, "MB"), "{name}");
    }
}

#[test]
fn a_unit_followed_by_a_colon_and_a_count_of_bytes_defines_it() {
    for text in ["MB: 1,000,000 bytes", "A megabyte: 1,000,000 bytes."] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}

#[test]
fn a_number_inside_a_web_address_or_after_a_path_mark_begins_no_quantity() {
    for written in [
        "https://example.com/files/512MB.bin",
        "https://example.com/?size=512MB",
        "example.com/get?size=512MB",
        "files#512MB",
        "a=1&512MB",
        "files/512MB.bin",
        "image.512MB",
        "size-512MB",
        "size_512MB",
    ] {
        let document = beside_binary(&format!("Download {written} first."));
        assert!(byte_units(&document).is_empty(), "{written}");
    }
}

#[test]
fn a_parenthesis_right_after_the_unit_needs_no_clause_opening() {
    for text in [
        "Sizes are in MB (10^6 bytes).",
        "Pages hold MB (1,048,576 bytes).",
    ] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}

#[test]
fn a_count_defines_a_unit_only_where_it_equals_the_unit_s_value() {
    for text in [
        "After compression, 1 MB is 400,000 bytes on disk.",
        "Limits: 1 MB (512 bytes per sector)",
        "e.g. 1 MB is 64 bytes of index.",
        "1 KB = 1,000,000 bytes.",
        "1 MB = 1,000 bytes.",
    ] {
        for document in with_text(text) {
            assert!(named(&document).contains(&one(FEATURE, "MB")[0]), "{text}");
        }
    }
    for text in [
        "1 MB = 1,048,576 bytes.",
        "MB: 2^20 bytes",
        "A KB is 1,024 bytes.",
        "1 GB = 10^9 bytes.",
        "1 TB = 1000^4 bytes.",
        "A kilobyte is 1,024 bytes.",
    ] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}

#[test]
fn a_count_defines_a_unit_only_where_it_closes_its_clause() {
    for text in [
        "1 MB = 1,000,000 bytes of index.",
        "MB (10^6 bytes per entry) is used.",
        "1 MB = 1,000,000 bytes on disk",
    ] {
        for document in with_text(text) {
            assert!(named(&document).contains(&one(FEATURE, "MB")[0]), "{text}");
        }
    }
    for text in [
        "1 MB = 1,000,000 bytes",
        "1 MB = 1,000,000 bytes, as everywhere.",
        "1 MB = 1,000,000 bytes; KB likewise.",
        "1 MB = 1,000,000 bytes\n\nSizes follow.",
    ] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}

#[test]
fn a_term_named_after_a_unit_settles_only_where_its_definition_names_or_counts_it() {
    for definition in [
        "The knowledge base the support agent searches.",
        "A megabyte.",
        "512 bytes.",
    ] {
        let mut document = beside_binary("A page holds 5 MB.");
        let mut entry = term(65, "KB", &[]);
        entry.definition = definition.to_owned();
        document.terms.push(entry);
        assert_eq!(named(&document), one(FEATURE, "MB"), "{definition}");
    }
    for definition in ["A kilobyte: 1,000 bytes.", "1,024 bytes.", "Kilobytes."] {
        let mut document = beside_binary("A page holds 5 MB.");
        let mut entry = term(65, "KB", &[]);
        entry.definition = definition.to_owned();
        document.terms.push(entry);
        assert!(byte_units(&document).is_empty(), "{definition}");
    }
}

#[test]
fn a_term_s_definition_is_read_for_quantities() {
    let mut document = with_bodies("A page holds 5 MB.", "Nothing measured.");
    let mut entry = term(65, "block", &[]);
    entry.definition = "A block holds 4 KiB.".to_owned();
    document.terms.push(entry);
    let found = byte_units(&document);
    assert_eq!(named(&document), one(FEATURE, "MB"));
    assert!(
        found[0].message.contains(&key(65).to_string()),
        "{}",
        found[0].message
    );
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    let mut entry = term(65, "page", &[]);
    entry.definition = "A page holds 5 MB.".to_owned();
    document.terms.push(entry);
    assert_eq!(named(&document), one(65, "MB"));
}

#[test]
fn each_list_item_is_read_apart_from_the_text_before_it() {
    let document = beside_binary("- Pages\n  - 512 MB each");
    assert_eq!(named(&document), one(FEATURE, "MB"));
    let document = beside_binary("- Pages: 2\n  - MB is the unit");
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
    let mut document = beside_binary("A page holds 5 MB.");
    document.components[0].component.details.body =
        Some("- Units\n  - MB: 1,000,000 bytes".to_owned());
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
    let document = beside_binary("Pages\n> 512 MB each");
    assert_eq!(named(&document), one(FEATURE, "MB"));
}

#[test]
fn a_narrow_no_break_space_before_a_defined_unit_keeps_its_clause_open() {
    for text in [
        "Sizes:\u{202f}1\u{202f}MB = 1,000,000 bytes.",
        "Sizes:\u{202f}MB = 10^6 bytes.",
        "Sizes: one\u{202f}MB is 10^6 bytes.",
    ] {
        for document in with_text(text) {
            assert!(byte_units(&document).is_empty(), "{text}");
        }
    }
}
