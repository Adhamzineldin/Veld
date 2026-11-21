//! "Did you mean" support for `veld.toml` keys.

pub(crate) use veld_ir::suggest::closest;

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
