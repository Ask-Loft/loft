//! What the formats of Loft share, and nothing that reaches outside the process: the reader and
//! writer of the strict YAML subset both the Format and `yardlog` are written in, the key their
//! records carry, and the UTC time, to the second, at which their publishes and Acts are dated.
//! It performs no I/O and depends on no other crate of the workspace.

pub mod key;
pub mod time;
pub mod yaml;

pub use key::{Key, KeyError};
pub use time::{Time, TimeError};
