//! Strings by code point, uuids in lower case, and the size of base64 data.

/// The number of Unicode code points (the meaning of `@length` and `len`).
#[must_use]
pub fn len_code_points(s: &str) -> usize {
    s.chars().count()
}

/// Whether `s` starts with `prefix`, compared by code points.
#[must_use]
pub fn starts_with(s: &str, prefix: &str) -> bool {
    s.starts_with(prefix)
}

/// Whether `s` ends with `suffix`, compared by code points.
#[must_use]
pub fn ends_with(s: &str, suffix: &str) -> bool {
    s.ends_with(suffix)
}

/// Whether `part` occurs in `s`, compared by code points.
#[must_use]
pub fn contains(s: &str, part: &str) -> bool {
    s.contains(part)
}

/// A uuid in its canonical lower-case form, or `None` when it is not 8-4-4-4-12 hex digits.
#[must_use]
pub fn normalize_uuid(s: &str) -> Option<String> {
    let dashes = [8usize, 13, 18, 23];
    let ok = s.len() == 36
        && s.bytes().enumerate().all(|(i, c)| {
            if dashes.contains(&i) {
                c == b'-'
            } else {
                c.is_ascii_hexdigit()
            }
        });
    ok.then(|| s.to_ascii_lowercase())
}

/// Uuid equality, ignoring ASCII case.
#[must_use]
pub fn uuid_eq(a: &str, b: &str) -> bool {
    a.eq_ignore_ascii_case(b)
}

/// The number of bytes that padded base64 text stands for, without decoding it. `None` when
/// the text is not padded base64.
#[must_use]
pub fn base64_byte_len(text: &str) -> Option<usize> {
    let b = text.as_bytes();
    if b.len() & 3 != 0 {
        return None;
    }
    let pad = b.iter().rev().take_while(|c| **c == b'=').count();
    if pad > 2 {
        return None;
    }
    let body = b.get(..b.len() - pad)?;
    let valid = body
        .iter()
        .all(|c| c.is_ascii_alphanumeric() || *c == b'+' || *c == b'/');
    valid.then(|| b.len() / 4 * 3 - pad)
}
