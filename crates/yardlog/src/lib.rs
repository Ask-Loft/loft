//! The `yardlog` format: the model of Yard Log, the record a branch keeps of how its work was
//! planned, asked about, decided and done; its records and their keys; the rules a write is held
//! to; the format's versions; its canonical YAML; and the merge of two copies. It performs no
//! I/O: a store reads and writes its files, and the clock and the actor are given to it.

pub mod act;
pub mod body;
pub mod codec;
pub mod datum;
pub mod format;
pub mod input;
pub mod keys;
pub mod log;
pub mod merge;
pub mod query;
pub mod respond;
mod rules;
pub mod settle;
pub mod write;

pub use act::{Act, Actor, Agent, Channel, Time};
pub use format::Format;
pub use keys::{BlockKey, Handle, Key, StageKey, SubmissionKey, YardLogKey};
pub use log::YardLog;
