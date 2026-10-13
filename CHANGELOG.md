# Changelog

Releases of the `loft` command line and the `loftspec` and `loftspec-store` libraries. The
Format and the hashing algorithm carry their own versions, which `loft --version` prints.

## Unreleased

- `@ask-loft/loft` on npm: prebuilt `loft` binaries for macOS, Linux and Windows on x64 and
  arm64, run by a launcher with no install script.
- Each release publishes from its tag, after the maintainer's approval, to crates.io and npm by
  Trusted Publishing, with the binaries attached to a GitHub release.

## 2.0.0 (2026-10-13)

The first public release. Version 2 because it succeeds the format of Loft v1, whose exports
`loft import v1` converts.

The Loft Specification Format, `loftspec/1`:

- A specification as a `spec/` tree of YAML files in a git repository: Products, Feature Groups,
  Features, Requirements in EARS forms with their acceptance criteria, Components, Constraints,
  Quality Attributes and architecture decisions.
- Every node carries a key; publishing gives nodes labels such as `RQ-12`, with aliases and
  withdrawn labels kept in `spec/labels.yaml`.
- Canonical hashing, `sha256-jcs/1`, into content, binding and state roots, pinned by
  `loft.lock.json`.
- Structural, language and writing rules, among them a compound criterion (`V2-005`), an
  oversized Requirement (`V2-006`), an obligation in a body (`V2-007`) and an article before a
  proper name (`V2-008`); a three-way merge over keys that writes typed conflict records,
  and the settling of two publishes that meet.

The command line:

- `loft init`, `fmt`, `check`, `render`, `import v1`, `publish`, `verify`, `merge-driver`,
  with `--install` and `--command` for a `loft` started by a dev-dependency launcher, and
  `skill install`.
- `--format json` on every command, each document described by a schema under `schemas/`, at
  version 1. Every document carries `schema`, `outcome` and `errors`; on an error it holds only
  those.
- Exit codes 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage error.

The agent skill:

- `loft-spec`, which teaches a coding agent to write, change and review a specification with
  `loft`: a short router and a reference for each workflow, with examples that run as tests.
- `loft init` writes it for Claude Code and Codex beside `spec/`, unless `--no-skill` is given;
  `loft skill install` writes it into any project.
- A plugin `loft` for each host, in this repository's own marketplaces.
