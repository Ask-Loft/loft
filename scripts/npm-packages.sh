#!/bin/sh
# Packs the npm packages of a release from the `loft` binary built for each platform:
# `<binaries>/<platform>-<arch>/loft` (`loft.exe` on Windows) becomes the package
# `@ask-loft/loft-<platform>-<arch>`, packed as `<out>/loft-<platform>-<arch>.tgz`, and
# `@ask-loft/loft` gets the launcher under `npm/loft/` with every platform package as an
# optional dependency, packed as `<out>/loft.tgz`. Every package carries the version of `loft`,
# and none has an install script. A tarball keeps the binary's execute bits, which an artifact
# transfer drops from a plain file, so only tarballs leave this script.
#   scripts/npm-packages.sh <binaries> <out>
set -eu
binaries=$(cd "$1" && pwd)
mkdir -p "$2"
out=$(cd "$2" && pwd)
stage=$(mktemp -d)
trap 'rm -rf "$stage"' EXIT
root=$(cd "$(dirname "$0")/.." && pwd)
cd "$root"
platforms="darwin-arm64 darwin-x64 linux-arm64 linux-x64 win32-arm64 win32-x64"
version=$(cargo metadata --no-deps --format-version 1 |
  jq -r '.packages[] | select(.name == "loft-cli") | .version')
repository='{"type": "git", "url": "git+https://github.com/Ask-Loft/loft.git"}'

# The fields every package shares.
common() {
  jq -n --arg name "$1" --arg version "$version" --arg description "$2" \
    --argjson repository "$repository" \
    '{name: $name, version: $version, description: $description, license: "Apache-2.0",
      repository: $repository, homepage: "https://github.com/Ask-Loft/loft",
      publishConfig: {access: "public"}}'
}

optional="{}"
for platform in $platforms; do
  os=${platform%-*}
  cpu=${platform#*-}
  executable=loft
  [ "$os" = win32 ] && executable=loft.exe
  [ -f "$binaries/$platform/$executable" ] ||
    { echo "npm-packages: no binary at $binaries/$platform/$executable" >&2; exit 1; }
  name="@ask-loft/loft-$platform"
  dir="$stage/loft-$platform"
  mkdir -p "$dir/bin"
  cp "$binaries/$platform/$executable" "$dir/bin/$executable"
  chmod +x "$dir/bin/$executable"
  cp -L LICENSE NOTICE "$dir/"
  common "$name" "The loft binary for $os on $cpu; install @ask-loft/loft instead." |
    jq --arg os "$os" --arg cpu "$cpu" '. + {os: [$os], cpu: [$cpu]}' >"$dir/package.json"
  optional=$(echo "$optional" | jq --arg name "$name" --arg version "$version" '. + {($name): $version}')
done

dir="$stage/loft"
mkdir -p "$dir"
cp -R npm/loft/. "$dir/"
cp -L LICENSE NOTICE "$dir/"
common "@ask-loft/loft" \
  "Author, check, format and publish a specification in the Loft Specification Format" |
  jq --argjson optional "$optional" \
    '. + {keywords: ["specification", "requirements", "ears", "traceability", "git"],
          bin: {loft: "bin/loft.js"}, engines: {node: ">=18"},
          optionalDependencies: $optional}' >"$dir/package.json"
for dir in "$stage"/*/; do
  name=$(basename "$dir")
  packed=$(npm pack "$dir" --pack-destination "$stage" --silent)
  mv "$stage/$packed" "$out/$name.tgz"
done
echo "npm-packages: @ask-loft/loft $version and $(echo $platforms | wc -w | tr -d ' ') platform packages in $out"
