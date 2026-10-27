//! Parses YAML frontmatter into JSON values, so the rest of the code handles one value model.

use serde_json::{Map, Value};
use yaml_rust2::{Yaml, YamlLoader};

#[derive(Debug)]
pub enum Parsed {
    None,
    Map(Map<String, Value>),
    Invalid(String),
}

pub fn parse(raw: Option<&str>) -> Parsed {
    let Some(raw) = raw else { return Parsed::None };
    if raw.trim().is_empty() {
        return Parsed::Map(Map::new());
    }
    match YamlLoader::load_from_str(raw) {
        Ok(docs) => match docs.into_iter().next() {
            Some(y @ Yaml::Hash(_)) => match to_json(&y) {
                Value::Object(m) => Parsed::Map(m),
                _ => Parsed::Invalid("frontmatter is not a mapping".into()),
            },
            Some(Yaml::Null) | None => Parsed::Map(Map::new()),
            Some(_) => Parsed::Invalid("frontmatter is not a mapping".into()),
        },
        Err(e) => Parsed::Invalid(e.to_string()),
    }
}

pub fn to_json(y: &Yaml) -> Value {
    match y {
        Yaml::Real(s) => s
            .parse::<f64>()
            .ok()
            .and_then(serde_json::Number::from_f64)
            .map(Value::Number)
            .unwrap_or_else(|| Value::String(s.clone())),
        Yaml::Integer(i) => Value::Number((*i).into()),
        Yaml::String(s) => Value::String(s.clone()),
        Yaml::Boolean(b) => Value::Bool(*b),
        Yaml::Array(a) => Value::Array(a.iter().map(to_json).collect()),
        Yaml::Hash(h) => Value::Object(
            h.iter()
                .filter_map(|(k, v)| {
                    let key = match k {
                        Yaml::String(s) | Yaml::Real(s) => s.clone(),
                        Yaml::Integer(i) => i.to_string(),
                        Yaml::Boolean(b) => b.to_string(),
                        _ => return None,
                    };
                    Some((key, to_json(v)))
                })
                .collect(),
        ),
        Yaml::Null | Yaml::BadValue | Yaml::Alias(_) => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nested_and_invalid() {
        let Parsed::Map(m) = parse(Some("id: a\nrel:\n  governs: [\"apps/shop/**\"]\n")) else { panic!() };
        assert_eq!(m["rel"]["governs"][0], "apps/shop/**");
        assert!(matches!(parse(Some("a: [unclosed\n")), Parsed::Invalid(_)));
        assert!(matches!(parse(Some("- a\n- b\n")), Parsed::Invalid(_)));
    }
}
