//! Names close to a mistyped one, by edit distance: the one owner of "did you mean" suggestions.

/// Edit distance between two strings, by characters.
pub fn distance(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut prev = row[0];
        row[0] = i + 1;
        for j in 0..b.len() {
            let cur = row[j + 1];
            row[j + 1] = (prev + usize::from(ca != b[j])).min(row[j] + 1).min(cur + 1);
            prev = cur;
        }
    }
    row[b.len()]
}

/// The one value close enough to `want` to be a typo of it: within a third of its length (at
/// least two edits), and strictly closer than any other value. Case is ignored.
pub fn nearest<'a>(want: &str, values: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    let mut scored: Vec<(usize, &str)> = values
        .map(|v| (distance(&want.to_lowercase(), &v.to_lowercase()), v))
        .filter(|(d, v)| *d > 0 && *d <= 2.max(v.chars().count() / 3))
        .collect();
    scored.sort();
    match scored.as_slice() {
        [(d, v), rest @ ..] if rest.first().is_none_or(|(e, _)| e > d) => Some(v),
        _ => None,
    }
}

/// The closest name a command word could be a typo or an abbreviation of: within a third of its
/// length (at least two edits) or starting with `want`; the first of equally close names wins.
pub fn nearest_or_prefixed<'a>(want: &str, names: impl Iterator<Item = &'a str>) -> Option<&'a str> {
    names
        .map(|n| (distance(want, n), n))
        .filter(|(d, n)| *d <= 2.max(n.len() / 3) || n.starts_with(want))
        .min_by_key(|(d, _)| *d)
        .map(|(_, n)| n)
}
