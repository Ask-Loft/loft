//! The integration tests of the `loft` command line, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

#[path = "common/commands.rs"]
mod commands;
mod common;
mod conformance;
#[path = "common/help.rs"]
mod help;
#[path = "common/markdown.rs"]
mod markdown;
#[path = "common/repo.rs"]
mod repo;
#[path = "common/schema.rs"]
mod schema;
#[path = "common/snapshot.rs"]
mod snapshot;
#[path = "common/workflow.rs"]
mod workflow;

mod check;
mod citations;
mod closed_output;
mod collector_contract;
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
mod record_install;
mod release_check;
mod release_feed_contract;
mod release_workflow;
mod render;
mod schemas;
mod sends_telemetry;
mod shipped_binary;
mod skill;
mod skill_files;
mod skill_format;
mod skill_install;
mod telemetry_event;
mod telemetry_failures;
mod telemetry_notice;
mod telemetry_switches;
mod telemetry_tokens;
mod test_modules;
mod update_check_switches;
mod verify;
mod version;
mod writes_skill;
mod writes_tree;
