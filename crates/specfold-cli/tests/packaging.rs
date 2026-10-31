//! Release packaging metadata agrees with what the release workflow builds.

use std::path::Path;

fn read(rel: &str) -> String {
    std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").join(rel)).unwrap()
}

#[test]
fn binstall_metadata_matches_release_archives() {
    let manifest = read("crates/specfold-cli/Cargo.toml");
    let workflow = read(".github/workflows/release.yml");
    // Archive names and layout the workflow produces.
    assert!(workflow.contains(r#"name="specfold-${{ matrix.target }}""#));
    assert!(workflow.contains(r#"tar -czf "$name.tar.gz" "$name""#));
    assert!(workflow.contains(r#"7z a -tzip "$name.zip" "$name""#));
    assert!(workflow.contains(
        r#"for bin in specfold spec; do cp "target/${{ matrix.target }}/release/$bin$ext" "dist/$name/"; done"#
    ));
    assert!(workflow.contains(r#"tag="v$version""#));
    // The binstall templates name the same files.
    for line in [
        r#"pkg-url = "{ repo }/releases/download/v{ version }/specfold-{ target }.tar.gz""#,
        r#"bin-dir = "specfold-{ target }/{ bin }{ binary-ext }""#,
        r#"pkg-url = "{ repo }/releases/download/v{ version }/specfold-{ target }.zip""#,
    ] {
        assert!(manifest.contains(line), "{line}");
    }
    // Every Windows target in the matrix has the zip override.
    for target in workflow.lines().filter_map(|l| l.split("target: ").nth(1)).map(|t| t.trim_end_matches(" }").trim()) {
        if target.contains("windows") {
            assert!(manifest.contains(&format!("[package.metadata.binstall.overrides.{target}]")), "{target}");
        }
    }
}

/// The `run` script of the workflow's "Release notes" step, dedented.
fn notes_step() -> String {
    let workflow = read(".github/workflows/release.yml");
    let at = workflow.find("- name: Release notes").expect("release notes step");
    let body = &workflow[at..];
    let run = body.find("run: |").expect("run block") + "run: |\n".len();
    body[run..]
        .lines()
        .take_while(|l| l.starts_with("          ") || l.is_empty())
        .map(|l| l.get(10..).unwrap_or(""))
        .collect::<Vec<_>>()
        .join("\n")
}

fn run_notes(dir: &Path, version: &str) -> String {
    let summary = dir.join("summary.md");
    let ok = std::process::Command::new("bash")
        .args(["-euc", &notes_step()])
        .current_dir(dir)
        .env("VERSION", version)
        .env("GITHUB_STEP_SUMMARY", &summary)
        .status()
        .unwrap()
        .success();
    assert!(ok);
    std::fs::read_to_string(dir.join("notes.md")).unwrap()
}

#[test]
fn release_notes_use_the_changelog_section_then_commit_subjects() {
    let t = tempfile::tempdir().unwrap();
    let git = |args: &[&str]| {
        let ok = std::process::Command::new("git")
            .args(["-c", "user.name=t", "-c", "user.email=t@t", "-c", "commit.gpgsign=false"])
            .args(args)
            .current_dir(t.path())
            .output()
            .unwrap()
            .status
            .success();
        assert!(ok, "git {args:?}");
    };
    git(&["init", "-q"]);
    std::fs::write(
        t.path().join("CHANGELOG.md"),
        "# Changelog\n\n## Unreleased\n\n- next\n\n## 0.4.0\n\n- Added four.\n- Fixed it.\n\n## 0.3.0\n\n- Three.\n",
    )
    .unwrap();
    git(&["add", "."]);
    git(&["commit", "-qm", "first change"]);
    git(&["tag", "v0.3.0"]);
    git(&["commit", "-q", "--allow-empty", "-m", "second change"]);
    assert_eq!(run_notes(t.path(), "0.4.0"), "- Added four.\n- Fixed it.\n\n");
    assert_eq!(run_notes(t.path(), "0.3.0"), "- Three.\n");
    // No section for the version: the commit subjects since the previous tag.
    assert_eq!(run_notes(t.path(), "0.5.0"), "- second change\n");
}
