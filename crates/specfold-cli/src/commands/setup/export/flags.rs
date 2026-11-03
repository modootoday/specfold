//! The flags a next command repeats so it does what the user asked.

use crate::cli::args::setup::ExportArgs;

/// C166: the flags the user gave that shape the run (preview, output, content), so a next
/// command from a refused preview stays a preview. `--json` is left out, as in every next.
pub(super) fn kept_flags(a: &ExportArgs) -> String {
    let dry = if a.dry_run { " --dry-run" } else { "" };
    format!("{}{dry}", content_flags(a))
}

/// The flags that change what an export writes (C177); never --dry-run, and never --no-guard,
/// which the user types themselves.
pub(super) fn content_flags(a: &ExportArgs) -> String {
    let mut f = String::new();
    for (on, flag) in [(a.local, " --local"), (a.banner, " --banner")] {
        if on {
            f.push_str(flag);
        }
    }
    if let Some(x) = &a.fail_on {
        f.push_str(&format!(" {}", specfold_core::shell::opt("--fail-on", x)));
    }
    f
}
