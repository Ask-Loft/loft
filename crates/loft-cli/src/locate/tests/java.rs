//! Java tests.

use super::{canonical, located, names, parses_cleanly};

const JAVA: &str = r#"
package shelf;

import org.junit.jupiter.api.Test;

/** The shelf's tests. */
class ShelfTest {
    @Test
    void addsABook() {
        assertEquals(1, shelf.add("Dune"));
    }

    @Nested
    @DisplayName("when empty")
    class WhenEmpty {
        @ParameterizedTest
        @ValueSource(ints = {1, 2,})
        public void holdsNothing(int count) {
            assertTrue(shelf.isEmpty());
        }
    }
}

interface Shared {
    default void inAnInterface() {}
}

enum Kind {
    BOOK;

    void inAnEnum() {}
}

record Entry(String title) {
    void inARecord() {}
}
"#;

#[test]
fn a_java_method_is_a_test_inside_classes_interfaces_enums_and_records() {
    assert!(parses_cleanly("ShelfTest.java", JAVA));
    let names = names("ShelfTest.java", JAVA);
    for name in [
        "addsABook",
        "holdsNothing",
        "inAnInterface",
        "inAnEnum",
        "inARecord",
    ] {
        assert!(names.contains(&name.to_owned()), "{names:?}");
    }
    for name in ["Shared", "Kind", "BOOK", "Entry", "assertEquals"] {
        assert!(!names.contains(&name.to_owned()), "{names:?}");
    }
}

#[test]
fn a_java_class_is_a_test_nested_or_not() {
    let tests = located("ShelfTest.java", JAVA);
    assert!(
        tests.contains(&("ShelfTest".to_owned(), 7, 22)),
        "{tests:?}"
    );
    assert!(
        tests.contains(&("WhenEmpty".to_owned(), 13, 21)),
        "{tests:?}"
    );
}

#[test]
fn the_annotations_and_modifiers_before_a_java_method_or_class_are_part_of_the_test() {
    let tests = located("ShelfTest.java", JAVA);
    assert!(
        tests.contains(&("addsABook".to_owned(), 8, 11)),
        "{tests:?}"
    );
    assert!(
        tests.contains(&("holdsNothing".to_owned(), 16, 20)),
        "{tests:?}"
    );
    assert_eq!(
        canonical("ShelfTest.java", JAVA, "holdsNothing"),
        "@ ParameterizedTest @ ValueSource ( ints = { 1 , 2 } ) public void holdsNothing ( int count ) { assertTrue ( shelf . isEmpty ( ) ) ; }"
    );
    let class = canonical("ShelfTest.java", JAVA, "WhenEmpty");
    assert!(
        class.starts_with("@ Nested @ DisplayName ( \"when empty\" ) class WhenEmpty {"),
        "{class}"
    );
}
