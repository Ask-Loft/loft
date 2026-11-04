#!/bin/sh
# Rewrites the ELK that the Yard page lays its flowcharts out with, from the release the npm
# registry holds: `elkjs`'s bundled script, which runs without a worker, compressed, and its
# licence, under crates/loft-cli/yard-page/vendor/. `loft` serves the script from the compressed
# file. To move to another release, change VERSION here and in that folder's README.md, run this
# script, record the digest it prints there, and run the page's tests.
set -eu
VERSION=0.12.1
root=$(cd "$(dirname "$0")/.." && pwd)
vendor="$root/crates/loft-cli/yard-page/vendor"
scratch=$(mktemp -d)
trap 'rm -rf "$scratch"' EXIT
curl --fail --silent --show-error --location \
  "https://registry.npmjs.org/elkjs/-/elkjs-$VERSION.tgz" -o "$scratch/elk.tgz"
tar -xzf "$scratch/elk.tgz" -C "$scratch" package/lib/elk.bundled.js package/LICENSE.md package/package.json
grep -q "\"version\": \"$VERSION\"" "$scratch/package/package.json"
mkdir -p "$vendor"
# -n leaves out the name and time, so the same release always gives the same bytes.
gzip -9 -n -c "$scratch/package/lib/elk.bundled.js" > "$vendor/elk.bundled.js.gz"
cp "$scratch/package/LICENSE.md" "$vendor/elk.LICENSE"
shasum -a 256 "$scratch/package/lib/elk.bundled.js" | cut -d' ' -f1
