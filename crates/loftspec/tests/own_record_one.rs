//! One record's own record view and digest, found by its kind and key (Own record views): the
//! entry the whole document's own records give it, without building any other record's.

use loftspec::codec::{RecordKind, own_record, own_records};
use proptest::prelude::*;

use crate::common::generated::{generated, rich_document};
use crate::common::key;
use crate::common::own::{FEATURE, REQUIREMENT};

proptest! {
    #[test]
    fn each_records_own_record_found_by_its_kind_and_key_is_the_whole_documents_entry(
        document in generated(),
    ) {
        prop_assume!(document.is_some());
        let document = document.unwrap();
        for ((kind, key), record) in own_records(&document) {
            prop_assert_eq!(own_record(&document, kind, key), Some(record), "{:?} {}", kind, key);
        }
    }
}

#[test]
fn every_record_of_the_rich_document_is_found_by_its_kind_and_key() {
    let document = rich_document();
    let all = own_records(&document);
    assert!(all.keys().any(|(kind, _)| *kind == RecordKind::Term));
    assert!(all.keys().any(|(kind, _)| *kind == RecordKind::Decision));
    for ((kind, key), record) in all {
        assert_eq!(
            own_record(&document, kind, key),
            Some(record),
            "{kind:?} {key}"
        );
    }
}

#[test]
fn a_kind_and_key_no_record_carries_give_no_own_record() {
    let document = rich_document();
    assert_eq!(own_record(&document, REQUIREMENT, key(999)), None);
    assert_eq!(own_record(&document, RecordKind::Binding, key(999)), None);
}

#[test]
fn a_key_another_kind_of_record_carries_gives_no_own_record_for_the_kind_asked() {
    let document = rich_document();
    assert!(own_record(&document, REQUIREMENT, key(4)).is_some());
    for kind in [
        FEATURE,
        RecordKind::Decision,
        RecordKind::Tag,
        RecordKind::Category,
        RecordKind::Term,
    ] {
        assert_eq!(own_record(&document, kind, key(4)), None, "{kind:?}");
    }
    assert_eq!(own_record(&document, REQUIREMENT, key(20)), None);
}

#[test]
fn a_bindings_own_record_is_found_by_the_key_of_the_element_it_binds_departed_or_not() {
    let document = rich_document();
    let all = own_records(&document);
    for n in [4, 99] {
        let found = own_record(&document, RecordKind::Binding, key(n));
        assert_eq!(
            found.as_ref(),
            all.get(&(RecordKind::Binding, key(n))),
            "{n}"
        );
        assert!(found.is_some(), "{n}");
    }
}
