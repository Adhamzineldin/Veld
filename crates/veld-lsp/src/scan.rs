//! A forgiving scan of the text before the cursor for the rules language. It knows which block
//! the cursor is in (an endpoint block, a policy, an example, an enum, the body of a model) and
//! where the statement being typed starts, without needing the text to parse.
//!
//! It never indexes with a computed position: every access goes through `get`.

/// What a token is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum K {
    Word,
    Num,
    Str,
    Punct,
}

/// One token with its place.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Tk {
    pub k: K,
    pub text: String,
    pub start: usize,
    pub end: usize,
    pub line: usize,
}

impl Tk {
    #[must_use]
    pub fn is(&self, text: &str) -> bool {
        self.text == text && self.k != K::Str
    }

    #[must_use]
    pub fn is_word(&self) -> bool {
        self.k == K::Word
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

const PAIRS: [&str; 8] = ["->", "=>", "&&", "||", "==", "!=", "<=", ">="];

/// Tokenises `text`. The flag is true when the text ends inside a comment or a string.
#[must_use]
pub fn lex(text: &str) -> (Vec<Tk>, bool) {
    let chars: Vec<(usize, char)> = text.char_indices().collect();
    let at = |i: usize| chars.get(i).map(|c| c.1);
    let byte = |i: usize| chars.get(i).map_or(text.len(), |c| c.0);
    let (mut out, mut i, mut line) = (Vec::new(), 0usize, 0usize);
    let push = |k: K, from: usize, to: usize, line: usize, out: &mut Vec<Tk>| {
        let (s, e) = (byte(from), byte(to));
        out.push(Tk {
            k,
            text: text.get(s..e).unwrap_or("").to_owned(),
            start: s,
            end: e,
            line,
        });
    };
    while let Some(c) = at(i) {
        if c == '\n' {
            line += 1;
            i += 1;
        } else if c.is_whitespace() {
            i += 1;
        } else if c == '/' && at(i + 1) == Some('/') {
            while at(i).is_some_and(|c| c != '\n') {
                i += 1;
            }
            if at(i).is_none() {
                return (out, true);
            }
        } else if c == '"' {
            let from = i;
            i += 1;
            while at(i).is_some_and(|c| c != '"' && c != '\n') {
                i += if at(i) == Some('\\') { 2 } else { 1 };
            }
            if at(i) != Some('"') {
                let ended = at(i).is_none() || i > chars.len();
                if ended {
                    return (out, true);
                }
                push(K::Str, from, i, line, &mut out);
                continue;
            }
            i += 1;
            push(K::Str, from, i, line, &mut out);
        } else if c.is_ascii_digit() {
            let from = i;
            while at(i).is_some_and(|c| c.is_ascii_alphanumeric() || c == '.')
                && !(at(i) == Some('.') && !at(i + 1).is_some_and(|d| d.is_ascii_digit()))
            {
                i += 1;
            }
            push(K::Num, from, i, line, &mut out);
        } else if is_word_char(c) {
            let from = i;
            while at(i).is_some_and(is_word_char) {
                i += 1;
            }
            push(K::Word, from, i, line, &mut out);
        } else {
            let two: String = [Some(c), at(i + 1)].into_iter().flatten().collect();
            let width = if PAIRS.contains(&two.as_str()) { 2 } else { 1 };
            push(K::Punct, i, i + width, line, &mut out);
            i += width;
        }
    }
    (out, false)
}

/// The block the cursor is in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Frame {
    Top,
    /// The body of a `model`, `error`, `event`, `enum`, `union` or `channel`.
    Decl {
        kw: String,
        name: String,
    },
    Service {
        name: String,
    },
    /// The `{ }` of an endpoint.
    Endpoint {
        service: String,
        name: String,
    },
    Policy {
        name: String,
    },
    Example,
    /// The `{ field: expr }` after `else Error` in a `require`.
    Payload {
        error: String,
    },
    /// A `{ }` value of an example.
    Object,
    Other,
}

/// An open `(` or `[` inside the current block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Open {
    /// Index of the opener in `Site::toks`.
    pub at: usize,
    /// The word right before a `(`: the function that is being called.
    pub call: Option<String>,
    pub paren: bool,
}

