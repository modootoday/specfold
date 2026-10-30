//! `spec hook print`: snippets for the hook manager you use. specfold never installs hooks.

use crate::cli::args::{global::Global, setup::HookArgs};
use crate::output::Output;
use anyhow::{Result, bail};

/// C126: the push check compares with the upstream, else with where HEAD left the default
/// branch, else says it skipped; a new branch must not block its own first push.
const PUSH: &str = "sh -c 'b=$(git rev-parse -q --verify @{upstream} || git merge-base HEAD \"$(git symbolic-ref -q --short refs/remotes/origin/HEAD)\" 2>/dev/null) && exec spec check --changed \"$b\"; echo \"spec check skipped (no upstream or default branch to compare with)\" >&2'";

fn snippet(target: &str) -> Option<(String, String)> {
    Some(match target {
        "git" => (
            format!(".git/hooks/pre-commit (and .git/hooks/pre-push with: {PUSH})"),
            "#!/bin/sh\nexec spec check --staged\n".into(),
        ),
        "pre-commit" => (
            ".pre-commit-config.yaml".into(),
            format!(
                "# language: unsupported needs pre-commit 4.4.0 or later; older versions call it system.\n- repo: local\n  hooks:\n    - id: spec-check\n      name: spec check\n      entry: spec check --staged\n      language: unsupported\n      pass_filenames: false\n    - id: spec-check-push\n      name: spec check (push)\n      entry: {PUSH}\n      language: unsupported\n      pass_filenames: false\n      stages: [pre-push]\n"
            ),
        ),
        "lefthook" => (
            "lefthook.yml".into(),
            format!(
                "pre-commit:\n  commands:\n    spec:\n      run: spec check --staged\npre-push:\n  commands:\n    spec:\n      run: {PUSH}\n"
            ),
        ),
        "husky" => (".husky/pre-commit".into(), "spec check --staged\n".into()),
        _ => return None,
    })
}

pub fn run(_g: &Global, a: &HookArgs) -> Result<Output> {
    if a.action != "print" {
        bail!("only `spec hook print` exists; specfold does not install hooks itself");
    }
    let Some((file, text)) = snippet(&a.target) else {
        bail!("unknown hook manager '{}' (git, pre-commit, lefthook, husky)", a.target)
    };
    Ok(Output::new("hook")
        .unpaged()
        .field("file", file.clone())
        .field("content", text.clone())
        .head(format!("# {file}"))
        .foot(text)
        .next("spec agents"))
}
