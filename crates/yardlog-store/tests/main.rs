//! The integration tests of yardlog-store, as one binary: a module for each file beside this
//! one, and the modules they share. One binary is linked once, and checked once by the operating
//! system before its first run.

mod common;

mod addressing;
mod boundary;
mod concurrent_writes;
mod delegation;
mod files;
mod git_contract;
mod home;
mod leases;
mod session_files;
