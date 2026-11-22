//! Syntax check for the portable regex subset (`spec/language.md` section 7.1).
//!
//! Allowed: literals, `.`, classes `[...]`, `\d \w \s`, escaped punctuation,
//! the quantifiers `* + ? {n,m}`, groups `( )`, alternation `|`, anchors `^ $`.
//! Everything else (lookaround, backreferences, lazy quantifiers, other
//! escapes) is rejected, because every target must enforce the same pattern.

/// Returns `Err(reason)` when `pattern` is outside the subset.
pub fn check(pattern: &str) -> Result<(), String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut p = Checker {
        chars: &chars,
        pos: 0,
    };
    p.alternation()?;
    match p.peek() {
        None => Ok(()),
        Some(')') => Err("unmatched `)`".to_string()),
        Some(c) => Err(format!("unexpected `{c}`")),
    }
}

struct Checker<'a> {
    chars: &'a [char],
    pos: usize,
}

impl Checker<'_> {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn alternation(&mut self) -> Result<(), String> {
        loop {
            self.sequence()?;
            if self.peek() == Some('|') {
                self.pos += 1;
            } else {
                return Ok(());
            }
        }
    }

    fn sequence(&mut self) -> Result<(), String> {
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let quantifiable = self.atom()?;
            self.quantifier(quantifiable)?;
        }
        Ok(())
    }

    /// Parses one atom; returns whether a quantifier may follow.
    fn atom(&mut self) -> Result<bool, String> {
        match self.bump() {
            Some('(') => {
                if self.peek() == Some('?') {
                    return Err(
                        "`(?` groups (lookaround, non-capturing, named) are not supported"
                            .to_string(),
                    );
                }
                self.alternation()?;
                if self.bump() != Some(')') {
                    return Err("unclosed group, expected `)`".to_string());
                }
                Ok(true)
            }
            Some('[') => self.class().map(|()| true),
            Some('\\') => self.escape().map(|()| true),
            Some('^' | '$') => Ok(false),
            Some('*' | '+' | '?') => Err("nothing to repeat before a quantifier".to_string()),
            Some('{') => Err("`{` must be a quantifier such as `{2}` or `{1,3}`".to_string()),
            Some(_) => Ok(true),
            None => Err("unexpected end of pattern".to_string()),
        }
    }

    fn escape(&mut self) -> Result<(), String> {
        match self.bump() {
            Some('d' | 'w' | 's') => Ok(()),
            Some(c @ '0'..='9') => Err(format!("backreference `\\{c}` is not supported")),
            Some('k') => Err("named backreferences are not supported".to_string()),
            Some(c) if c.is_ascii_alphanumeric() => Err(format!(
                "escape `\\{c}` is not in the supported subset (only `\\d`, `\\w`, `\\s`)"
            )),
            Some(_) => Ok(()),
            None => Err("pattern ends with a lone `\\`".to_string()),
        }
    }

    fn class(&mut self) -> Result<(), String> {
        if self.peek() == Some('^') {
            self.pos += 1;
        }
        let mut count = 0;
        loop {
            let Some(c) = self.bump() else {
                return Err("unclosed class, expected `]`".to_string());
            };
            match c {
                ']' if count > 0 => return Ok(()),
                '\\' => self.escape()?,
                _ => self.class_range(c)?,
            }
            count += 1;
        }
    }

    fn class_range(&mut self, lo: char) -> Result<(), String> {
        if self.peek() == Some('-') && self.chars.get(self.pos + 1).is_some_and(|n| *n != ']') {
            self.pos += 1;
            let hi = self.bump().unwrap_or(lo);
            if hi < lo {
                return Err(format!("class range `{lo}-{hi}` is out of order"));
            }
        }
        Ok(())
    }

    fn quantifier(&mut self, allowed: bool) -> Result<(), String> {
        let Some(c) = self.peek() else { return Ok(()) };
        if !matches!(c, '*' | '+' | '?' | '{') {
            return Ok(());
        }
        if !allowed {
            return Err("nothing to repeat before a quantifier".to_string());
        }
        self.pos += 1;
        if c == '{' {
            self.braces()?;
        }
        match self.peek() {
            Some('?') => Err("lazy quantifiers are not supported".to_string()),
            Some('*' | '+' | '{') => Err("nothing to repeat before a quantifier".to_string()),
            _ => Ok(()),
        }
    }

    fn braces(&mut self) -> Result<(), String> {
        let start = self.pos;
        let rest: String = self.chars[start..].iter().collect();
        let Some(end) = rest.find('}') else {
            return Err("unclosed `{` quantifier".to_string());
        };
        let body = &rest[..end];
        let mut parts = body.splitn(2, ',');
        let lo = parts.next().unwrap_or("");
        let hi = parts.next();
        let num = |s: &str| !s.is_empty() && s.chars().all(|c| c.is_ascii_digit());
        let ok = num(lo)
            && match hi {
                None | Some("") => true,
                Some(h) => num(h) && h.parse::<u64>().ok() >= lo.parse::<u64>().ok(),
            };
        if !ok {
            return Err(format!("invalid quantifier `{{{body}}}`"));
        }
        self.pos = start + body.chars().count() + 1;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::check;

    #[test]
    fn accepts_the_subset() {
        for p in [
            "^[a-z0-9_]+$",
            "\\d{3}-\\d{2,4}",
            "(a|b)*c?",
            ".+",
            "[^\\s]",
            "a{2,}",
            "\\.",
            "[a-]",
            "x]y}z",
            "",
        ] {
            assert!(check(p).is_ok(), "{p}: {:?}", check(p));
        }
    }

    #[test]
    fn rejects_lookaround_and_backreferences() {
        for p in [
            "(?=a)", "(?:a)", "(?<n>a)", "(a)\\1", "\\k<n>", "\\b", "\\D", "\\n",
        ] {
            assert!(check(p).is_err(), "{p}");
        }
    }

    #[test]
    fn rejects_malformed_patterns() {
        for p in [
            "(a", "a)", "[a", "[]", "*a", "a**", "a{", "a{x}", "a{3,1}", "a*?", "[z-a]", "a\\",
            "^*",
        ] {
            assert!(check(p).is_err(), "{p}");
        }
    }
}
