import assert from "node:assert/strict";
import { chmodSync, copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { PLATFORMS, binaryName, packageName } from "../launcher/platforms.js";

// Linux binaries are built on ubuntu-22.04 runners, which sets the glibc floor. macOS and
// Windows binaries are unsigned; npm sets no quarantine flag, so they run when installed through npm.
const REQUIRES = {
  linux: "glibc 2.35 or later",
  darwin: "unsigned, for installs through npm",
  win32: "unsigned, for installs through npm",
};
const BINARIES = ["spec", "specfold"];

const npmRoot = resolve(import.meta.dirname, "..");
const main = JSON.parse(readFileSync(join(npmRoot, "package.json"), "utf8"));

// Writes <out>/<os>-<cpu>/ with package.json, README, licenses and bin/, from binaries in binDir.
export function writePlatformPackage(target, binDir, out, pkg = main) {
  const platform = PLATFORMS[target];
  assert.ok(platform, `Unknown target ${target}`);
  const directory = join(out, `${platform.os}-${platform.cpu}`);
  rmSync(directory, { recursive: true, force: true });
  mkdirSync(join(directory, "bin"), { recursive: true });
  const binaries = BINARIES.map((name) => binaryName(name, platform.os));
  for (const name of binaries) {
    const source = join(binDir, name);
    assert.ok(existsSync(source), `Missing ${source}`);
    copyFileSync(source, join(directory, "bin", name));
    chmodSync(join(directory, "bin", name), 0o755);
  }
  for (const license of ["LICENSE-MIT", "LICENSE-APACHE"]) {
    const source = join(binDir, license);
    if (existsSync(source)) copyFileSync(source, join(directory, license));
  }
  const requires = REQUIRES[platform.os];
  const manifest = {
    name: packageName(platform),
    version: pkg.version,
    description: `Native binaries of ${pkg.name} for ${platform.os}-${platform.cpu} (${requires})`,
    license: pkg.license,
    repository: pkg.repository,
    os: [platform.os],
    cpu: [platform.cpu],
    ...(platform.os === "linux" ? { libc: ["glibc"] } : {}),
    files: ["bin", "README.md", "LICENSE-MIT", "LICENSE-APACHE"],
    preferUnplugged: true,
  };
  writeFileSync(join(directory, "package.json"), `${JSON.stringify(manifest, null, 2)}\n`);
  writeFileSync(
    join(directory, "README.md"),
    `# ${manifest.name}\n\n${binaries.join(", ")} for ${pkg.name} on ${platform.os}-${platform.cpu}, ${requires}. Install ${pkg.name}; npm selects this package for you.\n`,
  );
  return { directory, manifest, binaries };
}

// The optionalDependencies block the main package must carry for this version.
export function optionalDependencies(pkg = main) {
  return Object.fromEntries(Object.values(PLATFORMS).map((p) => [packageName(p), pkg.version]));
}

if (import.meta.url === `file://${process.argv[1]}`) {
  const args = process.argv.slice(2);
  if (args[0] === "--optional-dependencies") {
    console.log(JSON.stringify(optionalDependencies(), null, 2));
    process.exit(0);
  }
  const option = (name) => {
    const index = args.indexOf(name);
    assert.ok(index >= 0 && args[index + 1], `Missing ${name}`);
    return args[index + 1];
  };
  const written = writePlatformPackage(option("--target"), resolve(option("--bin-dir")), resolve(option("--out")));
  console.log(JSON.stringify({ directory: written.directory, name: written.manifest.name, binaries: written.binaries }));
}
