//! A local v1 export through the file store, timed: the first save, opening and loading the tree,
//! and a save of the unchanged document, which writes nothing.

use std::time::Instant;
use std::{env, fs};

use crate::fixtures::{converted, with_lint};
use loftspec::codec;
use loftspec_store::{DocumentStore, FileStore};

#[test]
#[ignore = "reads a local v1 export named by LOFTSPEC_V1_EXPORT; the export is not in this repository"]
fn a_live_v1_export_loads_and_saves_through_the_file_store() {
    let path = env::var("LOFTSPEC_V1_EXPORT").expect("LOFTSPEC_V1_EXPORT names a v1 export");
    let text = fs::read_to_string(path).unwrap();
    let document = converted(&text);
    let scratch = with_lint();

    let started = Instant::now();
    let saved = FileStore::open(&scratch.0)
        .unwrap()
        .save(&document)
        .unwrap();
    let first = started.elapsed();

    let started = Instant::now();
    let mut store = FileStore::open(&scratch.0).unwrap();
    let files = store.read_files().unwrap();
    let disk = started.elapsed();
    let loaded = codec::read(&files).unwrap();
    let load = started.elapsed();
    assert_eq!(store.load().unwrap(), loaded);

    let started = Instant::now();
    let resaved = store.save(&loaded).unwrap();
    let unchanged = started.elapsed();
    assert!(resaved.is_empty(), "{resaved:?}");

    let bytes: usize = scratch.snapshot().values().map(Vec::len).sum();
    println!(
        "{} files, {bytes} bytes; first save {first:?}, open and load {load:?} ({disk:?} on \
         disk), unchanged save {unchanged:?}",
        saved.written.len()
    );
}
