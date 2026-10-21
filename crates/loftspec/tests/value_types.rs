//! Value types a caller holds apart from a document: publish times in the order of the instants
//! they name, and a digest made from its 32 bytes.

use std::fmt::Write as _;

use loftspec::canonical::{Digest, Domain, digest};
use loftspec::publish::PublishedAt;
use proptest::prelude::*;
use serde_json::json;

fn at(seconds: u64) -> PublishedAt {
    PublishedAt::from_unix_seconds(seconds).unwrap()
}

#[test]
fn an_earlier_publish_time_orders_before_a_later_one() {
    assert!(at(1_700_000_000) < at(1_700_000_001));
    assert!(at(0) < at(253_402_300_799));
    assert_eq!(at(5).max(at(4)), at(5));
}

#[test]
fn two_publish_times_of_one_instant_are_equal() {
    assert_eq!(at(1_700_000_000), at(1_700_000_000));
    assert_eq!(
        at(1_700_000_000).cmp(&at(1_700_000_000)),
        std::cmp::Ordering::Equal
    );
}

proptest! {
    #[test]
    fn publish_times_read_from_their_text_order_as_their_instants(
        left in 0..=253_402_300_799u64,
        right in 0..=253_402_300_799u64,
    ) {
        let read = |seconds: u64| at(seconds).to_string().parse::<PublishedAt>().unwrap();
        prop_assert_eq!(read(left).cmp(&read(right)), left.cmp(&right));
    }

    #[test]
    fn a_digest_made_from_32_bytes_gives_back_those_bytes(bytes in any::<[u8; 32]>()) {
        prop_assert_eq!(*Digest::from_bytes(bytes).as_bytes(), bytes);
    }

    #[test]
    fn a_digest_made_from_32_bytes_is_written_as_their_hexadecimal_digits(
        bytes in any::<[u8; 32]>(),
    ) {
        let hex = bytes.iter().fold(String::new(), |mut hex, byte| {
            write!(hex, "{byte:02x}").unwrap();
            hex
        });
        prop_assert_eq!(Digest::from_bytes(bytes).to_string(), hex);
    }
}

#[test]
fn a_digest_made_from_the_bytes_of_a_digest_is_that_digest() {
    let made = digest(Domain::Node, &json!({"name": "Keys"}));
    assert_eq!(Digest::from_bytes(*made.as_bytes()), made);
}
