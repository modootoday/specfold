//! How a `key=value` assignment writes one value: a number where the field holds one, else text.

use crate::model::doctype::{INTEGER_FIELDS, TypeDef};
use serde_json::Value;

/// Keys whose values stay text even when they read as a number: ids, dates, commits, languages.
const TEXT_KEYS: &[&str] = &[
    "id",
    "title",
    "status",
    "owner",
    "created",
    "updated",
    "source_rev",
    "verified_at",
    "review_by",
    "generated_by",
    "lang",
];

/// The shape one key's value is written in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Shape {
    /// A field read as a whole number; a value that is not one is refused.
    Integer,
    /// A field whose value is text.
    Text,
    /// A key with no known type: a whole number stays one unless the key already holds text.
    Infer,
}

/// The shape of `key` in a document of type `ty`.
pub fn of(ty: &TypeDef, key: &str) -> Shape {
    if INTEGER_FIELDS.contains(&key) {
        Shape::Integer
    } else if key.contains('.') || TEXT_KEYS.contains(&key) || ty.kinds.contains_key(key) {
        Shape::Text
    } else {
        Shape::Infer
    }
}

/// `text` as the value to write, given what the key holds now.
pub fn value(shape: Shape, text: String, existing: Option<&Value>) -> Value {
    // Only the integer's own spelling counts, so 007 or +5 stay as typed.
    let whole = text.parse::<i64>().ok().filter(|n| n.to_string() == text);
    match (shape, whole) {
        (Shape::Integer, _) => text.parse::<u64>().map_or(Value::String(text), Value::from),
        (Shape::Infer, Some(n)) if existing.is_none_or(Value::is_number) => Value::from(n),
        _ => Value::String(text),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numbers_stay_numbers_and_text_stays_text() {
        let n = |s: &str| Value::from(s.parse::<i64>().unwrap());
        let s = |t: &str| Value::String(t.into());
        assert_eq!(value(Shape::Integer, "180000".into(), Some(&s("9"))), n("180000"));
        assert_eq!(value(Shape::Infer, "42".into(), None), n("42"));
        assert_eq!(value(Shape::Infer, "42".into(), Some(&n("7"))), n("42"));
        assert_eq!(value(Shape::Infer, "42".into(), Some(&s("x"))), s("42"));
        assert_eq!(value(Shape::Infer, "007".into(), None), s("007"));
        assert_eq!(value(Shape::Infer, "1.5".into(), None), s("1.5"));
        assert_eq!(value(Shape::Text, "42".into(), None), s("42"));
    }
}
