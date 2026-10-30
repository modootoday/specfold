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
    // C150, C151: both pipes are drained as the plugin writes (a full stderr pipe would stall
    // it), and neither is waited on past the deadline (a background child may keep them open).
    let pipe = |r: Option<Box<dyn Read + Send>>| {
        let (tx, rx) = std::sync::mpsc::channel::<Vec<u8>>();
        if let Some(mut r) = r {
            std::thread::spawn(move || {
                let mut buf = [0u8; 8192];
                while let Ok(n) = r.read(&mut buf) {
                    if n == 0 || tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
            });
        }
        rx
    };
    let out_rx = pipe(child.stdout.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let err_rx = pipe(child.stderr.take().map(|r| Box::new(r) as Box<dyn Read + Send>));
    let (mut out, mut err) = (vec![], vec![]);
    let drain = |rx: &std::sync::mpsc::Receiver<Vec<u8>>, buf: &mut Vec<u8>, keep: usize| {
        while let Ok(chunk) = rx.try_recv() {
            buf.extend(chunk);
            if keep > 0 && buf.len() > keep {
                buf.drain(..buf.len() - keep);
            }
        }
    };
    let deadline = Instant::now() + Duration::from_millis(p.timeout_ms);
    let status = loop {
        drain(&out_rx, &mut out, 0);
        drain(&err_rx, &mut err, 8192);
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
    // The plugin has exited; take what is left of its output until the pipes close or the deadline.
    for (rx, buf, keep) in [(&out_rx, &mut out, 0), (&err_rx, &mut err, 8192)] {
        loop {
            let left = deadline.saturating_duration_since(Instant::now());
            match rx.recv_timeout(left.min(Duration::from_millis(100)).max(Duration::from_millis(1))) {
                Ok(chunk) => {
                    buf.extend(chunk);
                    if keep > 0 && buf.len() > keep {
                        buf.drain(..buf.len() - keep);
                    }
                }
                Err(_) => break,
            }
            if Instant::now() > deadline {
                break;
            }
        }
    }
    if !status.success() {
        let err = String::from_utf8_lossy(&err);
        bail!("plugin {} failed on {method}: {}", p.name, err.lines().last().unwrap_or("exit status non-zero"));
    }
    serde_json::from_slice(&out).with_context(|| format!("plugin {} answered {method} with invalid JSON", p.name))
}
