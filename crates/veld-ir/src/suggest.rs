//! "Did you mean" support shared by the compiler and the project loader.

/// Levenshtein edit distance between two strings (by `char`).
#[must_use]
pub fn levenshtein(a: &str, b: &str) -> usize {
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
/// Ties go to the alphabetically first candidate, so the result is deterministic.
#[must_use]
pub fn closest<'a>(word: &str, candidates: impl IntoIterator<Item = &'a str>) -> Option<String> {
    let limit = (word.chars().count() / 3).max(2);
    candidates
        .into_iter()
        .filter(|c| *c != word)
        .map(|c| (levenshtein(word, c), c))
        .filter(|(d, _)| *d <= limit)
        .min_by(|a, b| a.0.cmp(&b.0).then(a.1.cmp(b.1)))
        .map(|(_, c)| c.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn distance_and_closest() {
        assert_eq!(levenshtein("kitten", "sitting"), 3);
        assert_eq!(
            closest("Strng", ["string", "int"]).as_deref(),
            Some("string")
        );
        assert_eq!(closest("zzzzzz", ["string", "int"]), None);
    }
}
