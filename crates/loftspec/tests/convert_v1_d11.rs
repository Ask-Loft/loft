//! The Format's own specification as it stood in Loft v1 when this repository imported it into
//! `spec/`: the v1 export it was imported from, kept as a public converter fixture, converts
//! without a finding to a pinned state root and to the sidecar recorded beside it.

use loftspec::codec;
use loftspec::convert::v1::export::Export;
use loftspec::convert::v1::{Conversion, convert};
use loftspec::hash::hash_document;

const EXPORT: &str = include_str!("../../../testdata/v1/d11-sequence-3/loft-authored-state.json");
const SIDECAR: &str = include_str!("../../../testdata/v1/d11-sequence-3/v1-sidecar.json");

/// The state root of the converted export, from this crate's hashing. A change to it is a
/// deliberate format change, never a refresh to make a failure disappear.
const STATE_ROOT: &str = "914637b6613fc4c91473c1eeef24bc4bd84897b6ea0b078d2d5277fe379c4635";

fn converted() -> Conversion {
    convert(&Export::from_json(EXPORT).unwrap()).unwrap()
}

#[test]
fn the_export_converts_without_a_finding() {
    assert_eq!(converted().findings, []);
}

#[test]
fn the_converted_export_hashes_to_its_pinned_state_root() {
    let hashes = hash_document(&converted().document).unwrap();
    assert_eq!(hashes.roots.state.to_string(), STATE_ROOT);
}

#[test]
fn the_sidecar_is_the_one_recorded_beside_the_export() {
    assert_eq!(converted().sidecar.to_json(), SIDECAR);
}

#[test]
fn ac_119_the_converted_export_round_trips_through_text() {
    let document = converted().document;
    let read = codec::read(&codec::write(&document)).unwrap();
    assert_eq!(codec::write(&read), codec::write(&document));
}
