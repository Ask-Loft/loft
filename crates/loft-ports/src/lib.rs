//! The ports Loft's stores share, and nothing that knows a document or Yard Log: the journaled
//! save over plain paths, in a journal directory its caller names, that the next command
//! finishes or undoes after a crash; the runner through which Loft reaches git, which the
//! Software Plant's simulator stands in for; and the clock.

pub mod clock;
mod error;
pub mod git;
pub mod journal;
mod plan;
pub mod sync;

pub use error::{PortError, io_error};
