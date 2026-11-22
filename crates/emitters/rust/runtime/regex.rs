//! A small backtracking regular expression engine for the `@pattern` constraint.
//!
//! Supported: literals, `.`, classes (`[a-z]`, `[^x]`, `\d \w \s` and their negations), anchors
//! (`^ $ \b \B`), groups (`( )`, `(?: )`), alternation and the quantifiers `* + ? {n} {n,} {n,m}`
//! with lazy forms. The generator refuses patterns that use anything else (lookaround and
//! backreferences) with `E0801`, so this engine never silently disagrees with another target.
//! A match is a search: it can start anywhere unless the pattern is anchored.

#[derive(Debug, Clone)]
enum ClassItem {
    Range(char, char),
    Digit(bool),
    Word(bool),
    Space(bool),
}

#[derive(Debug, Clone)]
enum Node {
    Char(char),
    Any,
    Class(Vec<ClassItem>, bool),
    Start,
    End,
    WordBoundary(bool),
    Seq(Vec<Node>),
    Alt(Vec<Node>),
    Repeat(Box<Node>, usize, usize, bool),
}

/// A compiled pattern.
#[derive(Debug, Clone)]
pub struct Regex {
    root: Node,
}

struct Parser {
    chars: Vec<char>,
    pos: usize,
}

impl Parser {
    fn peek(&self) -> Option<char> {
        self.chars.get(self.pos).copied()
    }

    fn next(&mut self) -> Option<char> {
        let c = self.peek();
        self.pos += 1;
        c
    }

    fn eat(&mut self, c: char) -> bool {
        if self.peek() == Some(c) {
            self.pos += 1;
            true
        } else {
            false
        }
    }

    fn alternation(&mut self) -> Option<Node> {
        let mut arms = vec![self.sequence()?];
        while self.eat('|') {
            arms.push(self.sequence()?);
        }
        Some(if arms.len() == 1 { arms.remove(0) } else { Node::Alt(arms) })
    }

    fn sequence(&mut self) -> Option<Node> {
        let mut items = Vec::new();
        while let Some(c) = self.peek() {
            if c == '|' || c == ')' {
                break;
            }
            let atom = self.atom()?;
            items.push(self.quantified(atom)?);
        }
        Some(Node::Seq(items))
    }

    fn number(&mut self) -> Option<usize> {
        let start = self.pos;
        while self.peek().is_some_and(|c| c.is_ascii_digit()) {
            self.pos += 1;
        }
        self.chars[start..self.pos].iter().collect::<String>().parse().ok()
    }

    fn bounds(&mut self) -> Option<Option<(usize, usize)>> {
        let (min, max) = match self.peek() {
            Some('*') => (0, usize::MAX),
            Some('+') => (1, usize::MAX),
            Some('?') => (0, 1),
            Some('{') => {
                let save = self.pos;
                self.pos += 1;
                let Some(lo) = self.number() else {
                    self.pos = save;
                    return Some(None);
                };
                let hi = if self.eat(',') {
                    if self.peek() == Some('}') { usize::MAX } else { self.number()? }
                } else {
                    lo
                };
                if self.peek() != Some('}') {
                    return None;
                }
                (lo, hi)
            }
            _ => return Some(None),
        };
        self.pos += 1;
        Some(Some((min, max)))
    }

    fn quantified(&mut self, atom: Node) -> Option<Node> {
        match self.bounds()? {
            None => Some(atom),
            Some((min, max)) => {
                let greedy = !self.eat('?');
                Some(Node::Repeat(Box::new(atom), min, max, greedy))
            }
        }
    }

    fn atom(&mut self) -> Option<Node> {
        match self.next()? {
            '.' => Some(Node::Any),
            '^' => Some(Node::Start),
            '$' => Some(Node::End),
            '(' => {
                if self.eat('?') && !self.eat(':') {
                    return None;
                }
                let inner = self.alternation()?;
                self.eat(')').then_some(inner)
            }
            '[' => self.class(),
            '\\' => self.escape(),
            '*' | '+' | '?' => None,
            c => Some(Node::Char(c)),
        }
    }

    fn escape(&mut self) -> Option<Node> {
        let c = self.next()?;
        Some(match c {
            'd' => Node::Class(vec![ClassItem::Digit(true)], false),
            'D' => Node::Class(vec![ClassItem::Digit(false)], false),
            'w' => Node::Class(vec![ClassItem::Word(true)], false),
            'W' => Node::Class(vec![ClassItem::Word(false)], false),
            's' => Node::Class(vec![ClassItem::Space(true)], false),
            'S' => Node::Class(vec![ClassItem::Space(false)], false),
            'b' => Node::WordBoundary(true),
            'B' => Node::WordBoundary(false),
            'n' => Node::Char('\n'),
            'r' => Node::Char('\r'),
            't' => Node::Char('\t'),
            c if c.is_ascii_alphanumeric() => return None,
            c => Node::Char(c),
        })
    }

    fn class_escape(&mut self) -> Option<Result<char, ClassItem>> {
        Some(match self.next()? {
            'd' => Err(ClassItem::Digit(true)),
            'D' => Err(ClassItem::Digit(false)),
            'w' => Err(ClassItem::Word(true)),
            'W' => Err(ClassItem::Word(false)),
            's' => Err(ClassItem::Space(true)),
            'S' => Err(ClassItem::Space(false)),
            'n' => Ok('\n'),
            'r' => Ok('\r'),
            't' => Ok('\t'),
            e => Ok(e),
        })
    }

