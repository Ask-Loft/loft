// Finds the `loft` binary that the platform package for this machine holds: one package per
// platform, `@ask-loft/loft-<platform>-<arch>`, is an optional dependency of `@ask-loft/loft`,
// and npm installs only the one whose `os` and `cpu` match.
"use strict";

// The binary's path, or the problem that leaves none, as `{ path }` or `{ problem }`.
function binary() {
  const name = `@ask-loft/loft-${process.platform}-${process.arch}`;
  const executable = process.platform === "win32" ? "loft.exe" : "loft";
  // The platform packages this release carries, as `package.json` lists them.
  const supported = Object.keys(require("../package.json").optionalDependencies || {});
  if (!supported.includes(name)) {
    return {
      problem:
        `no prebuilt binary exists for ${process.platform} on ${process.arch}; ` +
        "install `loft` with `cargo install loft-cli` instead",
    };
  }
  try {
    return { path: require.resolve(`${name}/bin/${executable}`) };
  } catch {
    return {
      problem:
        `the package ${name} is not installed; install @ask-loft/loft with its optional ` +
        "dependencies, without --no-optional or --omit=optional",
    };
  }
}

module.exports = { binary };
