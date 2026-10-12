//! The hasher's time on a large converted document, against a local v1 export.

mod common;

use std::time::{Duration, Instant};

use loftspec::hash::hash_document;

/// The fastest of `runs` timings of `work`.
fn fastest(runs: usize, mut work: impl FnMut()) -> Duration {
    (0..runs)
        .map(|_| {
            let started = Instant::now();
            work();
            started.elapsed()
        })
        .min()
        .unwrap()
}

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; the export is not in this repository"]
fn a_live_v1_export_hashes_in_reported_time() {
    let document = loftspec::convert::v1::convert(&common::v1::live_export())
        .unwrap()
        .document;
    let roots = hash_document(&document).unwrap().roots;
    let hashing = fastest(5, || {
        assert_eq!(hash_document(&document).unwrap().roots, roots);
    });
    println!("hash_document: {hashing:?}, state root {}", roots.state);
}
