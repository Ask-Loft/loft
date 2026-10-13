# Changelog

Releases of the `loft` command line and the `loftspec` and `loftspec-store` libraries. The
Format and the hashing algorithm carry their own versions, which `loft --version` prints.

## 2.0.0 (unreleased)

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
- Structural and language rules, a three-way merge over keys that writes typed conflict records,
  and the settling of two publishes that meet.

The command line:

- `loft init`, `fmt`, `check`, `render`, `import v1`, `publish`, `verify` and `merge-driver`,
  with `--install` and `--command` for a `loft` started by a dev-dependency launcher.
- `--format json` on every command, each document described by a schema under `schemas/`.
- Exit codes 0 on success, 1 on an error, a refusal or a failed check, and 2 on a usage error.
