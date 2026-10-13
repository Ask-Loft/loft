//! `loft init` (F-36): an empty document created in the current directory, with a fresh
//! document key. Refused, with nothing written, where `spec/` exists already (RQ-129).

use std::path::Path;
use std::process::ExitCode;

use loftspec::codec;
use loftspec::document::Document;
use loftspec::ids::DocumentId;
use loftspec_store::FileStore;

pub fn run(root: &Path) -> ExitCode {
    let document = Document::empty(DocumentId::mint());
    match FileStore::open(root).and_then(|store| store.create(codec::write(&document))) {
        Ok(()) => {
            println!("created spec/, document key {}", document.id);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
