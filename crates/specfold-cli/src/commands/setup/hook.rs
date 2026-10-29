//! `spec hook print`: snippets for the hook manager you use. specfold never installs hooks.

use crate::cli::args::{global::Global, setup::HookArgs};
use crate::output::Output;
use anyhow::{Result, bail};

fn snippet(target: &str) -> Option<(&'static str, String)> {
    Some(match target {
        "git" => (".git/hooks/pre-commit (and pre-push with --changed @{upstream})", "#!/bin/sh\nexec spec check --staged\n".into()),
        "pre-commit" => (".pre-commit-config.yaml", "# language: unsupported needs pre-commit 4.4.0 or later; older versions call it system.\n- repo: local\n  hooks:\n    - id: spec-check\n      name: spec check\n      entry: spec check --staged\n      language: unsupported\n      pass_filenames: false\n    - id: spec-check-push\n      name: spec check (push)\n      entry: spec check --changed @{upstream}\n      language: unsupported\n      pass_filenames: false\n      stages: [pre-push]\n".into()),
        "lefthook" => ("lefthook.yml", "pre-commit:\n  commands:\n    spec:\n      run: spec check --staged\npre-push:\n  commands:\n    spec:\n      run: spec check --changed @{upstream}\n".into()),
        "husky" => (".husky/pre-commit", "spec check --staged\n".into()),
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
        .field("file", file)
        .field("content", text.clone())
        .head(format!("# {file}"))
        .foot(text)
        .next("spec agents"))
}
