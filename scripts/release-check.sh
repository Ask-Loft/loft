#!/bin/sh
# Checks that a tag may be released: it is `v` and the version every published crate carries,
# its commit is on main, and CHANGELOG.md dates that version. Run by the release workflow before
# anything is published; run it by hand to check a tag before pushing it.
#   scripts/release-check.sh v2.0.1
set -eu
tag=$1
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
fail() {
  echo "release-check: $*" >&2
  exit 1
}
versions=$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.publish != []) | .version' | sort -u)
[ "$(echo "$versions" | wc -l | tr -d ' ')" = 1 ] ||
  fail "the published crates carry different versions: $(echo $versions)"
[ "$tag" = "v$versions" ] || fail "the tag $tag does not name the crates' version $versions"
git merge-base --is-ancestor "$tag^{commit}" origin/main ||
  fail "the commit of $tag is not on main"
grep -Eq "^## $versions \([0-9]{4}-[0-9]{2}-[0-9]{2}\)$" CHANGELOG.md ||
  fail "CHANGELOG.md has no dated entry \"## $versions (YYYY-MM-DD)\""
echo "release-check: $tag may be released"
