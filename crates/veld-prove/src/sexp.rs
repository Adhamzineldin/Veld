//! A small S-expression reader for what a solver prints, and the decoding of the numbers in it.

/// One S-expression. A quoted symbol (`|x y|`) is an atom without its bars; a string literal is
/// kept apart so that an `(error "...")` message cannot be mistaken for a symbol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Sexp {
    Atom(String),
    Str(String),
    List(Vec<Sexp>),
}

impl Sexp {
    /// The text of an atom.
    #[must_use]
    pub fn atom(&self) -> Option<&str> {
        match self {
            Sexp::Atom(a) => Some(a),
            Sexp::Str(_) | Sexp::List(_) => None,
        }
    }

    /// The items of a list.
    #[must_use]
    pub fn list(&self) -> Option<&[Sexp]> {
        match self {
            Sexp::List(items) => Some(items),
            Sexp::Atom(_) | Sexp::Str(_) => None,
        }
    }

    /// True for `(error "...")`.
    #[must_use]
    pub fn is_error(&self) -> bool {
        self.list()
            .and_then(<[Sexp]>::first)
            .and_then(Sexp::atom)
            .is_some_and(|a| a == "error")
    }

    /// An integer: `5`, `(- 5)`.
    #[must_use]
    pub fn as_int(&self) -> Option<i128> {
        match self {
            Sexp::Atom(a) => a.parse().ok(),
            Sexp::List(items) => match items.as_slice() {
                [Sexp::Atom(minus), x] if minus == "-" => x.as_int().map(|n| -n),
                _ => None,
            },
            Sexp::Str(_) => None,
        }
    }

    /// A rational `(numerator, denominator)` with a positive denominator: `1.5`, `3`,
    /// `(/ 1.0 3.0)`, `(- 2.5)`.
    #[must_use]
    pub fn as_rational(&self) -> Option<(i128, i128)> {
        self.rational().map(|(n, d)| {
            let g = gcd(n.unsigned_abs(), d.unsigned_abs()).max(1);
            let g = i128::try_from(g).unwrap_or(1);
            (n / g, d / g)
        })
    }

    fn rational(&self) -> Option<(i128, i128)> {
        match self {
            Sexp::Atom(a) => decimal_text(a),
            Sexp::List(items) => match items.as_slice() {
                [Sexp::Atom(minus), x] if minus == "-" => x.rational().map(|(n, d)| (-n, d)),
                [Sexp::Atom(slash), a, b] if slash == "/" => {
                    let ((an, ad), (bn, bd)) = (a.rational()?, b.rational()?);
                    let (num, den) = (an.checked_mul(bd)?, ad.checked_mul(bn)?);
                    if den == 0 {
                        None
                    } else if den < 0 {
                        Some((-num, -den))
                    } else {
                        Some((num, den))
                    }
                }
                _ => None,
            },
            Sexp::Str(_) => None,
        }
    }

    /// `true` or `false`.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self.atom()? {
            "true" => Some(true),
            "false" => Some(false),
            _ => None,
        }
    }
}

fn gcd(a: u128, b: u128) -> u128 {
    if b == 0 { a } else { gcd(b, a % b) }
}

/// `12`, `1.50` as a fraction.
fn decimal_text(text: &str) -> Option<(i128, i128)> {
    let (int, frac) = text.split_once('.').unwrap_or((text, ""));
    if int.is_empty() && frac.is_empty() {
        return None;
    }
    if !int.bytes().chain(frac.bytes()).all(|b| b.is_ascii_digit()) {
        return None;
    }
    let digits = format!("{int}{frac}");
    let num: i128 = digits.parse().ok()?;
    let den = 10i128.checked_pow(u32::try_from(frac.len()).ok()?)?;
    Some((num, den))
}

enum Token {
    Open,
    Close,
    Atom(String),
    Str(String),
}

fn tokens(text: &str) -> Vec<Token> {
    let chars: Vec<char> = text.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while let Some(&c) = chars.get(i) {
        match c {
            '(' => out.push(Token::Open),
            ')' => out.push(Token::Close),
            '"' => {
                let (s, next) = quoted(&chars, i + 1, '"', true);
                out.push(Token::Str(s));
                i = next;
                continue;
            }
            '|' => {
                let (s, next) = quoted(&chars, i + 1, '|', false);
                out.push(Token::Atom(s));
                i = next;
                continue;
            }
            c if c.is_whitespace() => {}
            _ => {
                let start = i;
                while chars
                    .get(i)
                    .is_some_and(|c| !c.is_whitespace() && !matches!(c, '(' | ')' | '"' | '|'))
                {
                    i += 1;
                }
                out.push(Token::Atom(chars[start..i].iter().collect()));
                continue;
            }
        }
        i += 1;
    }
    out
}

/// The text up to the closing `quote`, and the index after it. `doubled` allows `""` inside.
fn quoted(chars: &[char], from: usize, quote: char, doubled: bool) -> (String, usize) {
    let mut s = String::new();
    let mut i = from;
    while let Some(&c) = chars.get(i) {
        if c == quote {
            if doubled && chars.get(i + 1) == Some(&quote) {
                s.push(quote);
                i += 2;
                continue;
            }
            return (s, i + 1);
        }
        s.push(c);
        i += 1;
    }
    (s, i)
}

/// Reads every complete top-level expression of `text`; an unfinished one at the end is dropped.
#[must_use]
pub fn parse_all(text: &str) -> Vec<Sexp> {
    let mut stack: Vec<Vec<Sexp>> = vec![Vec::new()];
    for t in tokens(text) {
        match t {
            Token::Open => stack.push(Vec::new()),
            Token::Close => {
                if stack.len() > 1 {
                    let done = Sexp::List(stack.pop().unwrap_or_default());
                    if let Some(top) = stack.last_mut() {
                        top.push(done);
                    }
                }
            }
            Token::Atom(a) => push(&mut stack, Sexp::Atom(a)),
            Token::Str(s) => push(&mut stack, Sexp::Str(s)),
        }
    }
    stack.into_iter().next().unwrap_or_default()
}

fn push(stack: &mut [Vec<Sexp>], x: Sexp) {
    if let Some(top) = stack.last_mut() {
        top.push(x);
    }
}
