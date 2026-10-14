#!/usr/bin/env node
// Runs the `loft` binary that the platform package for this machine holds, passing it the
// arguments, the standard streams and the exit code.
"use strict";

const { spawnSync } = require("node:child_process");
const { binary } = require("../lib/binary.js");

function fail(message) {
  process.stderr.write(`loft: ${message}\n`);
  process.exit(1);
}

const found = binary();
if (found.problem) {
  fail(found.problem);
}

const result = spawnSync(found.path, process.argv.slice(2), { stdio: "inherit" });
if (result.error) {
  fail(`cannot run ${found.path}: ${result.error.message}`);
}
if (result.signal) {
  process.kill(process.pid, result.signal);
}
process.exit(result.status);
