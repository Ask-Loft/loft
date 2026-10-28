#!/bin/sh
# Rewrites the committed plugin packages from the skill sources: `loft skill package` composes
# each host's loft-spec package and, for each host with a Yard adapter, its yard package, from
# the sources `loft` embeds, and refuses while the Codex discovery entry is not the link to the
# canonical skill source. Edit only crates/loft-cli/skill/, then run this script; guards fail on
# drift.
set -eu
cd "$(dirname "$0")/.."
exec cargo run -q -p loft-cli -- skill package
