//! The palette and the symbol sets. Colour and Unicode are decided once and passed around as
//! a [`Theme`], so renderers stay pure functions of their input.

/// Semantic text styles. Nothing outside this module writes an escape code.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Style {
    Red,
    Green,
    Yellow,
    Cyan,
    Dim,
    Bold,
    /// The product colour: bold cyan.
    Brand,
}

impl Style {
    fn code(self) -> &'static str {
        match self {
            Style::Red => "1;31",
            Style::Green => "32",
            Style::Yellow => "1;33",
            Style::Cyan => "36",
            Style::Dim => "2",
            Style::Bold => "1",
            Style::Brand => "1;36",
        }
    }
}

/// Glyphs for one terminal class.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Symbols {
    pub ok: &'static str,
    pub fail: &'static str,
    pub warn: &'static str,
    pub arrow: &'static str,
    pub bullet: &'static str,
    pub active: &'static str,
    pub done: &'static str,
    pub rail: &'static str,
    pub top: &'static str,
    pub bottom: &'static str,
    pub rule: &'static str,
    pub frames: &'static [&'static str],
}

const UNICODE: Symbols = Symbols {
    ok: "✓",
    fail: "✗",
    warn: "⚠",
    arrow: "→",
    bullet: "•",
    active: "◆",
    done: "◇",
    rail: "│",
    top: "┌",
    bottom: "└",
    rule: "─",
    frames: &["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"],
};

const ASCII: Symbols = Symbols {
    ok: "+",
    fail: "x",
    warn: "!",
    arrow: "->",
    bullet: "*",
    active: "*",
    done: "o",
    rail: "|",
    top: "+",
    bottom: "+",
    rule: "-",
    frames: &["|", "/", "-", "\\"],
};

/// How to render for one output stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Theme {
    pub color: bool,
    pub unicode: bool,
}

impl Theme {
    /// No colour, Unicode glyphs: what tests and snapshots use.
    pub const PLAIN: Theme = Theme {
        color: false,
        unicode: true,
    };
    #[cfg(test)]
    pub const COLORED: Theme = Theme {
        color: true,
        unicode: true,
    };
    #[cfg(test)]
    pub const ASCII: Theme = Theme {
        color: false,
        unicode: false,
    };

    #[must_use]
    pub fn sym(&self) -> &'static Symbols {
        if self.unicode { &UNICODE } else { &ASCII }
    }

    #[must_use]
    pub fn paint(&self, style: Style, text: &str) -> String {
        if self.color && !text.is_empty() {
            format!("\x1b[{}m{text}\x1b[0m", style.code())
        } else {
            text.to_string()
        }
    }
}

/// Unicode is on unless `VELD_ASCII` is set, `TERM=dumb`, or a POSIX `LANG=C`.
#[must_use]
pub fn detect_unicode() -> bool {
    if std::env::var_os("VELD_ASCII").is_some_and(|v| !v.is_empty()) {
        return false;
    }
    if std::env::var("TERM").is_ok_and(|t| t == "dumb") {
        return false;
    }
    if cfg!(windows) {
        return true;
    }
    match std::env::var("LC_ALL").or_else(|_| std::env::var("LANG")) {
        Ok(l) => !(l == "C" || l == "POSIX"),
        Err(_) => true,
    }
}

/// Colour is off for `--no-color` and for a non-empty `NO_COLOR`.
#[must_use]
pub fn color_allowed(no_color_flag: bool) -> bool {
    let env_off = std::env::var_os("NO_COLOR").is_some_and(|v| !v.is_empty());
    !no_color_flag && !env_off
}

/// Display width of `s` without escape codes (one column per char; the glyphs used are narrow).
#[must_use]
pub fn width(s: &str) -> usize {
    let mut n = 0;
    let mut in_esc = false;
    for c in s.chars() {
        match (in_esc, c) {
            (true, 'm') => in_esc = false,
            (true, _) => {}
            (false, '\x1b') => in_esc = true,
            (false, _) => n += 1,
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn plain_adds_nothing_and_colour_wraps() {
        assert_eq!(Theme::PLAIN.paint(Style::Green, "ok"), "ok");
        assert_eq!(
            Theme::COLORED.paint(Style::Green, "ok"),
            "\x1b[32mok\x1b[0m"
        );
        assert_eq!(width(&Theme::COLORED.paint(Style::Bold, "abc")), 3);
    }

    #[test]
    fn ascii_fallback_has_no_non_ascii() {
        let s = Theme::ASCII.sym();
        for g in [
            s.ok, s.fail, s.warn, s.arrow, s.bullet, s.active, s.done, s.rail, s.top, s.bottom,
            s.rule,
        ] {
            assert!(g.is_ascii(), "{g}");
        }
        assert!(s.frames.iter().all(|f| f.is_ascii()));
    }
}
