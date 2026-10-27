//! The spec.toml `spec init` writes for a new project.

pub struct StarterKind {
    pub pattern: String,
    pub id: String,
    pub manifest: bool,
}

pub fn render(root_owner: &str, kinds: &[StarterKind]) -> String {
    let mut s = String::new();
    s.push_str("# specfold configuration. Run `specfold explain config` for every key.\n");
    s.push_str("[spec]\nschema = 1\n\n[owners]\n");
    s.push_str(&format!("root = \"{root_owner}\"\n"));
    for k in kinds {
        s.push_str(&format!(
            "\n[[owners.kinds]]\npattern = \"{}\"\nid = \"{}\"\nmanifest = {}\n",
            k.pattern, k.id, k.manifest
        ));
    }
    s
}