/// Where the cursor is.
#[derive(Debug, Clone)]
pub struct Site {
    /// The tokens before the cursor, without the word that is being typed.
    pub toks: Vec<Tk>,
    /// The word being typed.
    pub partial: String,
    /// The innermost block, and the blocks around it, outermost first.
    pub frames: Vec<Frame>,
    /// Index in `toks` of the first token of the current statement.
    pub stmt: usize,
    /// The `(` and `[` that are open in the current statement, outermost first.
    pub opens: Vec<Open>,
    /// The cursor is inside a comment or a string.
    pub inert: bool,
    /// No token before the cursor is on the cursor's line.
    pub line_start: bool,
}

impl Site {
    /// The innermost block.
    #[must_use]
    pub fn frame(&self) -> &Frame {
        self.frames.last().unwrap_or(&Frame::Top)
    }

    /// The tokens of the current statement.
    #[must_use]
    pub fn stmt_toks(&self) -> &[Tk] {
        self.toks.get(self.stmt..).unwrap_or(&[])
    }

    /// The first word of the current statement.
    #[must_use]
    pub fn first(&self) -> Option<&str> {
        self.stmt_toks().first().map(|t| t.text.as_str())
    }

    /// The nearest enclosing endpoint block, as `(service, endpoint)`.
    #[must_use]
    pub fn endpoint(&self) -> Option<(&str, &str)> {
        self.frames.iter().rev().find_map(|f| match f {
            Frame::Endpoint { service, name } => Some((service.as_str(), name.as_str())),
            Frame::Top
            | Frame::Decl { .. }
            | Frame::Service { .. }
            | Frame::Policy { .. }
            | Frame::Example
            | Frame::Payload { .. }
            | Frame::Object
            | Frame::Other => None,
        })
    }
}

enum Level {
    Brace { frame: Frame, stmt: usize },
    Group(Open),
}

fn text_at(toks: &[Tk], i: usize) -> &str {
    toks.get(i).map_or("", |t| t.text.as_str())
}

fn first_on_line(toks: &[Tk], i: usize) -> bool {
    match (i.checked_sub(1).and_then(|p| toks.get(p)), toks.get(i)) {
        (Some(p), Some(t)) => p.line < t.line,
        (None, _) | (_, None) => true,
    }
}

/// The words that start a statement in a block, where they stand first on a line.
fn starts_statement(frame: &Frame, toks: &[Tk], i: usize) -> bool {
    let w = text_at(toks, i);
    if !first_on_line(toks, i) || toks.get(i).is_none_or(|t| !t.is_word()) {
        return false;
    }
    match frame {
        Frame::Top => matches!(
            w,
            "use"
                | "model"
                | "enum"
                | "error"
                | "union"
                | "service"
                | "const"
                | "event"
                | "channel"
                | "policy"
                | "principal"
                | "example"
        ),
        Frame::Decl { kw, .. } if kw == "enum" => {
            matches!(w, "flow" | "final") && text_at(toks, i + 1) != ":"
        }
        Frame::Decl { .. } => w == "rule" && text_at(toks, i + 1) != ":",
        Frame::Service { .. } => {
            veld_compiler::language::is_method_word(w) || matches!(w, "publishes" | "subscribes")
        }
        Frame::Endpoint { .. } => {
            matches!(w, "fact" | "require" | "ensure" | "allow" | "transition")
        }
        Frame::Policy { .. } => matches!(w, "permit" | "forbid"),
        Frame::Example => w != "returns",
        Frame::Payload { .. } | Frame::Object | Frame::Other => false,
    }
}

