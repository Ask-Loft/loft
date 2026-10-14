//! The integration tests of the Software Plant, as one binary: a module for each file beside
//! this one, and the modules they share. One binary is linked once, and checked once by the
//! operating system before its first run.

mod clock;
mod git;
mod published;
mod reach;