    fn class(&mut self) -> Option<Node> {
        let negate = self.eat('^');
        let mut items = Vec::new();
        let mut first = true;
        loop {
            let c = self.next()?;
            if c == ']' && !first {
                break;
            }
            first = false;
            let lo = if c == '\\' {
                match self.class_escape()? {
                    Ok(ch) => ch,
                    Err(item) => {
                        items.push(item);
                        continue;
                    }
                }
            } else {
                c
            };
            if self.peek() == Some('-') && self.chars.get(self.pos + 1).is_some_and(|n| *n != ']') {
                self.pos += 1;
                let mut hi = self.next()?;
                if hi == '\\' {
                    hi = self.next()?;
                }
                if hi < lo {
                    return None;
                }
                items.push(ClassItem::Range(lo, hi));
            } else {
                items.push(ClassItem::Range(lo, lo));
            }
        }
        Some(Node::Class(items, negate))
    }
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

fn class_matches(items: &[ClassItem], negate: bool, c: char) -> bool {
    let hit = items.iter().any(|item| match item {
        ClassItem::Range(lo, hi) => *lo <= c && c <= *hi,
        ClassItem::Digit(yes) => c.is_ascii_digit() == *yes,
        ClassItem::Word(yes) => is_word(c) == *yes,
        ClassItem::Space(yes) => c.is_whitespace() == *yes,
    });
    hit != negate
}

type Cont<'a> = &'a mut dyn FnMut(usize) -> bool;

struct Rep<'a> {
    inner: &'a Node,
    min: usize,
    max: usize,
    greedy: bool,
}

fn run(node: &Node, s: &[char], i: usize, k: Cont<'_>) -> bool {
    match node {
        Node::Char(c) => s.get(i) == Some(c) && k(i + 1),
        Node::Any => s.get(i).is_some_and(|c| *c != '\n' && *c != '\r') && k(i + 1),
        Node::Class(items, negate) => {
            s.get(i).is_some_and(|c| class_matches(items, *negate, *c)) && k(i + 1)
        }
        Node::Start => i == 0 && k(i),
        Node::End => i == s.len() && k(i),
        Node::WordBoundary(want) => {
            let before = i > 0 && is_word(s[i - 1]);
            let after = s.get(i).is_some_and(|c| is_word(*c));
            ((before != after) == *want) && k(i)
        }
        Node::Seq(items) => seq(items, s, i, k),
        Node::Alt(arms) => {
            for arm in arms {
                if run(arm, s, i, k) {
                    return true;
                }
            }
            false
        }
        Node::Repeat(inner, min, max, greedy) => {
            let rep = Rep { inner, min: *min, max: *max, greedy: *greedy };
            repeat(&rep, s, i, 0, k)
        }
    }
}

fn seq(items: &[Node], s: &[char], i: usize, k: Cont<'_>) -> bool {
    match items.split_first() {
        None => k(i),
        Some((first, rest)) => run(first, s, i, &mut |j| seq(rest, s, j, k)),
    }
}

fn repeat(rep: &Rep<'_>, s: &[char], i: usize, count: usize, k: Cont<'_>) -> bool {
    let more = |k: Cont<'_>| {
        count < rep.max
            && run(rep.inner, s, i, &mut |j| {
                // An empty iteration cannot make progress: refuse it to avoid looping forever.
                if j == i && count >= rep.min {
                    return false;
                }
                repeat(rep, s, j, count + 1, k)
            })
    };
    if count < rep.min {
        return more(k);
    }
    if rep.greedy {
        if more(k) {
            return true;
        }
        k(i)
    } else {
        if k(i) {
            return true;
        }
        more(k)
    }
}

impl Regex {
    /// Compiles `pattern`; `None` when it uses syntax this engine does not support.
    #[must_use]
    pub fn new(pattern: &str) -> Option<Regex> {
        let mut p = Parser { chars: pattern.chars().collect(), pos: 0 };
        let root = p.alternation()?;
        (p.pos == p.chars.len()).then_some(Regex { root })
    }

    /// Whether the pattern matches anywhere in `text`.
    #[must_use]
    pub fn is_match(&self, text: &str) -> bool {
        let s: Vec<char> = text.chars().collect();
        (0..=s.len()).any(|start| run(&self.root, &s, start, &mut |_| true))
    }
}

#[cfg(test)]
mod tests {
    use super::Regex;

    fn m(p: &str, t: &str) -> bool {
        Regex::new(p).is_some_and(|r| r.is_match(t))
    }

    #[test]
    fn matches_the_contract_patterns() {
        assert!(m("^[A-Z]{3}-[0-9]+$", "ABC-12"));
        assert!(!m("^[A-Z]{3}-[0-9]+$", "AB-12"));
        assert!(m("^(a|bc)+d?$", "abca"));
        assert!(m("b+", "abbbc"));
        assert!(!m("^\\d{2,3}$", "1"));
        assert!(m("^[^x]*?y$", "aby"));
        assert!(Regex::new("(?=a)").is_none());
        assert!(Regex::new("\\1").is_none());
    }
}
