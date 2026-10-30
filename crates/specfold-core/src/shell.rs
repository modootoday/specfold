//! Shell words for the commands specfold suggests (`next:`, fix commands, reruns).

/// `s` as one POSIX shell word: plain words stay as they are, anything else is single-quoted
/// with embedded `'` written as `'\''`. Every suggested command that embeds a path, id, or
/// other user value goes through this one function.
pub fn quote(s: &str) -> String {
    let plain = !s.is_empty()
        && !s.starts_with(['#', '~', '='])
        && s.chars().all(|c| c.is_ascii_alphanumeric() || "-_./:#@=+,".contains(c));
    if plain { s.to_string() } else { format!("'{}'", s.replace('\'', "'\\''")) }
}

/// Any displayable value (an id, a path) as one shell word.
pub fn word(v: impl std::fmt::Display) -> String {
    quote(&v.to_string())
}

/// Every item quoted and joined with spaces.
pub fn join<S: AsRef<str>>(items: &[S]) -> String {
    items.iter().map(|s| quote(s.as_ref())).collect::<Vec<_>>().join(" ")
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quotes_only_what_the_shell_would_split_or_expand() {
        assert_eq!(quote("src/new-dir"), "src/new-dir");
        assert_eq!(quote("sot:shop/cart-totals"), "sot:shop/cart-totals");
        assert_eq!(quote("src/new dir"), "'src/new dir'");
        assert_eq!(quote("src/it's"), "'src/it'\\''s'");
        assert_eq!(quote("src/한글"), "'src/한글'");
        assert_eq!(quote(""), "''");
        assert_eq!(quote("#x"), "'#x'");
        assert_eq!(quote("a$b"), "'a$b'");
    }
}
