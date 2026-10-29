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
