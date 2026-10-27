//! Phase timings on stderr when `SPECFOLD_TRACE` is set, for diagnosing slow runs.

use std::sync::OnceLock;
use std::time::Instant;

fn enabled() -> bool {
    static ON: OnceLock<bool> = OnceLock::new();
    *ON.get_or_init(|| std::env::var_os("SPECFOLD_TRACE").is_some())
}

/// Prints how long a phase took since `start` and returns a new start.
pub fn phase(name: &str, start: Instant) -> Instant {
    if enabled() {
        eprintln!("specfold: {name:<12} {:>7.1} ms", start.elapsed().as_secs_f64() * 1000.0);
    }
    Instant::now()
}
