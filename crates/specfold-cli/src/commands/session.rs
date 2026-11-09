//! Loads configuration, corpus and graph once per command, only as far as the command needs, and
//! resolves what the command line names against them.

mod input;
mod load;
mod location;
mod resolve;
mod warnings;

pub use input::{LINE_BREAKS, check_date, known_statuses, known_types};
pub use load::{Session, config, config_quiet, discover, example_doc_for, location, open, open_for, open_tracked};
pub use location::Location;
pub use resolve::{example_doc, resolve};
pub use warnings::{config_warnings, hold_config_warnings, show_config_warnings};
