//! T rules and the rules that keep documents current with what they describe: supersede pairs,
//! forward-only statuses, sealed bodies, moving `updated` dates, reviewed exceptions (T1-T5),
//! governed-code drift (D1), verify commands and generators (X1, X2, G1), translations (R1, R2)
//! and counted numbers (N1).

mod changes;
mod commands;
mod counts;
mod drift;
mod exceptions;
mod supersede;
mod translations;

pub use changes::{ForwardOnly, Sealed, UpdatedMoves};
pub use commands::{GeneratedInSync, VerifyDeclared, VerifyPasses};
pub use counts::CountsMatch;
pub use drift::GovernedCodeDrift;
pub use exceptions::{ExceptionsReviewed, is_date, overdue_exceptions};
pub use supersede::SupersedePairs;
pub use translations::{TranslationsCurrent, TranslationsPlaced};
