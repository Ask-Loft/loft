#!/bin/sh
# Checks that a tag may be released: it is a release `vX.Y.Z` or a prerelease
# `vX.Y.Z-alpha.N`, `-beta.N` or `-rc.N` of the version every published crate carries, its
# commit is on main (or, for a prerelease, on develop), and CHANGELOG.md dates that version.
# The commit is the tag's once the tag exists, and `HEAD` before it does. Where GitHub's runner
# names its outputs file in GITHUB_OUTPUT, the check writes there the npm dist-tag to publish
# under, `channel`: `latest` for a release, or the prerelease's identifier. Run by the release
# workflow before anything is published; run it by hand before tagging.
#   scripts/release-check.sh v2.0.1
set -eu
tag=$1
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
fail() {
  echo "release-check: $*" >&2
  exit 1
}
if echo "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+$'; then
  channel=latest
  branches="main"
elif echo "$tag" | grep -Eq '^v[0-9]+\.[0-9]+\.[0-9]+-(alpha|beta|rc)\.[0-9]+$'; then
  channel=$(echo "$tag" | sed -E 's/^[^-]*-([a-z]+)\..*$/\1/')
  branches="develop main"
else
  fail "the tag $tag names neither a release vX.Y.Z nor a prerelease vX.Y.Z-alpha.N, vX.Y.Z-beta.N or vX.Y.Z-rc.N"
fi
versions=$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.publish != []) | .version' | sort -u)
[ "$(echo "$versions" | wc -l | tr -d ' ')" = 1 ] ||
  fail "the published crates carry different versions: $(echo $versions)"
[ "$tag" = "v$versions" ] || fail "the tag $tag does not name the crates' version $versions"
commit=$(git rev-parse -q --verify "$tag^{commit}" || git rev-parse HEAD)
on=
for branch in $branches; do
  if git merge-base --is-ancestor "$commit" "origin/$branch" 2>/dev/null; then
    on=$branch
  fi
done
if [ -z "$on" ]; then
  short=$(git rev-parse --short "$commit")
  [ "$channel" = latest ] && fail "the commit of $tag, $short, is not on main"
  fail "the commit of $tag, $short, is on neither develop nor main"
fi
grep -Eq "^## $versions \([0-9]{4}-[0-9]{2}-[0-9]{2}\)$" CHANGELOG.md ||
  fail "CHANGELOG.md has no dated entry \"## $versions (YYYY-MM-DD)\""
if [ -n "${GITHUB_OUTPUT:-}" ]; then
  echo "channel=$channel" >>"$GITHUB_OUTPUT"
fi
echo "release-check: $tag may be released"
