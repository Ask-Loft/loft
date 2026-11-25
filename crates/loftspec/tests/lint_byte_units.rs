//! `V2-014` (F-21): a quantity in a decimal byte unit in a document that also states one in a
//! binary byte unit and defines no decimal byte unit.

use crate::common::lint::{configured, findings, keys, of, severity};
use crate::common::{key, section, small_document, term};
use loftspec::decision::DecisionState;
use loftspec::document::Document;
use loftspec::identity::Key;
use loftspec::ids::OptionId;
use loftspec::lint::{Finding, RuleId, Severity};
use loftspec::statement::{EarsForm, Statement};

/// The Feature F-1, keyed 3.
const FEATURE: u32 = 3;
/// The Product P-1, keyed 1.
const PRODUCT: u32 = 1;
/// The decision ADR-1, keyed 20.
const DECISION: u32 = 20;

/// The small document with F-1's body and P-1's body set.
fn with_bodies(feature: &str, product: &str) -> Document {
    let mut document = small_document();
    document.products[0].details.body = Some(product.to_owned());
    document.products[0].feature_groups[0].features[0]
        .details
        .body = Some(feature.to_owned());
    document
}

fn byte_units(document: &Document) -> Vec<Finding> {
    of(&findings(document), RuleId::V2014)
        .into_iter()
        .cloned()
        .collect()
}

/// The element each `V2-014` finding names and the unit it is about.
fn named(document: &Document) -> Vec<(Vec<Key>, Option<String>)> {
    byte_units(document)
        .iter()
        .map(|finding| (keys(finding), finding.term.clone()))
        .collect()
}

fn one(element: u32, unit: &str) -> Vec<(Vec<Key>, Option<String>)> {
    vec![(vec![key(element)], Some(unit.to_owned()))]
}

#[test]
fn a_decimal_quantity_beside_a_binary_one_is_one_finding_on_the_decimal_element() {
    let document = with_bodies("A page holds 512 MB.", "A block holds 4 KiB.");
    assert_eq!(named(&document), one(FEATURE, "MB"));
    assert_eq!(severity(RuleId::V2014), Severity::Warning);
}

#[test]
fn each_decimal_unit_is_recognized() {
    for unit in ["kB", "KB", "MB", "GB", "TB"] {
        let document = with_bodies(&format!("A page holds 5 {unit}."), "A block holds 4 KiB.");
        assert_eq!(named(&document), one(FEATURE, unit), "{unit}");
    }
}

#[test]
fn each_binary_unit_is_recognized() {
    for unit in ["KiB", "MiB", "GiB", "TiB"] {
        let document = with_bodies("A page holds 5 MB.", &format!("A block holds 4 {unit}."));
        assert_eq!(named(&document), one(FEATURE, "MB"), "{unit}");
    }
}

#[test]
fn a_unit_is_matched_in_its_own_case() {
    for (feature, product) in [
        ("A link carries 5 Mb.", "A block holds 4 KiB."),
        ("A page holds 5 mb.", "A block holds 4 KiB."),
        ("A page holds 5 MB.", "A block holds 4 kib."),
        ("A page holds 5 MB.", "A block holds 4 MIB."),
    ] {
        assert!(
            byte_units(&with_bodies(feature, product)).is_empty(),
            "{feature} {product}"
        );
    }
}

#[test]
fn a_quantity_takes_nothing_one_space_one_no_break_space_or_one_hyphen_before_its_unit() {
    for written in [
        "512MB",
        "512 MB",
        "512\u{a0}MB",
        "16-MB",
        "1,024 MB",
        "1.5 MB",
    ] {
        let document = with_bodies(&format!("A page holds {written}."), "A block holds 4 KiB.");
        assert_eq!(named(&document), one(FEATURE, "MB"), "{written}");
    }
    for written in ["512  MB", "512 - MB", "v512MB", "512MBs", "x5 MB"] {
        let document = with_bodies(&format!("A page holds {written}."), "A block holds 4 KiB.");
        assert!(byte_units(&document).is_empty(), "{written}");
    }
}

