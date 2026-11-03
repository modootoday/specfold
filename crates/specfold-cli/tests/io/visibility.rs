//! `spec sync visibility` end to end, with a stand-in `gh` on PATH so no test reaches GitHub.
#![cfg(unix)]

#[path = "../common/mod.rs"]
mod common;

use serde_json::Value;
use std::os::unix::fs::PermissionsExt;

/// A fake gh: acme/blog is public, anything else private; FAKE_GH_OFFLINE fails as gh does
/// without a network.
const FAKE_GH: &str = "#!/bin/sh\nif [ -n \"$FAKE_GH_OFFLINE\" ]; then echo 'error connecting to api.github.com' >&2; exit 1; fi\nif [ \"$3\" = acme/blog ]; then echo '{\"visibility\":\"PUBLIC\"}'; else echo '{\"visibility\":\"PRIVATE\"}'; fi\n";

fn setup() -> (common::Repo, String) {
    let r = common::repo();
    r.write(".gitmodules", "[submodule \"blog\"]\n\tpath = services/blog\n\turl = https://github.com/acme/blog.git\n");
    let toml = r.read("spec.toml").replace("[public]\n", "[public]\nsubmodules_manifest = \"vis.json\"\n");
    r.write("spec.toml", &toml);
    let bin = r.home.path().join("bin");
    std::fs::create_dir_all(&bin).unwrap();
    let gh = bin.join("gh");
    std::fs::write(&gh, FAKE_GH).unwrap();
    std::fs::set_permissions(&gh, std::fs::Permissions::from_mode(0o755)).unwrap();
    let path = format!("{}:{}", bin.display(), std::env::var("PATH").unwrap_or_default());
    (r, path)
}

fn sync(r: &common::Repo, path: &str, args: &[&str], offline: bool) -> (i32, Value) {
    let mut c = r.spec();
    c.env("PATH", path).args(["sync", "visibility"]).args(args).arg("--json");
    if offline {
        c.env("FAKE_GH_OFFLINE", "1");
    }
    let out = c.output().unwrap();
    let v = serde_json::from_slice(&out.stdout).unwrap();
    (out.status.code().unwrap_or(-1), v)
}

#[test]
fn sync_visibility_previews_writes_and_then_has_nothing_to_do() {
    let (r, path) = setup();
    let (code, v) = r.json(&["check", "--rule", "V1"]);
    assert_eq!(code, 1, "{v}");
    assert!(v.to_string().contains("next: spec sync visibility --write"), "{v}");
    let (code, v) = sync(&r, &path, &[], false);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], false);
    assert!(!r.path().join("vis.json").exists());
    let (code, v) = sync(&r, &path, &["--write"], false);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["written"], true, "{v}");
    assert_eq!(v["submodules"][0]["visibility"], "PUBLIC", "{v}");
    let written = r.read("vis.json");
    assert_eq!(r.json(&["check", "--rule", "V1"]).0, 0);
    let (code, v) = sync(&r, &path, &["--write"], false);
    assert_eq!(code, 0, "{v}");
    assert_eq!(v["changed"], false, "{v}");
    assert_eq!(r.read("vis.json"), written);
}

#[test]
fn sync_visibility_offline_writes_nothing() {
    let (r, path) = setup();
    let (code, v) = sync(&r, &path, &["--write"], true);
    assert_eq!(code, 3, "{v}");
    assert!(v["error"].as_str().unwrap().contains("offline"), "{v}");
    assert!(!r.path().join("vis.json").exists());
}

#[test]
fn sync_visibility_refuses_a_url_off_github() {
    let (r, path) = setup();
    r.write(
        ".gitmodules",
        "[submodule \"blog\"]\n\tpath = services/blog\n\turl = https://git.example.com/acme/blog.git\n",
    );
    let (code, v) = sync(&r, &path, &["--write"], false);
    assert_eq!(code, 4, "{v}");
    assert!(v["error"].as_str().unwrap().contains("not a github.com repository URL"), "{v}");
    assert!(!r.path().join("vis.json").exists());
}
