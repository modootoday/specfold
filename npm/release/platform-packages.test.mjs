import assert from "node:assert/strict";
import { mkdtempSync, readFileSync, statSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { test } from "node:test";
import { binaryPath } from "../launcher/launch.js";
import { PLATFORMS } from "../launcher/platforms.js";
import { optionalDependencies, writePlatformPackage } from "./platform-packages.mjs";

const pkg = { name: "@modootoday/specfold", version: "9.9.9", license: "MIT OR Apache-2.0" };

function binaries(names) {
  const dir = mkdtempSync(join(tmpdir(), "specfold-bins-"));
  for (const name of names) writeFileSync(join(dir, name), "#!/bin/sh\n");
  writeFileSync(join(dir, "LICENSE-MIT"), "mit");
  return dir;
}

test("a linux package carries both binaries, glibc and the release version", () => {
  const out = mkdtempSync(join(tmpdir(), "specfold-out-"));
  const { directory, manifest } = writePlatformPackage("x86_64-unknown-linux-gnu", binaries(["spec", "specfold"]), out, pkg);
  assert.equal(manifest.name, "@modootoday/specfold-linux-x64");
  assert.equal(manifest.version, "9.9.9");
  assert.deepEqual([manifest.os, manifest.cpu, manifest.libc], [["linux"], ["x64"], ["glibc"]]);
  assert.equal(statSync(join(directory, "bin", "spec")).mode & 0o111, 0o111);
  assert.equal(readFileSync(join(directory, "LICENSE-MIT"), "utf8"), "mit");
});

test("a windows package names the exe binaries", () => {
  const out = mkdtempSync(join(tmpdir(), "specfold-out-"));
  const { binaries: names, manifest } = writePlatformPackage("x86_64-pc-windows-msvc", binaries(["spec.exe", "specfold.exe"]), out, pkg);
  assert.deepEqual(names, ["spec.exe", "specfold.exe"]);
  assert.equal(manifest.libc, undefined);
});

test("a missing binary fails the package", () => {
  const out = mkdtempSync(join(tmpdir(), "specfold-out-"));
  assert.throws(() => writePlatformPackage("aarch64-apple-darwin", binaries(["spec"]), out, pkg), /Missing/);
});

test("the main package names every platform package at its own version", () => {
  const deps = optionalDependencies(pkg);
  assert.equal(Object.keys(deps).length, Object.keys(PLATFORMS).length);
  assert.ok(Object.values(deps).every((v) => v === "9.9.9"));
});

test("the launcher names an unsupported host and a missing platform package", () => {
  assert.throws(() => binaryPath("spec", "sunos", "sparc"), /sunos-sparc/);
  assert.throws(() => binaryPath("spec", "linux", "x64"), /not installed/);
});
