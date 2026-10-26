#!/bin/sh
# Rewrites the committed plugin packages from the skill's source: `loft skill install` composes
# the source for each host in a scratch directory, and each package replaces the plugin's copy.
# Yard's common workflow and each host adaptation compose one file beside loft-spec.
# Edit only crates/loft-cli/skill/, then run this script; guards fail on drift.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
discovery=".agents/skills/loft-spec"
if [ ! -L "$discovery" ] || [ "$(readlink "$discovery")" != "../../crates/loft-cli/skill/loft-spec" ]; then
  printf '%s\n' 'The Codex discovery entry must link to the canonical skill source.' >&2
  exit 1
fi
if [ ! -f "$discovery/SKILL.md" ] || [ ! -f "$discovery/agents/openai.yaml" ]; then
  printf '%s\n' 'The Codex discovery link must resolve to the router and Codex metadata.' >&2
  exit 1
fi
cargo build --quiet --release -p loft-cli
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
# Packaging is not a use of `loft`: it sends no event, asks for no update and needs no settings.
(cd "$scratch" && LOFT_TELEMETRY=off LOFT_UPDATE_CHECK=off "$root/target/release/loft" skill install --no-plugin >/dev/null)
for pair in "claude:.claude/skills/loft-spec" "codex:.agents/skills/loft-spec"; do
  host=${pair%%:*}
  package=${pair#*:}
  target="plugins/$host/loft/skills/loft-spec"
  rm -rf "$target"
  mkdir -p "$(dirname "$target")"
  cp -R "$scratch/$package" "$target"
done
yard_source="crates/loft-cli/skill/yard"
for host in claude codex; do
  yard_target="plugins/$host/loft/skills/yard"
  rm -rf "$yard_target"
  mkdir -p "$yard_target"
  {
    cat "$yard_source/SKILL.md"
    printf '\n'
    cat "$yard_source/agents/$host.md"
  } > "$yard_target/SKILL.md"
done