fn classify(parent: &Frame, grouped: bool, toks: &[Tk], stmt: usize, i: usize) -> Frame {
    let first = text_at(toks, stmt);
    let word = |n: usize| text_at(toks, stmt + n).to_owned();
    match parent {
        Frame::Top => match first {
            "model" | "error" | "event" | "enum" | "union" | "channel" => Frame::Decl {
                kw: word(0),
                name: word(1),
            },
            "service" => Frame::Service { name: word(1) },
            "policy" => Frame::Policy { name: word(1) },
            "example" => Frame::Example,
            _ => Frame::Other,
        },
        Frame::Service { name } if veld_compiler::language::is_method_word(first) => {
            Frame::Endpoint {
                service: name.clone(),
                name: word(2),
            }
        }
        Frame::Endpoint { .. } if first == "require" => {
            let at = (stmt..i).find(|&n| text_at(toks, n) == "else");
            match at {
                Some(n) => Frame::Payload {
                    error: text_at(toks, n + 1).to_owned(),
                },
                None => Frame::Other,
            }
        }
        Frame::Example | Frame::Object => Frame::Object,
        Frame::Payload { .. } if grouped => Frame::Object,
        Frame::Decl { .. }
        | Frame::Service { .. }
        | Frame::Endpoint { .. }
        | Frame::Policy { .. }
        | Frame::Payload { .. }
        | Frame::Other => Frame::Other,
    }
}

fn pop_brace(stack: &mut Vec<Level>, next: usize) {
    while stack.len() > 1 {
        match stack.pop() {
            Some(Level::Brace { .. }) | None => break,
            Some(Level::Group(_)) => {}
        }
    }
    if let Some(Level::Brace { stmt, .. }) = stack.last_mut() {
        *stmt = next;
    }
}

/// Scans the text before the cursor.
#[must_use]
pub fn site(prefix: &str) -> Site {
    let (mut toks, inert) = lex(prefix);
    let typing = prefix.ends_with(is_word_char) && toks.last().is_some_and(Tk::is_word);
    let partial = if typing {
        toks.pop().map_or(String::new(), |t| t.text)
    } else {
        String::new()
    };
    let cursor_line = prefix.matches('\n').count();
    let line_start = toks.last().is_none_or(|t| t.line < cursor_line);
    let mut stack = vec![Level::Brace {
        frame: Frame::Top,
        stmt: 0,
    }];
    for i in 0..toks.len() {
        feed(&mut stack, &toks, i);
    }
    let mut frames = Vec::new();
    let (mut stmt, mut opens) = (0, Vec::new());
    for level in &stack {
        match level {
            Level::Brace { frame, stmt: s } => {
                frames.push(frame.clone());
                stmt = *s;
                opens.clear();
            }
            Level::Group(o) => opens.push(o.clone()),
        }
    }
    Site {
        toks,
        partial,
        frames,
        stmt,
        opens,
        inert,
        line_start,
    }
}

fn feed(stack: &mut Vec<Level>, toks: &[Tk], i: usize) {
    let Some(t) = toks.get(i) else { return };
    let grouped = matches!(stack.last(), Some(Level::Group(_)));
    let (frame, _) = innermost_brace(stack);
    if !grouped && starts_statement(&frame, toks, i) {
        set_stmt(stack, i);
    }
    if t.k != K::Punct {
        return;
    }
    match t.text.as_str() {
        "{" => {
            let (frame, stmt) = innermost_brace(stack);
            let new = classify(&frame, grouped, toks, stmt, i);
            stack.push(Level::Brace {
                frame: new,
                stmt: i + 1,
            });
        }
        "}" => pop_brace(stack, i + 1),
        "(" | "[" => {
            let call = i
                .checked_sub(1)
                .and_then(|p| toks.get(p))
                .filter(|p| t.text == "(" && p.is_word() && p.end == t.start)
                .map(|p| p.text.clone());
            stack.push(Level::Group(Open {
                at: i,
                call,
                paren: t.text == "(",
            }));
        }
        ")" | "]" => {
            if matches!(stack.last(), Some(Level::Group(_))) {
                stack.pop();
            }
        }
        _ => {}
    }
}

