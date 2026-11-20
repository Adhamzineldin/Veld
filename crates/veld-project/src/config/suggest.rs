//! "Did you mean" support.

/// Levenshtein edit distance between two strings (by `char`).
pub(crate) fn levenshtein(a: &str, b: &str) -> usize {
    let b: Vec<char> = b.chars().collect();
    let mut prev: Vec<usize> = (0..=b.len()).collect();
    for (i, ca) in a.chars().enumerate() {
        let mut cur = vec![i + 1];
        for (j, cb) in b.iter().enumerate() {
            let cost = usize::from(ca != *cb);
            cur.push((prev[j] + cost).min(prev[j + 1] + 1).min(cur[j] + 1));
        }
        prev = cur;
    }
    prev[b.len()]
}

/// The closest candidate to `word`, if it is close enough to be a plausible typo.
pub(crate) fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let limit = (word.chars().count() / 3).max(2);
    candidates
        .into_iter()
        .map(|c| (levenshtein(word, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c.to_string())
}

/// `; did you mean `x`?` or, with no close match, `; valid keys: a, b`.
pub(crate) fn hint(suggestion: Option<&str>, valid: &[&str]) -> String {
    match suggestion {
        Some(s) => format!("; did you mean `{s}`?"),
        None if valid.is_empty() => String::new(),
        None => format!("; valid keys: {}", valid.join(", ")),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance() {
        assert_eq!(levenshtein("", ""), 0);
        assert_eq!(levenshtein("abc", "abc"), 0);
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(levenshtein("emiter", "emitter"), 1);
        assert_eq!(levenshtein("", "ab"), 2);
    }

    #[test]
    fn closest_match() {
        let keys = ["role", "emitter", "out"];
        assert_eq!(closest("emiter", keys), Some("emitter".to_string()));
        assert_eq!(closest("ot", keys), Some("out".to_string()));
        assert_eq!(closest("completely-different", keys), None);
    }

    #[test]
    fn hints() {
        assert_eq!(hint(Some("x"), &["a"]), "; did you mean `x`?");
        assert_eq!(hint(None, &["a", "b"]), "; valid keys: a, b");
        assert_eq!(hint(None, &[]), "");
    }
}
