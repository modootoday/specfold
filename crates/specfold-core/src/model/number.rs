//! Sequence numbers used as document keys: `0001`, `0042`, `12345`.

/// How many digits a number key has at least.
pub const WIDTH: usize = 4;

/// The key for `n`: zero-padded to four digits, wider when it needs more.
pub fn key(n: u64) -> String {
    format!("{n:0WIDTH$}")
}

/// The number a key stands for, when the key is written the one way [`key`] writes it.
pub fn parse(s: &str) -> Option<u64> {
    if s.len() < WIDTH || !s.bytes().all(|b| b.is_ascii_digit()) {
        return None;
    }
    let n: u64 = s.parse().ok()?;
    (key(n) == s).then_some(n)
}

/// Whether `s` is a number key.
pub fn is_number(s: &str) -> bool {
    parse(s).is_some()
}

/// The number key after `k`.
pub fn next(k: &str) -> Option<String> {
    parse(k)?.checked_add(1).map(key)
}

/// The first number above every key in `taken`; `0001` when there is none.
pub fn next_free<'a>(taken: impl IntoIterator<Item = &'a str>) -> String {
    let top = taken.into_iter().filter_map(parse).max().unwrap_or(0);
    key(top.saturating_add(1))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn number_keys_are_padded_and_written_one_way() {
        assert_eq!(key(7), "0007");
        assert_eq!(key(12345), "12345");
        assert_eq!(parse("0042"), Some(42));
        assert_eq!(parse("00042"), None, "a wider key than needed names the same number twice");
        assert_eq!(parse("042"), None);
        assert_eq!(parse("12a4"), None);
        assert_eq!(next("0009").as_deref(), Some("0010"));
        assert_eq!(next_free(["0001", "0007", "x", "0003"]), "0008");
        assert_eq!(next_free([]), "0001");
    }
}
