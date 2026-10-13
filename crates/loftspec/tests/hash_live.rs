//! The hasher's time on a large converted document, against a local v1 export.

use std::time::{Duration, Instant};

use crate::common;
use loftspec::hash::{hash_document, hash_indexed_with_views};

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
    let with_views = fastest(5, || {
        assert_eq!(
            hash_indexed_with_views(&document.index().unwrap()).0.roots,
            roots
        );
    });
    println!(
        "hash_document: {hashing:?}, with views: {with_views:?}, state root {}",
        roots.state
    );
}
