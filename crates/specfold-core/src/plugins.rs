//! Plugins: external programs that add rules, importers, exporters, audits and event handlers
//! over a JSON protocol. Only programs named in spec.toml ever run.

pub mod payload;
pub mod process;
pub mod protocol;
pub mod registry;
pub mod rules;

pub use protocol::{Description, PROTOCOL};
pub use registry::{Loaded, load};

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::config::{Config, PluginDef, RawConfig};

    #[test]
    fn describes_and_times_out() {
        let tmp = tempfile::tempdir().unwrap();
        let r = tmp.path();
        let script = r.join("plugin.sh");
        std::fs::write(&script, "#!/bin/sh\nread req\ncase \"$req\" in\n*describe*) echo '{\"protocol\":\"specfold.plugin.v1\",\"name\":\"demo\",\"provides\":{\"rules\":[{\"id\":\"r1\",\"title\":\"t\"}]}}';;\n*) sleep 5;;\nesac\n").unwrap();
        let raw = RawConfig {
            plugins: vec![PluginDef {
                name: "demo".into(),
                cmd: vec!["sh".into(), script.to_string_lossy().into_owned()],
                timeout_ms: 300,
            }],
            ..Default::default()
        };
        let cfg = Config::from_raw(r.to_path_buf(), raw, false);
        let loaded = load(&cfg);
        let d = loaded[0].description.as_ref().unwrap();
        assert_eq!(d.provides.rules[0].id, "r1");
        let slow: anyhow::Result<serde_json::Value> =
            process::call(&cfg, &cfg.raw.plugins[0], "rule.run", serde_json::Value::Null);
        assert!(slow.unwrap_err().to_string().contains("did not answer"));
    }
}
