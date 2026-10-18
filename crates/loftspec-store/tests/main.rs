//! The integration tests of loftspec-store, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

mod common;
mod fixtures;
#[path = "common/real_git.rs"]
mod real_git;

mod clock_contract;
mod conformance;
mod git_contract;
mod git_contract_base;
mod git_contract_stopped;
mod journal;
mod live_export;
mod openspec_project;
mod openspec_render;
mod publish_save;
mod simulators;
mod stores;
