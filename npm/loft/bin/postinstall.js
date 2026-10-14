#!/usr/bin/env node
// The install script of `@ask-loft/loft`: records that npm installed `loft` by running
// `loft record-install npm` with the platform package's binary. An install never fails because
// of it: a missing binary, a run that fails and a record that cannot be written all end the
// script with 0, and it prints nothing. Where the script never runs, as when the package
// manager skips install scripts, `loft` infers the same from where its binary lies.
"use strict";

const { spawnSync } = require("node:child_process");

try {
  const found = require("../lib/binary.js").binary();
  if (found.path) {
    spawnSync(found.path, ["record-install", "npm"], { stdio: "ignore", timeout: 10000 });
  }
} catch {
  // Nothing an install needs depends on the record.
}
process.exit(0);
