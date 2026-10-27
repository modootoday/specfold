// The Rust targets the release builds, and the npm package that carries each one's binaries.
export const PLATFORMS = {
  "x86_64-unknown-linux-gnu": { os: "linux", cpu: "x64" },
  "aarch64-unknown-linux-gnu": { os: "linux", cpu: "arm64" },
  "x86_64-apple-darwin": { os: "darwin", cpu: "x64" },
  "aarch64-apple-darwin": { os: "darwin", cpu: "arm64" },
  "x86_64-pc-windows-msvc": { os: "win32", cpu: "x64" },
};

export const MAIN = "@modootoday/specfold";

export function packageName({ os, cpu }) {
  return `${MAIN}-${os}-${cpu}`;
}

export function binaryName(name, os) {
  if (os === "win32") return `${name}.exe`;
  return name;
}

export function hostPlatform(os = process.platform, cpu = process.arch) {
  return Object.values(PLATFORMS).find((p) => p.os === os && p.cpu === cpu);
}