#[test]
fn a_unit_with_no_number_before_it_states_no_quantity() {
    let document = with_bodies("Sizes are given in MB.", "A block holds 4 KiB.");
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
    let document = with_bodies("A page holds 5 MB.", "Blocks are counted in KiB.");
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
}

#[test]
fn a_quality_attribute_whose_unit_is_a_byte_unit_states_a_quantity_in_it() {
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    document.products[0].feature_groups[0].features[0].quality_attributes[0]
        .target
        .unit = "MB".to_owned();
    assert_eq!(named(&document), one(10, "MB"));
}

#[test]
fn a_quality_attributes_target_and_a_constraints_required_value_are_read() {
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    document.components[0].component.quality_attributes[0]
        .target
        .scenario = "formatting a tree of 20 MB".to_owned();
    assert_eq!(named(&document), one(8, "MB"));
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    document.components[0].component.constraints[0]
        .binding
        .as_mut()
        .unwrap()
        .required_value = "64 MB".to_owned();
    assert_eq!(named(&document), one(7, "MB"));
}

#[test]
fn a_name_a_statement_and_a_section_are_read() {
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    document.products[0].feature_groups[0].features[0].name = "Pages of 5 MB".to_owned();
    assert_eq!(named(&document), one(FEATURE, "MB"));
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    let feature = &mut document.products[0].feature_groups[0].features[0];
    feature.details.sections = vec![section(64, "Pages", "A page holds 5 MB.")];
    assert_eq!(named(&document), one(64, "MB"));
    let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
    document.products[0].feature_groups[0].features[0].requirements[0].statement =
        Statement::Form(EarsForm::Ubiquitous {
            system: "The writer".to_owned(),
            shall: "write pages of 5 MB.".to_owned(),
        });
    assert_eq!(named(&document), one(4, "MB"));
}

#[test]
fn text_in_a_code_span_or_a_fenced_code_block_states_no_quantity() {
    let document = with_bodies("A page holds `512 MB`.", "A block holds 4 KiB.");
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
    let document = with_bodies(
        "A page:\n\n```\nsize = 512 MB\n```\n",
        "A block holds 4 KiB.",
    );
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
    let document = with_bodies("A page holds 512 MB.", "A block holds `4 KiB`.");
    assert_eq!(byte_units(&document), Vec::<Finding>::new());
}

#[test]
fn a_decision_is_read_in_every_state() {
    let selected = OptionId::new(key(23));
    let states = [
        DecisionState::Proposed,
        DecisionState::Accepted {
            selected_option: selected,
            outcome: "Kept.".to_owned(),
        },
        DecisionState::Rejected {
            rationale: "Not needed.".to_owned(),
        },
        DecisionState::Retired {
            selected_option: selected,
            outcome: "Kept.".to_owned(),
            rationale: "Over.".to_owned(),
        },
    ];
    for state in states {
        let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
        document.decisions[0].state = state;
        document.decisions[0].context = "A file is about 48 KB.".to_owned();
        assert_eq!(named(&document), one(DECISION, "KB"));
    }
}

#[test]
fn every_text_field_of_a_decision_is_read() {
    let edits: [fn(&mut loftspec::decision::Decision); 9] = [
        |decision| decision.title = "Pages of 5 MB".to_owned(),
        |decision| decision.options[0].title = "Pages of 5 MB".to_owned(),
        |decision| {
            decision.state = DecisionState::Rejected {
                rationale: "Pages of 5 MB are too big.".to_owned(),
            };
        },
        |decision| {
            if let loftspec::decision::DriverPayload::Local { label, .. } =
                &mut decision.drivers[0].payload
            {
                *label = "Pages of 5 MB".to_owned();
            }
        },
        |decision| decision.options[0].description = "Send 5 MB at once.".to_owned(),
        |decision| decision.assessments[0].rationale = "5 MB is too much".to_owned(),
        |decision| decision.consequences[0].statement = "A page takes 5 MB.".to_owned(),
        |decision| {
            decision.state = DecisionState::Accepted {
                selected_option: OptionId::new(key(23)),
                outcome: "Pages are 5 MB.".to_owned(),
            };
        },
        |decision| {
            if let loftspec::decision::DriverPayload::Local { statement, .. } =
                &mut decision.drivers[0].payload
            {
                *statement = "A page fits in 5 MB.".to_owned();
            }
        },
    ];
    for edit in edits {
        let mut document = with_bodies("Nothing measured.", "A block holds 4 KiB.");
        edit(&mut document.decisions[0]);
        assert_eq!(named(&document), one(DECISION, "MB"));
    }
}

