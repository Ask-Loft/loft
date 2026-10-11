//! Parsing a sentence into its EARS slots (D-11 RQ-52): slots are kept only when the form's
//! template renders them back to the sentence byte for byte; anything else stays text.

use loftspec::statement::{EarsForm, Statement};

fn form(sentence: &str) -> EarsForm {
    match Statement::parse(sentence) {
        Statement::Form(form) => form,
        Statement::Text(_) => panic!("`{sentence}` should parse into slots"),
    }
}

fn stays_text(sentence: &str) {
    assert_eq!(
        Statement::parse(sentence),
        Statement::Text(sentence.to_owned()),
        "`{sentence}` should stay text"
    );
}

#[test]
fn ac_185_a_ubiquitous_sentence_parses_into_its_slots() {
    assert_eq!(
        form("The writer shall write a key on every node."),
        EarsForm::Ubiquitous {
            system: "The writer".to_owned(),
            shall: "write a key on every node.".to_owned(),
        }
    );
}

#[test]
fn ac_185_a_bare_proper_name_is_a_system() {
    assert_eq!(
        form("`loft check` shall run the validator and then the linter over the tree."),
        EarsForm::Ubiquitous {
            system: "`loft check`".to_owned(),
            shall: "run the validator and then the linter over the tree.".to_owned(),
        }
    );
}

#[test]
fn ac_185_each_prefixed_form_parses_into_its_slots() {
    assert_eq!(
        form("When formatting a node without a key, the formatter shall mint a key."),
        EarsForm::EventDriven {
            when: "formatting a node without a key".to_owned(),
            system: "the formatter".to_owned(),
            shall: "mint a key.".to_owned(),
        }
    );
    assert_eq!(
        form("While a conflict record exists, the validator shall report an error."),
        EarsForm::StateDriven {
            r#while: "a conflict record exists".to_owned(),
            system: "the validator".to_owned(),
            shall: "report an error.".to_owned(),
        }
    );
    assert_eq!(
        form("If `spec/` already exists, then `loft init` shall refuse and exit 1."),
        EarsForm::Unwanted {
            r#if: "`spec/` already exists".to_owned(),
            system: "`loft init`".to_owned(),
            shall: "refuse and exit 1.".to_owned(),
        }
    );
    assert_eq!(
        form("Where `--check` is given, `loft fmt` shall report drift without writing."),
        EarsForm::Optional {
            r#where: "`--check` is given".to_owned(),
            system: "`loft fmt`".to_owned(),
            shall: "report drift without writing.".to_owned(),
        }
    );
}

#[test]
fn the_comma_that_closes_the_precondition_is_the_last_before_shall() {
    assert_eq!(
        form("When a, b and c hold, the publisher shall refuse."),
        EarsForm::EventDriven {
            when: "a, b and c hold".to_owned(),
            system: "the publisher".to_owned(),
            shall: "refuse.".to_owned(),
        }
    );
}

#[test]
fn ac_186_a_sentence_outside_the_five_forms_stays_text() {
    stays_text("Keys are UUIDs.");
    stays_text("The writer must write a key.");
    stays_text("The writer shall");
}

#[test]
fn ac_186_a_combined_form_stays_text() {
    stays_text("When a node is read, if it has no key, the formatter shall mint one.");
    stays_text("While formatting, when a key is absent, the formatter shall mint one.");
}

#[test]
fn ac_186_a_sentence_the_template_cannot_reproduce_stays_text() {
    // The precondition ends at `the` with no comma: the form parses, but the template writes a
    // comma the sentence does not have.
    stays_text("When formatting the formatter shall mint a key.");
    // The template capitalises the prefix word.
    stays_text("when formatting, the formatter shall mint a key.");
    // Two spaces before `shall`.
    stays_text("The writer  shall write a key.");
    // `then` is the template's for the unwanted form only.
    stays_text("If a key is absent, the formatter shall mint one.");
}

#[test]
fn ac_186_a_ubiquitous_bare_name_with_a_lead_in_clause_stays_text() {
    // A bare proper name holds no later `the`: this is a lead-in clause, not a system phrase.
    stays_text("After the merge Loft shall rehash every ancestor.");
}

#[test]
fn every_parsed_form_renders_back_to_its_sentence() {
    for sentence in [
        "The writer shall write a key on every node.",
        "When publishing a tree that changed since the last publish, the publisher shall write only the lock and the bindings file.",
        "If two keys carry the same live label, then the validator shall report an error naming both keys.",
        "Where `--format json` is given, `loft` shall print one JSON document on standard output.",
    ] {
        assert_eq!(Statement::parse(sentence).render(), sentence);
        assert!(matches!(Statement::parse(sentence), Statement::Form(_)));
    }
}

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; no export is committed here"]
fn every_requirement_of_a_live_export_parses_or_stays_text() {
    use loftspec::convert::v1::export::{Export, Tier};
    let path = std::env::var("LOFTSPEC_V1_EXPORT").expect("LOFTSPEC_V1_EXPORT names an export");
    let export = Export::from_json(&std::fs::read_to_string(path).unwrap()).unwrap();
    let names: Vec<&str> = export
        .nodes
        .iter()
        .filter(|node| node.tier == Some(Tier::Requirement))
        .map(|node| node.name.as_str())
        .collect();
    let slots = names
        .iter()
        .filter(|name| matches!(Statement::parse(name), Statement::Form(_)))
        .count();
    for name in &names {
        let statement = Statement::parse(name);
        assert_eq!(statement.render(), *name);
        if matches!(statement, Statement::Text(_)) {
            println!("text: {name}");
        }
    }
    println!(
        "{slots} of {} Requirement names parse into slots",
        names.len()
    );
}
