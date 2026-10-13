//! The statement record and its render templates (F-10, F-11).

use loftspec::statement::{EarsForm, Statement};

fn slots(text: &str) -> String {
    text.to_owned()
}

#[test]
fn ubiquitous_renders_as_system_shall_response() {
    let form = EarsForm::Ubiquitous {
        system: slots("The writer"),
        shall: slots("write a key on every node."),
    };
    assert_eq!(form.render(), "The writer shall write a key on every node.");
}

#[test]
fn event_driven_renders_with_when() {
    let form = EarsForm::EventDriven {
        when: slots("formatting a node without a key"),
        system: slots("the formatter"),
        shall: slots("mint a version 7 UUID as its key."),
    };
    assert_eq!(
        form.render(),
        "When formatting a node without a key, the formatter shall mint a version 7 UUID as its key."
    );
}

#[test]
fn state_driven_renders_with_while() {
    let form = EarsForm::StateDriven {
        r#while: slots("formatting"),
        system: slots("the reader"),
        shall: slots("accept a node without a key."),
    };
    assert_eq!(
        form.render(),
        "While formatting, the reader shall accept a node without a key."
    );
}

#[test]
fn unwanted_renders_with_if_and_then() {
    let form = EarsForm::Unwanted {
        r#if: slots("the manifest is absent"),
        system: slots("the reader"),
        shall: slots("report an error naming the expected path."),
    };
    assert_eq!(
        form.render(),
        "If the manifest is absent, then the reader shall report an error naming the expected path."
    );
}

#[test]
fn optional_renders_with_where() {
    let form = EarsForm::Optional {
        r#where: slots("a document is derived from a Loft v1 export"),
        system: slots("the publisher"),
        shall: slots("record the source state in the lock."),
    };
    assert_eq!(
        form.render(),
        "Where a document is derived from a Loft v1 export, the publisher shall record the source state in the lock."
    );
}

#[test]
fn the_system_slot_is_rendered_verbatim() {
    let form = EarsForm::Ubiquitous {
        system: slots("`loft check`"),
        shall: slots("exit 0."),
    };
    assert_eq!(form.render(), "`loft check` shall exit 0.");
}

#[test]
fn rendering_the_same_statement_twice_yields_identical_bytes() {
    let statement = || {
        Statement::Form(EarsForm::Unwanted {
            r#if: slots("two nodes carry one key"),
            system: slots("the reader"),
            shall: slots("report an error."),
        })
    };
    let sentence = "If two nodes carry one key, then the reader shall report an error.";
    assert_eq!(statement().render(), sentence);
    assert_eq!(statement().render(), sentence);
}

#[test]
fn a_text_statement_renders_as_its_text() {
    let statement = Statement::Text(slots("The converter shall store bodies verbatim."));
    assert_eq!(
        statement.render(),
        "The converter shall store bodies verbatim."
    );
}

#[test]
fn each_form_names_its_slots() {
    let forms = [
        (
            EarsForm::Ubiquitous {
                system: slots("s"),
                shall: slots("r"),
            },
            "ubiquitous",
        ),
        (
            EarsForm::EventDriven {
                when: slots("w"),
                system: slots("s"),
                shall: slots("r"),
            },
            "event_driven",
        ),
        (
            EarsForm::StateDriven {
                r#while: slots("w"),
                system: slots("s"),
                shall: slots("r"),
            },
            "state_driven",
        ),
        (
            EarsForm::Unwanted {
                r#if: slots("i"),
                system: slots("s"),
                shall: slots("r"),
            },
            "unwanted",
        ),
        (
            EarsForm::Optional {
                r#where: slots("w"),
                system: slots("s"),
                shall: slots("r"),
            },
            "optional",
        ),
    ];
    for (form, name) in forms {
        assert_eq!(form.kind().as_str(), name);
    }
}
