//! Slugs: their syntax and the rule that derives one from a name (D-11 F-6, AC-245, AC-246).

mod common;

use common::v1::{convert_value, full_export};
use loftspec::slug::Slug;

fn derive(name: &str) -> String {
    Slug::derive(name, "feature-group").to_string()
}

#[test]
fn a_slug_is_lowercase_ascii_letters_and_digits_in_runs_joined_by_single_hyphens() {
    assert!(Slug::parse("node-registry-and-link-mapping").is_ok());
    assert!(Slug::parse("f2").is_ok());
    for bad in ["", "Keys", "-keys", "keys-", "a--b", "a_b", "a.b", "é"] {
        assert!(Slug::parse(bad).is_err(), "{bad}");
    }
}

#[test]
fn only_a_to_z_are_lowercased_and_every_other_run_is_one_hyphen() {
    assert_eq!(
        derive("Node, registry and link mapping"),
        "node-registry-and-link-mapping"
    );
    assert_eq!(derive("`loft fmt`"), "loft-fmt");
    assert_eq!(derive("Ünïcode Kelvin\u{212A}"), "n-code-kelvin");
    assert_eq!(derive("  RQ-12: the key  "), "rq-12-the-key");
}

#[test]
fn a_long_slug_is_cut_at_a_hyphen_within_48_characters() {
    let name = "one two three four five six seven eight nine ten eleven";
    let slug = derive(name);
    assert_eq!(slug, "one-two-three-four-five-six-seven-eight-nine-ten");
    assert!(slug.len() <= 48);
    assert_eq!(derive(&"x".repeat(60)), "x".repeat(48));
}

#[test]
fn a_name_that_leaves_nothing_takes_the_kind() {
    assert_eq!(derive("§ — ¶"), "feature-group");
}

#[test]
fn a_reserved_device_name_takes_the_suffix_1() {
    assert_eq!(derive("CON"), "con-1");
    assert_eq!(derive("lpt9"), "lpt9-1");
    assert_eq!(derive("console"), "console");
}

#[test]
fn a_repeated_slug_takes_the_first_free_suffix_in_order() {
    let mut slugs: Vec<Slug> = ["keys", "keys", "keys-2", "keys"]
        .iter()
        .map(|text| Slug::parse(text).unwrap())
        .collect();
    Slug::make_unique(&mut slugs);
    let written: Vec<&str> = slugs.iter().map(Slug::as_str).collect();
    assert_eq!(written, ["keys", "keys-2", "keys-2-2", "keys-3"]);
}

#[test]
fn the_converter_derives_each_slug_from_its_v1_name() {
    let document = convert_value(&full_export()).unwrap().document;
    let product = &document.products[0];
    assert_eq!(product.slug, Slug::derive(&product.name, "product"));
    let group = &product.feature_groups[0];
    assert_eq!(
        group.features[0].slug,
        Slug::derive(&group.features[0].name, "feature")
    );
    assert_eq!(
        document.components[0].slug,
        Slug::derive(&document.components[0].name, "component")
    );
    assert_eq!(
        document.decisions[0].slug,
        Slug::derive(&document.decisions[0].title, "decision")
    );
}
