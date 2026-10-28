//! The integration tests of the `loft` command line, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

#[path = "common/base.rs"]
mod base;
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
#[path = "common/served.rs"]
mod served;
#[path = "common/snapshot.rs"]
mod snapshot;
#[path = "common/workflow.rs"]
mod workflow;
#[path = "common/yard.rs"]
mod yard;

mod check;
mod check_baseline;
mod checks_for_updates;
mod citations;
mod clock_port;
mod closed_output;
mod collector_contract;
mod find;
mod fmt;
mod import;
mod import_openspec;
mod init;
mod merge_contract;
mod merge_driver;
mod merge_driver_install;
mod needs_repository;
mod network_client;
mod npm;
mod npm_install;
mod plugins;
mod publish;
mod publish_former;
mod publish_meet_contract;
mod publish_settle;
mod reads_tree;
mod reconcile;
mod reconcile_located;
mod record_install;
mod release_check;
mod release_feed_contract;
mod release_workflow;
mod render;
mod render_openspec;
mod review;
mod schemas;
mod sends_telemetry;
mod shipped_binary;
mod skill;
mod skill_files;
mod skill_format;
mod skill_hosts;
mod skill_install;
mod source_limits;
mod status;
mod telemetry_event;
mod telemetry_failures;
mod telemetry_notice;
mod telemetry_order;
mod telemetry_switches;
mod telemetry_tokens;
mod terms;
mod test_locator;
mod test_modules;
mod update_check;
mod update_check_switches;
mod verify;
mod version;
mod writes_skill;
mod writes_skill_settings;
mod writes_tree;
mod yard_answer;
mod yard_base;
mod yard_callers;
mod yard_capabilities;
mod yard_close;
mod yard_codex;
mod yard_codex_cachewrite;
mod yard_codex_contract;
mod yard_codex_notifications;
mod yard_codex_skill;
mod yard_codex_tasks;
mod yard_codex_usage;
mod yard_codex_watch;
mod yard_commands;
mod yard_continue;
mod yard_delegation;
#[cfg(unix)]
mod yard_dogfood;
mod yard_event_ids;
mod yard_hosts;
mod yard_merge_contract;
mod yard_merge_driver;
mod yard_newer;
mod yard_page_browser;
mod yard_protocol;
mod yard_published;
mod yard_read;
mod yard_read_history;
mod yard_serve;
mod yard_serve_access;
mod yard_serve_base;
mod yard_serve_events;
mod yard_serve_inherited;
mod yard_serve_logs;
mod yard_serve_page_data;
mod yard_serve_subagents;
mod yard_settle;
mod yard_skill;
mod yard_take_up;
mod yard_upgrade;
mod yard_view_reports;
mod yard_views;
mod yard_wait;
mod yard_write;
