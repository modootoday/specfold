//! Notes on what an imported body still repeats and on a slug that lost its words.

use super::names::strip_number;
use crate::import::body_meta;
use crate::import::map::ImportMap;
use crate::model::DocId;

pub(super) fn body_notes(
    map: &ImportMap,
    body: &str,
    generated: bool,
    heading: Option<&str>,
    named: &str,
    slug: &str,
    id: &DocId,
) -> Vec<String> {
    let mut notes = vec![];
    // C118: say what the body still repeats, rather than rewrite prose the user wrote.
    if !generated {
        let kept = body_meta::copied_bullets(body);
        if !kept.is_empty() {
            notes.push(format!(
                "the body keeps {} next to the same values in frontmatter; delete those lines so they cannot drift",
                kept.iter().map(|k| format!("'{k}:'")).collect::<Vec<_>>().join(", ")
            ));
        }
        if let Some(h) = heading
            && map.strip_title_number
            && strip_number(h) != h
        {
            notes.push(format!("the title drops the number, but the body's heading keeps '# {h}'"));
        }
    }
    // As with `spec new`: a mostly non-ASCII name loses its words from the slug.
    let alnum = named.chars().filter(|c| c.is_alphanumeric() && !c.is_ascii_digit()).count();
    let ascii = named.chars().filter(|c| c.is_ascii_alphabetic()).count();
    if alnum > 0 && ascii * 2 < alnum {
        notes.push(format!("the name is mostly non-ASCII, so the slug is '{slug}' (the title stays); to rename: spec mv {} --slug <slug> --write", crate::shell::word(id)));
    }
    notes
}
