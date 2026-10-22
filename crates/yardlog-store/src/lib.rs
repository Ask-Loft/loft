//! The store of Yard Log: a branch's Yard Log in the files under `.yard/<branch>/` at the top of
//! its worktree, read and written one write at a time under a lock in git's own directory for the
//! worktree, through the journaled save of `loft-ports`; the leases of waits beside that lock;
//! and what belongs to the machine rather than the branch, in `~/.yard/`. It reaches git only
//! through `loft-ports`' runner, and knows nothing of `spec/`.

mod error;
mod files;
pub mod git;
pub mod home;
pub mod lock;
pub mod place;
pub mod store;

pub use error::YardStoreError;
pub use files::{ATTRIBUTE, YARD};
pub use place::Place;
pub use store::{Held, Updated, YardStore};
