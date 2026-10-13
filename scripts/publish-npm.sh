#!/bin/sh
# Publishes the npm packages that `scripts/npm-packages.sh` assembled in <out>: the platform
# packages first, then `@ask-loft/loft`, skipping any version the registry already holds, so a
# release run that stopped part way can run again. Further arguments go to `npm publish`.
#   scripts/publish-npm.sh <out> [--dry-run]
set -eu
out=$1
shift
for dir in "$out"/loft-* "$out/loft"; do
  name=$(jq -r .name "$dir/package.json")
  version=$(jq -r .version "$dir/package.json")
  if [ "$(npm view "$name@$version" version 2>/dev/null)" = "$version" ]; then
    echo "publish-npm: $name $version is already on npm"
  else
    npm publish "$dir" "$@"
  fi
done
