#!/bin/sh
# Rewrites the Mermaid that the Yard page draws its diagrams with, from the release the npm
# registry holds: its minified script, compressed, and its licence, under
# crates/loft-cli/yard-page/vendor/. `loft` serves the script from the compressed file. To move to
# another release, change VERSION here and in that folder's README.md, run this script, and run
# the page's tests.
set -eu
VERSION=11.17.2
root=$(cd "$(dirname "$0")/.." && pwd)
vendor="$root/crates/loft-cli/yard-page/vendor"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
curl --fail --silent --show-error --location \
  "https://registry.npmjs.org/mermaid/-/mermaid-$VERSION.tgz" -o "$scratch/mermaid.tgz"
tar -xzf "$scratch/mermaid.tgz" -C "$scratch" package/dist/mermaid.min.js package/LICENSE
grep -q "version:\"$VERSION\"" "$scratch/package/dist/mermaid.min.js"
mkdir -p "$vendor"
# -n leaves out the name and time, so the same release always gives the same bytes.
gzip -9 -n -c "$scratch/package/dist/mermaid.min.js" > "$vendor/mermaid.min.js.gz"
cp "$scratch/package/LICENSE" "$vendor/mermaid.LICENSE"
shasum -a 256 "$scratch/package/dist/mermaid.min.js" | cut -d' ' -f1
