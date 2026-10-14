#!/bin/sh
# Publishes the npm packages that `scripts/npm-packages.sh` packed in <out>, from their
# tarballs: the platform packages first, then `@ask-loft/loft`, skipping any version the
# registry already holds, so a release run that stopped part way can run again. Further arguments go to `npm publish`.
#   scripts/publish-npm.sh <out> [--dry-run]
set -eu
out=$1
shift
for tarball in "$out"/loft-*.tgz "$out/loft.tgz"; do
  name=$(tar -xzOf "$tarball" package/package.json | jq -r .name)
  version=$(tar -xzOf "$tarball" package/package.json | jq -r .version)
  if [ "$(npm view "$name@$version" version 2>/dev/null)" = "$version" ]; then
    echo "publish-npm: $name $version is already on npm"
  else
    npm publish "$tarball" "$@"
  fi
done
