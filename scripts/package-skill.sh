#!/bin/sh
# Rewrites the committed plugin packages from the skill's source: `loft skill install` composes
# the source for each host in a scratch directory, and each package replaces the plugin's copy.
# Edit only crates/loft-cli/skill/loft-spec/, then run this script; a guard fails on drift.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
cargo build --quiet --release -p loft-cli
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
(cd "$scratch" && "$root/target/release/loft" skill install >/dev/null)
for pair in "claude:.claude/skills/loft-spec" "codex:.agents/skills/loft-spec"; do
  host=${pair%%:*}
  package=${pair#*:}
  target="plugins/$host/loft/skills/loft-spec"
  rm -rf "$target"
  mkdir -p "$(dirname "$target")"
  cp -R "$scratch/$package" "$target"
done
