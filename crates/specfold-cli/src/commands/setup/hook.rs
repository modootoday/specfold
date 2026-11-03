//! `spec hook print`: snippets for the hook manager you use. `spec hook install`: the git
//! pre-push hook, written only with --write and never over another tool's hook.

use crate::cli::args::{global::Global, setup::HookArgs};
use crate::output::{Code, Output};
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

/// The pre-push hook `spec hook install` writes: the push check, failing only on the owners the
/// push touches (D1).
pub(crate) fn pre_push() -> String {
    format!(
        "#!/bin/sh\n# installed by spec hook install\n{}\n",
        PUSH.replace("spec check --changed", "spec check --owners --changed")
    )
}

fn install(g: &Global, a: &HookArgs) -> Result<Output> {
    let dir = crate::commands::session::location(g).start()?;
    let out = std::process::Command::new("git")
        .arg("-C")
        .arg(&dir)
        .args(["rev-parse", "--git-path", "hooks/pre-push"])
        .output()?;
    if !out.status.success() {
        bail!("not inside a git repository; next: git init");
    }
    let path = dir.join(String::from_utf8_lossy(&out.stdout).trim());
    let text = pre_push();
    let shown = path.display().to_string();
    let o = Output::new("hook").unpaged().field("file", shown.clone()).field("content", text.clone());
    match std::fs::read_to_string(&path) {
        Ok(old) if old == text => {
            return Ok(o.field("written", false).foot(format!("{shown}: already installed")).next("git push"));
        }
        Ok(_) => {
            return Ok(o
                .field("written", false)
                .code(Code::Refused)
                .foot(format!("{shown} exists and is not this hook; left alone; next: spec hook print --for git (add the push line by hand)"))
                .next("spec hook print --for git"));
        }
        Err(_) => {}
    }
    if !a.write {
        return Ok(o.head(format!("# {shown}")).foot(text).preview().next("spec hook install --write"));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, &text)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755))?;
    }
    Ok(o.field("written", true).foot(format!("wrote {shown}")).next("git push"))
}

pub fn run(g: &Global, a: &HookArgs) -> Result<Output> {
    match a.action.as_str() {
        "print" => {}
        "install" => return install(g, a),
        other => bail!("unknown hook action '{other}'; next: spec hook print --for git, or spec hook install"),
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
