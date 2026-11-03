//! Configuration warnings: shown on standard error once, and only when a command asks for them.

use specfold_core::Config;

/// Configuration that is valid but never does what it says: unknown types and statuses.
pub fn config_warnings(cfg: &Config) -> Vec<String> {
    let mut w = specfold_core::model::registry::suspect_overrides(&cfg.raw.types);
    w.extend(specfold_core::model::registry::suspect_coverage(&cfg.raw.types, &cfg.raw.audit.coverage_types));
    w.extend(shell_problem(cfg.raw.verify.shell.as_deref()));
    w
}

/// C237: a `[verify] shell` that is empty or not on PATH makes every verify command fail.
fn shell_problem(shell: Option<&str>) -> Option<String> {
    let s = shell?;
    if s.trim().is_empty() {
        return Some("[verify] shell is empty, so no verify command can run".into());
    }
    let found = if s.contains('/') {
        std::path::Path::new(s).is_file()
    } else {
        std::env::var_os("PATH").is_some_and(|p| std::env::split_paths(&p).any(|d| d.join(s).is_file()))
    };
    (!found).then(|| format!("[verify] shell \"{s}\" is not on PATH, so no verify command can run"))
}

/// Set once the config warnings were shown, or while no command asked for them.
static WARNED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

/// Shows the config warnings with the next configuration loaded (`spec check`).
pub fn show_config_warnings() {
    WARNED.store(false, std::sync::atomic::Ordering::Relaxed);
}

/// Keeps them back: the command reports them itself (doctor).
pub fn hold_config_warnings() {
    WARNED.store(true, std::sync::atomic::Ordering::Relaxed);
}

/// Prints the config warnings for `cfg` when a command asked for them and they were not shown yet.
pub(super) fn show_once(cfg: &Config) {
    if !WARNED.swap(true, std::sync::atomic::Ordering::Relaxed) {
        for w in config_warnings(cfg) {
            eprintln!("warning: spec.toml: {w}");
        }
    }
}
