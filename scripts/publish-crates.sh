#!/bin/sh
# Publishes, in dependency order, every crate of the workspace whose version crates.io does not
# hold yet, so a release run that stopped part way can run again. With `--dry-run`, packages
# and verifies them without uploading.
set -eu
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
pending=0
for crate in $(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.publish != []) | "\(.name)@\(.version)"'); do
  name=${crate%@*}
  version=${crate#*@}
  status=$(curl -s -o /dev/null -w '%{http_code}' -A 'Ask-Loft/loft release' \
    "https://crates.io/api/v1/crates/$name/$version")
  if [ "$status" = 200 ]; then
    echo "publish-crates: $name $version is already on crates.io"
    set -- "$@" --exclude "$name"
  else
    pending=$((pending + 1))
  fi
done
if [ "$pending" = 0 ]; then
  echo "publish-crates: nothing to publish"
  exit 0
fi
cargo publish --workspace "$@"
