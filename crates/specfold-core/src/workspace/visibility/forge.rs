//! The hosting service asked for each submodule's visibility, behind a trait so tests answer
//! without a network.

use std::fmt;

/// Why the forge gave no visibility.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ForgeError {
    /// The URL names no repository on this forge.
    Unsupported(String),
    /// The forge cannot be reached: no network, no client, or not signed in.
    Unreachable(String),
    /// The forge answered but not with a visibility (no such repository, no access).
    Failed(String),
}

impl fmt::Display for ForgeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ForgeError::Unsupported(m) | ForgeError::Unreachable(m) | ForgeError::Failed(m) => f.write_str(m),
        }
    }
}

/// A hosting service that knows whether a repository is public.
pub trait Forge {
    /// The service's name, for messages.
    fn name(&self) -> &str;
    /// The repository a clone URL names on this forge, or Unsupported.
    fn repository(&self, url: &str) -> Result<String, ForgeError>;
    /// The visibility of a repository `repository` returned, as the forge spells it.
    fn visibility(&self, repository: &str) -> Result<String, ForgeError>;
}
