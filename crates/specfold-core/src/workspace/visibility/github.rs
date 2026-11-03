//! GitHub as the forge, asked through the `gh` command line client, so specfold itself opens no
//! network connection and uses the user's own sign-in.

use super::forge::{Forge, ForgeError};
use std::process::Command;

/// GitHub through `gh repo view <owner/repo> --json visibility`.
pub struct GitHub;

/// Signs in gh's error output that it never reached GitHub.
const UNREACHABLE: &[&str] = &[
    "error connecting",
    "dial tcp",
    "no such host",
    "could not resolve host",
    "network is unreachable",
    "connection refused",
    "timeout",
    "gh auth login",
    "not logged in",
];

/// `owner/repo` from a github.com clone URL in https, ssh or scp form.
pub fn repository_of(url: &str) -> Option<String> {
    let u = url.trim();
    let rest = if let Some(r) = u.strip_prefix("git@") {
        let (host, path) = r.split_once(':')?;
        host.eq_ignore_ascii_case("github.com").then_some(path)?
    } else {
        let (_, r) = u.split_once("://")?;
        let (authority, path) = r.split_once('/')?;
        let host = authority.rsplit('@').next()?.split(':').next()?;
        let github = host.eq_ignore_ascii_case("github.com") || host.eq_ignore_ascii_case("www.github.com");
        github.then_some(path)?
    };
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let mut parts = rest.split('/');
    let (owner, repo) = (parts.next()?, parts.next()?);
    let fine = |s: &str| !s.is_empty() && s != "." && s != "..";
    (fine(owner) && fine(repo) && parts.next().is_none()).then(|| format!("{owner}/{repo}"))
}

impl Forge for GitHub {
    fn name(&self) -> &str {
        "GitHub"
    }

    fn repository(&self, url: &str) -> Result<String, ForgeError> {
        repository_of(url).ok_or_else(|| {
            ForgeError::Unsupported(format!(
                "{url} is not a github.com repository URL; only GitHub visibility can be measured"
            ))
        })
    }

    fn visibility(&self, repository: &str) -> Result<String, ForgeError> {
        let out = Command::new("gh")
            .args(["repo", "view", repository, "--json", "visibility"])
            .env("GH_PROMPT_DISABLED", "1")
            .env("NO_COLOR", "1")
            .output()
            .map_err(|e| ForgeError::Unreachable(format!("cannot run gh, the GitHub CLI ({e})")))?;
        let err = String::from_utf8_lossy(&out.stderr);
        let first = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("no message").to_string();
        if !out.status.success() {
            let lower = err.to_lowercase();
            return Err(if UNREACHABLE.iter().any(|s| lower.contains(s)) {
                ForgeError::Unreachable(format!("gh cannot reach GitHub: {first}"))
            } else {
                ForgeError::Failed(format!("gh repo view {repository}: {first}"))
            });
        }
        let v: serde_json::Value = serde_json::from_slice(&out.stdout)
            .map_err(|_| ForgeError::Failed(format!("gh repo view {repository} printed no JSON")))?;
        v.get("visibility")
            .and_then(|s| s.as_str())
            .filter(|s| !s.is_empty())
            .map(str::to_string)
            .ok_or_else(|| ForgeError::Failed(format!("gh repo view {repository} gave no visibility")))
    }
}

#[cfg(test)]
mod tests {
    use super::repository_of;

    #[test]
    fn repository_of_reads_https_ssh_and_scp_urls() {
        let r = |u: &str| repository_of(u);
        assert_eq!(r("https://github.com/acme/shop.git").as_deref(), Some("acme/shop"));
        assert_eq!(r("https://bot@github.com/acme/shop/").as_deref(), Some("acme/shop"));
        assert_eq!(r("git@github.com:acme/shop.git").as_deref(), Some("acme/shop"));
        assert_eq!(r("ssh://git@github.com:22/acme/shop").as_deref(), Some("acme/shop"));
        assert_eq!(r("https://gitlab.com/acme/shop.git"), None);
        assert_eq!(r("../shop.git"), None);
        assert_eq!(r("https://github.com/acme"), None);
    }
}
