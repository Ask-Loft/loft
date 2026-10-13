#!/usr/bin/env node
// Runs the `loft` binary that the platform package for this machine holds: one package per
// platform, `@ask-loft/loft-<platform>-<arch>`, is an optional dependency of this one, and npm
// installs only the one whose `os` and `cpu` match.
"use strict";

const { spawnSync } = require("node:child_process");

const platform = `${process.platform}-${process.arch}`;
const name = `@ask-loft/loft-${platform}`;
const executable = process.platform === "win32" ? "loft.exe" : "loft";
// The platform packages this release carries, as `package.json` lists them.
const supported = Object.keys(require("../package.json").optionalDependencies || {});

function fail(message) {
  process.stderr.write(`loft: ${message}\n`);
  process.exit(1);
}

if (!supported.includes(name)) {
  fail(
    `no prebuilt binary exists for ${process.platform} on ${process.arch}; ` +
      "install `loft` with `cargo install loft-cli` instead",
  );
}

let binary;
try {
  binary = require.resolve(`${name}/bin/${executable}`);
} catch {
  fail(
    `the package ${name} is not installed; install @ask-loft/loft with its optional ` +
      "dependencies, without --no-optional or --omit=optional",
  );
}

const result = spawnSync(binary, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  fail(`cannot run ${binary}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status);
