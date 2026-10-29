//! The spec.toml `spec init` writes for a new project.

pub struct StarterKind {
    pub pattern: String,
    pub id: String,
    pub manifest: bool,
}

pub fn render(root_owner: &str, kinds: &[StarterKind]) -> String {
    let mut s = String::new();
    s.push_str("# specfold configuration. Run `spec explain config` for every key.\n");
    s.push_str("[spec]\nschema = 1\n\n[owners]\n");
    s.push_str(&format!("root = \"{root_owner}\"\n"));
    for k in kinds {
        s.push_str(&format!(
            "\n[[owners.kinds]]\npattern = \"{}\"\nid = \"{}\"\nmanifest = {}\n",
            k.pattern, k.id, k.manifest
        ));
    }
    s.push_str(concat!(
        "\n# Plain markdown outside documents whose <!-- spec:count --> markers N1 keeps current.\n",
        "# [check]\n",
        "# count_paths = [\"README.md\", \"docs/**/*.md\"]\n",
    ));
    s
}

#[cfg(test)]
mod tests {
    #[test]
    fn the_starter_shows_count_paths_commented_out_and_still_parses() {
        let s = super::render("ws", &[]);
        assert!(s.contains("# count_paths = ["), "{s}");
        let v: toml::Value = toml::from_str(&s).unwrap();
        assert!(v.get("check").is_none());
    }
}
