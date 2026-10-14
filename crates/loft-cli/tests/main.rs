//! The integration tests of the `loft` command line, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

#[path = "common/commands.rs"]
mod commands;
mod common;
mod conformance;
#[path = "common/help.rs"]
mod help;
#[path = "common/repo.rs"]
mod repo;
#[path = "common/schema.rs"]
mod schema;
#[path = "common/snapshot.rs"]
mod snapshot;

mod check;
mod citations;
mod find;
mod fmt;
mod import;
mod init;
mod merge_contract;
mod merge_driver;
mod merge_driver_install;
mod needs_repository;
mod npm;
mod plugins;
mod publish;
mod publish_meet_contract;
mod publish_settle;
mod reads_tree;
mod reconcile;
mod render;
mod schemas;
mod skill;
mod skill_install;
mod test_modules;
mod verify;
mod version;
mod writes_skill;
mod writes_tree;
