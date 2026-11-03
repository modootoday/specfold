//! Runs the shell commands documents name (`verify`, `generated_by`): with a deadline, in
//! parallel, each distinct command once, and with a missing program reported as its own verdict.

mod pool;
mod shell;
mod tools;

pub use pool::{Job, Settings, run};
pub use shell::{Outcome, Verdict};
pub use tools::missing_programs;