#[test]
fn a_binary_quantity_in_a_decision_counts_too() {
    let mut document = with_bodies("A page holds 5 MB.", "Nothing measured.");
    document.decisions[0].context = "A block holds 4 KiB.".to_owned();
    assert_eq!(named(&document), one(FEATURE, "MB"));
}

#[test]
fn each_element_that_states_a_decimal_quantity_is_one_finding() {
    let document = with_bodies(
        "A page holds 5 MB and a row 2 KB.",
        "A block holds 4 KiB and 5 MB.",
    );
    assert_eq!(
        named(&document),
        vec![
            (vec![key(PRODUCT)], Some("MB".to_owned())),
            (vec![key(FEATURE)], Some("MB".to_owned())),
        ]
    );
}

#[test]
fn one_element_that_states_both_families_is_a_finding() {
    let document = with_bodies("A page holds 5 MB in blocks of 4 KiB.", "Nothing measured.");
    assert_eq!(named(&document), one(FEATURE, "MB"));
}

#[test]
fn a_document_with_one_family_of_units_yields_no_finding() {
    assert_eq!(
        byte_units(&with_bodies("A page holds 5 MB.", "A block holds 4 KB.")),
        Vec::<Finding>::new()
    );
    assert_eq!(
        byte_units(&with_bodies("A page holds 5 MiB.", "A block holds 4 KiB.")),
        Vec::<Finding>::new()
    );
}

#[test]
fn a_decimal_unit_given_as_a_count_of_bytes_settles_the_document() {
    for definition in [
        "Here 1 MB = 1,000,000 bytes.",
        "A MB is 1,000,000 bytes.",
        "A MB means 10^6 bytes.",
        "One MB equals exactly 1 000 000 bytes.",
        "MB denotes 1000000 bytes.",
        "MB stands for 10^6 bytes.",
        "Sizes are in MB (10^6 bytes).",
        "Sizes are in kB (kilobytes).",
    ] {
        let document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
        let mut defined = document.clone();
        defined.components[0].component.details.body = Some(definition.to_owned());
        assert!(byte_units(&defined).is_empty(), "{definition}");
        let mut decided = document.clone();
        decided.decisions[0].context = definition.to_owned();
        assert!(byte_units(&decided).is_empty(), "{definition}");
        let mut termed = document;
        let mut entry = term(65, "page size", &[]);
        entry.definition = definition.to_owned();
        termed.terms.push(entry);
        assert!(byte_units(&termed).is_empty(), "{definition}");
    }
}

#[test]
fn a_term_named_after_a_decimal_unit_settles_the_document() {
    for name in ["MB", "KB", "megabyte", "Kilobytes"] {
        let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
        document.terms.push(term(65, name, &[]));
        assert!(byte_units(&document).is_empty(), "{name}");
    }
}

#[test]
fn a_definition_of_a_binary_unit_alone_leaves_the_finding() {
    let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
    document.terms.push(term(65, "KiB", &[]));
    document.components[0].component.details.body =
        Some("A KiB is 1,024 bytes; see MiB (mebibytes).".to_owned());
    assert_eq!(named(&document), one(FEATURE, "MB"));
}

