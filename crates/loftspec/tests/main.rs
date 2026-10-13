//! The integration tests of loftspec, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

mod common;
#[path = "support/oracle.rs"]
mod oracle;

mod bindings;
mod boundary;
mod codec_conflicts;
mod codec_read;
mod codec_round_trip;
mod codec_sites;
mod conflicts;
mod content_hash;
mod convert_v1;
mod convert_v1_d11;
mod convert_v1_export;
mod convert_v1_links;
mod convert_v1_reading_list;
mod convert_v1_sidecar;
mod digest;
mod fmt;
mod fmt_names;
mod golden;
mod hash_live;
mod identity;
mod index;
mod label_allocation;
mod lint;
mod lint_craft;
mod lint_language;
mod lint_live;
mod lint_withdrawn;
mod lock;
mod manifest;
mod merge_file;
mod merge_labels;
mod merge_order;
mod names;
mod node;
mod prose;
mod publish;
mod publish_rewrite;
mod publish_settle;
mod render;
mod roots;
mod slug;
mod statement;
mod statement_parse;
mod v1_parity;
mod validate;
mod wire_names;
mod withdrawn;
mod yaml_oracle;
mod yaml_read;
mod yaml_write;
