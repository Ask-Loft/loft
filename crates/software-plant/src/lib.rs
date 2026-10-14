//! The Software Plant: simulators of the external systems Loft depends on, so that tests run
//! Loft's shipped binaries, unchanged, against scripted and repeatable behaviour. Each simulator
//! reaches the binary under test only through its search path or its environment, and a contract
//! suite runs the same scenarios against the simulator and the real system. The thinking behind
//! it is at <https://askloft.ai/vision>.
//!
//! This crate is test equipment: it is never published and never a dependency of a published
//! crate.

pub mod clock;
pub mod collector;
pub mod git;
pub mod http;
mod install;
pub mod real_git;
pub mod releases;

pub use clock::Clock;
pub use collector::Collector;
pub use git::GitScenario;
pub use install::{Installed, git_executable};
pub use releases::ReleaseFeed;
