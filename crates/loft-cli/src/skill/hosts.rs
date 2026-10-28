//! The hosts selected for a skill installation; an omitted option retains both packages.

use clap::ValueEnum;

use super::compose::Host;

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum Hosts {
    Codex,
    ClaudeCode,
    All,
}

impl Hosts {
    pub const fn selected(self) -> &'static [Host] {
        match self {
            Self::Codex => &[Host::Codex],
            Self::ClaudeCode => &[Host::ClaudeCode],
            Self::All => &Host::ALL,
        }
    }

    pub fn includes_claude(self) -> bool {
        self.selected().contains(&Host::ClaudeCode)
    }
}
