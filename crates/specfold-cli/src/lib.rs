//! The `spec` command line. Both binaries (`specfold` and `spec`) call [`main`].

// Library code only: tests may panic (AGENTS.md).
#![deny(clippy::panic)]
// CLI-20: no unwrap outside tests (clippy.toml allow-unwrap-in-tests).
#![deny(clippy::unwrap_used)]
#![deny(clippy::too_many_lines)]
#![deny(clippy::cognitive_complexity)]

pub mod cli;
pub mod commands;
pub mod lsp;
pub mod mcp;
pub mod output;

pub use cli::execute;
pub use commands::session;
use output::render;
use std::io::Write;
use std::process::ExitCode;

pub fn main() -> ExitCode {
    let ex = match execute::utf8_args(std::env::args_os().collect()) {
        Ok(argv) => execute::execute(argv),
        Err(ex) => *ex,
    };
    if ex.serve_mcp {
        return match mcp::server::serve() {
            Ok(()) => ExitCode::SUCCESS,
            Err(_) => ExitCode::from(3),
        };
    }
    let mut stdout = std::io::stdout();
    let text = match ex.raw {
        Some(raw) => raw,
        None => render(&ex.output, ex.format, &ex.page),
    };
    let _ = stdout.write_all(text.as_bytes());
    ExitCode::from(ex.output.code as u8)
}
