//! The telemetry notice: printed to standard error before the first event leaves a machine, so
//! that a person or an agent reading the run learns that telemetry is on, what it sends, where,
//! for how long, and how to switch it off.

/// The notice, naming the collector's `address`.
pub fn text(address: &str) -> String {
    format!(
        "\
loft: telemetry is on. After each command, loft sends one event to {address}:
  its schema and a random event id, loft's version, operating system and architecture, how it
  was installed, the hour, the command, its outcome and error kinds, how long it took, whether
  CI or an agent ran it, and random tokens for this installation, this checkout and its branch.
  It never sends the specification, the code, a path, a repository address, a name or an
  error message. The tokens are pseudonymous; the collector also sees the network address each
  event comes from. Events with tokens are kept for at most 30 days and aggregates without
  tokens for at most 90 days.
  Switch it off with `loft telemetry off`, or with LOFT_TELEMETRY=off for one shell or job.
  More: https://github.com/Ask-Loft/loft#telemetry-and-the-update-check
"
    )
}

#[cfg(test)]
mod tests {
    use super::text;

    #[test]
    fn the_notice_names_the_address_both_switches_and_the_retention() {
        let notice = text("https://collector.example/api/telemetry");
        for needed in [
            "https://collector.example/api/telemetry",
            "loft telemetry off",
            "LOFT_TELEMETRY=off",
            "30 days",
            "90 days",
            "network address",
            "pseudonymous",
        ] {
            assert!(notice.contains(needed), "{needed}");
        }
        assert!(!notice.contains("anonymous"), "{notice}");
    }
}
