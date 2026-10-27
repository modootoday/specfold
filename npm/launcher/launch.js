import { spawnSync } from "node:child_process";
import { createRequire } from "node:module";
import { binaryName, hostPlatform, packageName } from "./platforms.js";

const require = createRequire(import.meta.url);

// The platform package is an optional dependency, so npm installs only the one matching this
// host; a missing one means an unsupported host or an install that skipped optional packages.
export function binaryPath(name, os = process.platform, cpu = process.arch) {
  const platform = hostPlatform(os, cpu);
  if (!platform) throw new Error(`specfold has no binary for ${os}-${cpu}`);
  const pkg = packageName(platform);
  try {
    return require.resolve(`${pkg}/bin/${binaryName(name, os)}`);
  } catch {
    throw new Error(`${pkg} is not installed; reinstall without --omit=optional or --no-optional`);
  }
}

export function launch(name) {
  let bin;
  try {
    bin = binaryPath(name);
  } catch (error) {
    console.error(`specfold: ${error.message}`);
    process.exit(127);
  }
  const res = spawnSync(bin, process.argv.slice(2), { stdio: "inherit" });
  if (res.error) {
    console.error(`specfold: cannot run ${bin}: ${res.error.message}`);
    process.exit(127);
  }
  if (res.signal) process.kill(process.pid, res.signal);
  process.exit(res.status ?? 1);
}
