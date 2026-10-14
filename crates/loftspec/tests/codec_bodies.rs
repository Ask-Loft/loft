//! Bodies and Sections through a write and a read of the whole tree: body text comes back byte
//! for byte, and child Sections come back in the order they were written.

use crate::common::fields::scalar;
use crate::common::text::PRODUCT;
use crate::common::{key, section, small_document};
use loftspec::codec;
use loftspec::document::Document;
use loftspec::hash::hash_document;
use loftspec::tree::Section;
use proptest::prelude::*;

/// The small document with its Feature's body set to `body`.
fn with_feature_body(body: &str) -> Document {
    let mut document = small_document();
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(body.to_owned());
    document
}

/// The Feature's body after the small document holding `body` is written and read back.
fn feature_body_read_back(body: &str) -> Option<String> {
    let files = codec::write(&with_feature_body(body));
    let back = codec::read(&files).unwrap();
    back.products[0].feature_groups[0].features[0]
        .details
        .body
        .clone()
}

/// The small document whose Product holds one Section whose child Sections are `children`, each
/// a key's number and a name, in that order.
fn with_child_sections(children: &[(u32, &str)]) -> Document {
    let children = children
        .iter()
        .map(|(n, name)| section(*n, name, &format!("Section {n}.")))
        .collect();
    let mut document = small_document();
    document.products[0].details.sections = vec![Section {
        sections: children,
        ..section(70, "Notes", "Section 70.")
    }];
    document
}

#[test]
fn child_sections_keep_their_written_order_through_a_write_and_a_read() {
    let children = [(73, "Charlie"), (71, "Alpha"), (72, "Bravo")];
    let document = with_child_sections(&children);
    let files = codec::write(&document);
    let written: Vec<String> = (0..3)
        .map(|index| {
            let route = ["sections", "0", "sections", &index.to_string(), "key"];
            scalar(&files, PRODUCT, &route)
        })
        .collect();
    let expected: Vec<String> = children.iter().map(|(n, _)| key(*n).to_string()).collect();
    assert_eq!(written, expected, "neither by key nor by name");
    let back = codec::read(&files).unwrap();
    assert_eq!(
        back.products[0].details.sections,
        document.products[0].details.sections
    );
}

#[test]
fn the_order_of_child_sections_is_content() {
    let roots = |children: &[(u32, &str)]| {
        hash_document(&with_child_sections(children))
            .unwrap()
            .roots
            .content
    };
    assert_ne!(
        roots(&[(71, "Alpha"), (72, "Bravo")]),
        roots(&[(72, "Bravo"), (71, "Alpha")])
    );
}

#[test]
fn every_unicode_scalar_value_in_a_body_survives_a_write_and_a_read() {
    let body: String = ('\0'..=char::MAX).collect();
    assert_eq!(body.chars().count(), 0x11_0000 - 0x800);
    assert_eq!(feature_body_read_back(&body), Some(body));
}

proptest! {
    #[test]
    fn trailing_whitespace_in_a_body_survives_a_write_and_a_read(
        lines in prop::collection::vec("[a-z]{0,6}[ \\t]{1,4}", 1..5),
        feeds in 0..3usize,
    ) {
        let mut body = lines.join("\n");
        body.extend(std::iter::repeat_n('\n', feeds));
        prop_assert_eq!(feature_body_read_back(&body), Some(body));
    }

    #[test]
    fn a_tab_in_a_body_survives_a_write_and_a_read(
        before in "[ a-z\\n]{0,12}",
        after in "[ a-z\\t\\n]{0,12}",
    ) {
        let body = format!("{before}\t{after}");
        prop_assert_eq!(feature_body_read_back(&body), Some(body));
    }

    #[test]
    fn any_string_as_a_body_survives_a_write_and_a_read(body in any::<String>()) {
        prop_assert_eq!(feature_body_read_back(&body), Some(body));
    }
}
