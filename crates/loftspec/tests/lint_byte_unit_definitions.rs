//! `V2-014` (F-21): what defines a decimal byte unit, and so settles the document, and what does
//! not; and the numbers that begin no quantity.

use crate::common::decisions::decision;
use crate::common::{section, term};
use crate::lint_byte_units::{FEATURE, beside_binary, byte_units, named, one};
use loftspec::document::Document;

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
        "Sizes are in MB (10^6 bytes).",
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
        "files/512MB.bin",
        "image.512MB",
        "size-512MB",
        "size_512MB",
    ] {
        let document = beside_binary(&format!("Download {written} first."));
        assert!(byte_units(&document).is_empty(), "{written}");
    }
}
