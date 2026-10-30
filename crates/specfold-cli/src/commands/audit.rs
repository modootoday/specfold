//! `spec audit`: picks diagnose, pack or record.

mod diagnose;
mod dispatch;
mod pack;
mod record;

pub use dispatch::run;
