//! The integration tests of loft-ports, as one binary: a module for each file beside this one,
//! and the modules they share. One binary is linked once, and checked once by the operating
//! system before its first run.

mod common;

mod boundary;
mod clock;
mod git;
mod journal;