fn innermost_brace(stack: &[Level]) -> (Frame, usize) {
    stack
        .iter()
        .rev()
        .find_map(|l| match l {
            Level::Brace { frame, stmt } => Some((frame.clone(), *stmt)),
            Level::Group(_) => None,
        })
        .unwrap_or((Frame::Top, 0))
}

fn set_stmt(stack: &mut [Level], i: usize) {
    if let Some(Level::Brace { stmt, .. }) = stack
        .iter_mut()
        .rev()
        .find(|l| matches!(l, Level::Brace { .. }))
    {
        *stmt = i;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexes_words_numbers_strings_and_pairs() {
        let (t, inert) = lex("a.b >= 1.5 && \"x y\" -> z // c\n d");
        let texts: Vec<_> = t.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(
            texts,
            ["a", ".", "b", ">=", "1.5", "&&", "\"x y\"", "->", "z", "d"]
        );
        assert!(!inert);
        assert_eq!(t.last().map(|t| t.line), Some(1));
        assert!(lex("x // c").1);
        assert!(lex("x \"abc").1);
        assert!(!lex("x \"abc\ny").1);
    }

    #[test]
    fn multibyte_text_does_not_break_offsets() {
        let (t, _) = lex("é \"ü\" b");
        let b = t.last().map(|t| (t.start, t.end));
        assert_eq!(b, Some((8, 9)));
    }

    #[test]
    fn finds_the_endpoint_block_and_the_statement() {
        let s = site(
            "service Bookings at \"/b\" {\n  post \"/\" make(body: A) -> A {\n    fact room: Room = body.roomId\n    require body.guests <= room.cap",
        );
        assert_eq!(
            s.frame(),
            &Frame::Endpoint {
                service: "Bookings".into(),
                name: "make".into()
            }
        );
        assert_eq!(s.first(), Some("require"));
        assert_eq!(s.partial, "cap");
        assert!(s.endpoint().is_some());
        assert!(s.opens.is_empty());
    }

    #[test]
    fn a_payload_and_an_allow_call_are_found() {
        let s = site("service S {\n post \"/\" f() -> A {\n  require x else E { cap: ");
        assert_eq!(s.frame(), &Frame::Payload { error: "E".into() });
        let s = site("service S {\n post \"/\" f() -> A {\n  allow Ok(user, ");
        assert_eq!(s.opens.len(), 1);
        assert_eq!(s.opens.first().and_then(|o| o.call.as_deref()), Some("Ok"));
    }

    #[test]
    fn after_a_closed_block_the_service_body_is_current_again() {
        let s = site("service S {\n post \"/\" f() -> A {\n  require true\n }\n ");
        assert_eq!(s.frame(), &Frame::Service { name: "S".into() });
        assert!(s.line_start);
    }

    #[test]
    fn rules_policies_enums_and_examples_are_blocks() {
        let s = site("model A {\n  x: int\n  rule r: x >");
        assert_eq!(s.first(), Some("rule"));
        let s = site("policy P(user: User) {\n  permit when user.");
        assert_eq!(s.frame(), &Frame::Policy { name: "P".into() });
        let s = site("enum E {\n a, b\n flow a -> ");
        assert_eq!(s.first(), Some("flow"));
        let s = site("example \"x\" {\n  given room = { a: 1 }\n  ");
        assert_eq!(s.frame(), &Frame::Example);
        let s = site("example \"x\" {\n  given room = { a: ");
        assert_eq!(s.frame(), &Frame::Object);
    }

    #[test]
    fn a_field_called_rule_is_not_a_rule() {
        let s = site("model A {\n  rule: int\n  ");
        assert_ne!(s.first(), Some("rule"));
    }
}