#[test]
fn the_finding_names_its_unit_and_the_key_of_a_binary_element() {
    let document = with_bodies("A page holds 512 MB.", "A block holds 4 KiB.");
    let found = byte_units(&document);
    assert_eq!(found[0].term.as_deref(), Some("MB"));
    assert!(
        found[0].message.contains(&key(PRODUCT).to_string()),
        "{}",
        found[0].message
    );
}

#[test]
fn a_suppression_of_the_decimal_element_drops_its_finding() {
    let document = with_bodies("A page holds 512 MB.", "A block holds 4 KiB.");
    let yaml = format!(
        "suppressions:\n  - rule: V2-014\n    key: {}\n    reason: \"Sizes as the vendor states them.\"\n",
        key(FEATURE)
    );
    assert_eq!(
        of(&configured(&document, &yaml), RuleId::V2014),
        Vec::<&Finding>::new()
    );
    let yaml = format!(
        "suppressions:\n  - rule: V2-014\n    key: {}\n    reason: \"The block size is exact.\"\n",
        key(PRODUCT)
    );
    assert_eq!(of(&configured(&document, &yaml), RuleId::V2014).len(), 1);
}

#[test]
fn an_emphasized_unit_or_number_is_read_as_the_text_shows_it() {
    for (feature, product) in [
        ("A page holds 512 **MB**.", "A block holds 4 KiB."),
        ("A page holds 512 MB.", "A block holds 4 *KiB*."),
        ("A page holds **512 MB**.", "A block holds _4_ KiB."),
    ] {
        assert_eq!(
            named(&with_bodies(feature, product)),
            one(FEATURE, "MB"),
            "{feature} {product}"
        );
    }
}

#[test]
fn a_code_span_beside_a_unit_or_a_number_states_no_quantity() {
    for feature in ["A page holds `512` MB.", "A page holds 512 `MB`."] {
        let document = with_bodies(feature, "A block holds 4 KiB.");
        assert_eq!(byte_units(&document), Vec::<Finding>::new(), "{feature}");
    }
}

#[test]
fn a_power_of_any_base_is_a_count_of_bytes() {
    for definition in ["Here 1 MB = 1000^2 bytes.", "A KB is 1000 ^ 1 bytes."] {
        let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
        document.components[0].component.details.body = Some(definition.to_owned());
        assert_eq!(byte_units(&document), Vec::<Finding>::new(), "{definition}");
    }
}

#[test]
fn a_decimal_unit_defined_by_its_full_name_settles_the_document() {
    for definition in [
        "A megabyte is 1,000,000 bytes.",
        "One kilobyte means 1,000 bytes.",
        "Gigabytes (10^9 bytes) are used.",
    ] {
        let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
        document.components[0].component.details.body = Some(definition.to_owned());
        assert_eq!(byte_units(&document), Vec::<Finding>::new(), "{definition}");
    }
    let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
    document.components[0].component.details.body =
        Some("A mebibyte is 1,048,576 bytes.".to_owned());
    assert_eq!(named(&document), one(FEATURE, "MB"));
}

#[test]
fn a_term_named_after_a_decimal_unit_in_the_plural_settles_the_document() {
    for name in ["MBs", "KBs"] {
        let mut document = with_bodies("A page holds 5 MB.", "A block holds 4 KiB.");
        document.terms.push(term(65, name, &[]));
        assert_eq!(byte_units(&document), Vec::<Finding>::new(), "{name}");
    }
}

#[test]
fn the_binary_element_named_is_another_than_the_finding_s_own_where_there_is_one() {
    let mut document = with_bodies("A page holds 5 MB in blocks of 4 KiB.", "Nothing measured.");
    document.decisions[0].context = "A block holds 4 KiB.".to_owned();
    let found = byte_units(&document);
    assert_eq!(found.len(), 1);
    assert!(
        found[0].message.contains(&key(DECISION).to_string()),
        "{}",
        found[0].message
    );
    let document = with_bodies("A page holds 5 MB in blocks of 4 KiB.", "Nothing measured.");
    let found = byte_units(&document);
    assert!(
        found[0].message.contains(&key(FEATURE).to_string()),
        "{}",
        found[0].message
    );
}
