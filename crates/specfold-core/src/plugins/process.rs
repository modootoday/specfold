//! Runs a plugin process for one request, with a deadline.

use super::protocol::{PROTOCOL, Request};
use crate::config::{Config, PluginDef};
use anyhow::{Context, Result, anyhow, bail};
use serde::de::DeserializeOwned;
use serde_json::Value;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

fn program(cfg: &Config, p: &PluginDef) -> Result<(PathBuf, Vec<String>)> {
    let (prog, args) = p.cmd.split_first().ok_or_else(|| anyhow!("plugin {} has an empty cmd", p.name))?;
    let local = cfg.root.join(prog);
    let resolved = if prog.contains('/') && local.exists() { local } else { PathBuf::from(prog) };
    Ok((resolved, args.to_vec()))
}

pub fn call<T: DeserializeOwned>(cfg: &Config, p: &PluginDef, method: &str, params: Value) -> Result<T> {
    let (prog, args) = program(cfg, p)?;
    let mut child = Command::new(&prog)
        .args(&args)
        .current_dir(&cfg.root)
        .env("SPECFOLD_PROTOCOL", PROTOCOL)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .with_context(|| format!("plugin {}: cannot start {}", p.name, prog.display()))?;
    let body = serde_json::to_vec(&Request { protocol: PROTOCOL, method, params })?;
    let mut stdin = child.stdin.take().context("no stdin")?;
    let writer = std::thread::spawn(move || {
        let _ = stdin.write_all(&body);
    });
    let mut stdout = child.stdout.take().context("no stdout")?;
    let reader = std::thread::spawn(move || {
        let mut buf = vec![];
        let _ = stdout.read_to_end(&mut buf);
        buf
    });
    let deadline = Instant::now() + Duration::from_millis(p.timeout_ms);
    let status = loop {
        if let Some(s) = child.try_wait()? {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            let _ = child.wait();
            bail!("plugin {} did not answer {method} within {} ms", p.name, p.timeout_ms);
        }
        std::thread::sleep(Duration::from_millis(10));
    };
    let _ = writer.join();
    let out = reader.join().map_err(|_| anyhow!("plugin {}: reader failed", p.name))?;
    if !status.success() {
        let mut err = String::new();
        if let Some(mut e) = child.stderr.take() {
            let _ = e.read_to_string(&mut err);
        }
        bail!("plugin {} failed on {method}: {}", p.name, err.lines().last().unwrap_or("exit status non-zero"));
    }
    serde_json::from_slice(&out).with_context(|| format!("plugin {} answered {method} with invalid JSON", p.name))
}
