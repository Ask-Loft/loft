//! `yardlog-store` stands beside `loftspec-store`, over `loft-ports`: it depends on no crate of
//! the Format and not on `loftspec-store`, reaches git only through `loft-ports`' runner, and
//! writes the files of Yard Log only through `loft-ports`' journal.

use std::fs;
use std::path::Path;

/// Each source file of the crate, by its name, with its text.
fn sources() -> Vec<(String, String)> {
    let src = Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found: Vec<(String, String)> = fs::read_dir(src)
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .map(|path| {
            let name = path.file_name().unwrap().to_string_lossy().into_owned();
            (name, fs::read_to_string(path).unwrap())
        })
        .collect();
    assert!(found.len() > 5, "the scan found {} files", found.len());
    found
}

/// The names the manifest's `[dependencies]` table lists.
fn dependencies() -> Vec<String> {
    let manifest =
        fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("Cargo.toml")).unwrap();
    manifest
        .lines()
        .skip_while(|line| line.trim() != "[dependencies]")
        .skip(1)
        .take_while(|line| !line.starts_with('['))
        .filter_map(|line| line.split(['=', '.']).next())
        .map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_store_of_yard_log_depends_on_no_crate_of_the_format_and_not_on_the_document_store() {
    let listed = dependencies();
    assert_eq!(listed, ["loft-ports", "thiserror", "yardlog"]);
    for (name, text) in sources() {
        for item in ["loftspec::", "loftspec_store"] {
            assert!(!text.contains(item), "{name} names {item}");
        }
    }
}

#[test]
fn the_store_of_yard_log_reaches_git_only_through_the_ports_runner() {
    for (name, text) in sources() {
        assert!(!text.contains("Command::new"), "{name} starts a command");
        if name != "git.rs" {
            assert!(!text.contains("Command"), "{name} names a command");
            assert!(!text.contains("loft_ports::git"), "{name} runs git");
        }
    }
}

#[test]
fn the_store_of_yard_log_writes_its_files_only_through_the_ports_journal() {
    let writes = [
        "fs::write",
        "File::create",
        "fs::rename",
        "OpenOptions",
        "remove_file",
    ];
    for (name, text) in sources() {
        // `~/.yard/` and the files of locks are the machine's, not Yard Log's.
        if name == "home.rs" || name == "lock.rs" {
            continue;
        }
        for call in writes {
            assert!(!text.contains(call), "{name} writes a file with {call}");
        }
    }
    let store = sources()
        .into_iter()
        .find(|(name, _)| name == "store.rs")
        .unwrap()
        .1;
    assert!(store.contains(".save(&changes)"));
}
