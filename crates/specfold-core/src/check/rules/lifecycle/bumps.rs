//! D1: a package manifest or lockfile whose change only moves package versions does not ask the
//! documents governing it, unless a document names one of the packages it moves.

mod diff;
mod quiet;

pub(super) use quiet::Quiet;
