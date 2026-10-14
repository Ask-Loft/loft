#!/bin/sh
# Runs `loft --version` through the launcher of `@ask-loft/loft`, with this machine's platform
# package beside it, both extracted from the tarballs that `scripts/npm-packages.sh` packed in
# <out> as npm installs them. Fails unless the binary runs and reports the tarballs' version.
# The release runs it before the approval and again on the tarballs it is about to publish.
#   scripts/npm-smoke.sh <out>
set -eu
out=$(cd "$1" && pwd)
host=$(node -p 'process.platform + "-" + process.arch')
project=$(mktemp -d)
trap 'rm -rf "$project"' EXIT
for name in loft "loft-$host"; do
  mkdir -p "$project/node_modules/@ask-loft/$name"
  tar -xzf "$out/$name.tgz" --strip-components 1 -C "$project/node_modules/@ask-loft/$name"
done
version=$(tar -xzOf "$out/loft.tgz" package/package.json | jq -r .version)
reported=$(cd "$project" && node node_modules/@ask-loft/loft/bin/loft.js --version)
echo "$reported"
case "$reported" in
  "loft $version"*) ;;
  *) echo "npm-smoke: the launcher did not report loft $version" >&2; exit 1 ;;
esac
