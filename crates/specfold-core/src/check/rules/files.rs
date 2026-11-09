//! Rules over what files hold and where they may go: attachments (P2, P3, E1, E2), agent assets
//! (A1-A4), invisible characters (H1), secrets and guards (U1, U2), visibility (V1-V3), and
//! size budgets and generated files (B1, G2).

mod asset_refs;
mod assets;
mod attachments;
mod guarded;
mod hidden;
mod limits;
mod machine_paths;
mod secrets;
mod visibility;

pub use asset_refs::AssetRefsExist;
pub use assets::{AssetFrontmatter, AssetNamesUnique};
pub use attachments::{HashesMatch, Registered, SizeLimit, StoredExist};
pub use guarded::GuardsPass;
pub use hidden::HiddenChars;
pub use limits::{GeneratedManifest, SizeBudgets};
pub use machine_paths::NoMachinePaths;
pub use secrets::NoSecrets;
pub use visibility::{ExternalCites, NoLocalCites, NotInPublic};
