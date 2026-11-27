//! A forgiving lexical scan of the text before the cursor. Completion cannot rely on the
//! syntax tree, because the line being typed is usually not valid yet.

/// Kind of a declaration body.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyKind {
    Model,
    Error,
    Enum,
    Union,
    Service,
}

/// What encloses the cursor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Body(BodyKind),
    /// The `{ ... }` of a `use`.
    Import,
    /// Endpoint parameters.
    Params,
    /// Arguments of `@name(`.
    AnnotationArgs(String),
    /// The `{ ... }` of an endpoint (its rule statements) or of a `subscribes` line.
    Endpoint,
    /// Any other `{ ... }` that no declaration owns: a payload, an object value.
    Nested,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Tok {
    Ident(String),
    Punct(char),
    Arrow,
    Str,
    Num,
}

#[derive(Debug, Clone)]
struct Item {
    tok: Tok,
    line: usize,
}

/// The result of scanning.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Context {
    /// The innermost enclosing block, if any.
    pub block: Option<Block>,
    /// Tokens of the current statement before the cursor, partial word excluded.
    pub stmt: Vec<Tok>,
    /// The word being typed.
    pub partial: String,
    /// Tokens of the `use` statement that holds the cursor, from `use` on.
    pub use_stmt: Option<Vec<Tok>>,
    /// The cursor is inside a comment or a string.
    pub inert: bool,
    /// The last top-level declaration keyword before the cursor (for `extends` and `at`).
    pub at_top_level: bool,
    /// The text of the cursor line before the partial word, for the event positions.
    pub line: String,
}

fn is_word(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

/// Scans `prefix` (the text before the cursor).
#[must_use]
pub fn scan(prefix: &str) -> Context {
    let (mut items, inert) = lex(prefix);
    let mut partial = String::new();
    if let Some(Item {
        tok: Tok::Ident(w), ..
    }) = items.last()
        && prefix.ends_with(|c: char| is_word(c))
    {
        partial.clone_from(w);
        items.pop();
    }
    let line = line_before(prefix, &partial);
    let cursor_line = prefix.matches('\n').count();
    let mut st = State::default();
    for it in &items {
        st.feed(&it.tok);
    }
    let block = st.stack.last().cloned();
    let stmt = statement(&items, &st, cursor_line);
    Context {
        block,
        stmt,
        partial,
        use_stmt: st.use_stmt,
        inert,
        at_top_level: st.stack.is_empty(),
        line,
    }
}

/// The last line of `prefix` without the partial word that ends it.
fn line_before(prefix: &str, partial: &str) -> String {
    let line = prefix.rsplit('\n').next().unwrap_or("");
    line.strip_suffix(partial).unwrap_or(line).to_owned()
}

#[derive(Default)]
struct State {
    stack: Vec<Block>,
    pending: Option<BodyKind>,
    use_stmt: Option<Vec<Tok>>,
    prev: Vec<Tok>,
}

impl State {
    fn feed(&mut self, t: &Tok) {
        if self.stack.is_empty() {
            self.top_level(t);
        } else {
            self.nested(t);
        }
        self.prev.push(t.clone());
        if self.prev.len() > 3 {
            self.prev.remove(0);
        }
    }

    fn top_level(&mut self, t: &Tok) {
        match t {
            Tok::Ident(w) if w == "use" => self.use_stmt = Some(vec![t.clone()]),
            Tok::Ident(w) => {
                if matches!(
                    w.as_str(),
                    "model" | "enum" | "error" | "union" | "service" | "const"
                ) {
                    self.use_stmt = None;
                }
                self.pending = body_kind(w).or(self.pending);
                if let Some(u) = &mut self.use_stmt {
                    u.push(t.clone());
                }
            }
            Tok::Punct('{') => {
                if self.use_stmt.is_some() {
                    self.stack.push(Block::Import);
                } else if let Some(k) = self.pending.take() {
                    self.stack.push(Block::Body(k));
                }
            }
            other @ (Tok::Punct(_) | Tok::Arrow | Tok::Str | Tok::Num) => {
                if let Some(u) = &mut self.use_stmt {
                    u.push(other.clone());
                }
            }
        }
    }

    fn nested(&mut self, t: &Tok) {
        match t {
            Tok::Punct('{') => {
                let block = match self.pending.take() {
                    Some(k) => Block::Body(k),
                    None if matches!(self.stack.last(), Some(Block::Body(BodyKind::Service))) => {
                        Block::Endpoint
                    }
                    None => Block::Nested,
                };
                self.stack.push(block);
            }
            Tok::Punct('}') => {
                if matches!(self.stack.pop(), Some(Block::Import)) {
                    self.use_stmt = None;
                }
            }
            Tok::Punct('(') => {
                let ann = match self.prev.as_slice() {
                    [.., Tok::Punct('@'), Tok::Ident(n)] => Some(n.clone()),
                    _ => None,
                };
                self.stack
                    .push(ann.map_or(Block::Params, Block::AnnotationArgs));
            }
            Tok::Punct(')') => {
                if matches!(
                    self.stack.last(),
                    Some(Block::Params | Block::AnnotationArgs(_))
                ) {
                    self.stack.pop();
                }
            }
            Tok::Ident(_) | Tok::Punct(_) | Tok::Arrow | Tok::Str | Tok::Num => {}
        }
    }
}

fn body_kind(w: &str) -> Option<BodyKind> {
    Some(match w {
        "model" => BodyKind::Model,
        "error" => BodyKind::Error,
        "enum" => BodyKind::Enum,
        "union" => BodyKind::Union,
        "service" => BodyKind::Service,
        _ => return None,
    })
}

/// The tokens of the statement the cursor is in.
fn statement(items: &[Item], st: &State, cursor_line: usize) -> Vec<Tok> {
    let line_scoped = matches!(st.stack.last(), Some(Block::Body(_)) | None);
    let mut out: Vec<Tok> = Vec::new();
    for it in items.iter().rev() {
        let boundary = matches!(it.tok, Tok::Punct('{' | '}' | ';'))
            || (!matches!(st.stack.last(), Some(Block::Import)) && it.tok == Tok::Punct(','))
            || (matches!(
                st.stack.last(),
                Some(Block::Params | Block::AnnotationArgs(_))
            ) && it.tok == Tok::Punct('('));
        if boundary || (line_scoped && it.line != cursor_line) {
            break;
        }
        out.push(it.tok.clone());
    }
    out.reverse();
    out
}

/// Tokenises; the flag is true when the text ends inside a comment or an unclosed string.
fn lex(text: &str) -> (Vec<Item>, bool) {
    let chars: Vec<char> = text.chars().collect();
    let (mut items, mut i, mut line) = (Vec::new(), 0usize, 0usize);
    while i < chars.len() {
        let c = chars[i];
        match c {
            '\n' => {
                line += 1;
                i += 1;
            }
            c if c.is_whitespace() => i += 1,
            '/' if chars.get(i + 1) == Some(&'/') => {
                while i < chars.len() && chars[i] != '\n' {
                    i += 1;
                }
                if i >= chars.len() {
                    return (items, true);
                }
            }
            '"' => {
                i += 1;
                while i < chars.len() && chars[i] != '"' && chars[i] != '\n' {
                    i += if chars[i] == '\\' { 2 } else { 1 };
                }
                if i >= chars.len() || chars[i] == '\n' {
                    return (items, i >= chars.len());
                }
                i += 1;
                items.push(Item {
                    tok: Tok::Str,
                    line,
                });
            }
            '-' if chars.get(i + 1) == Some(&'>') => {
                items.push(Item {
                    tok: Tok::Arrow,
                    line,
                });
                i += 2;
            }
            c if c.is_ascii_digit() => {
                while i < chars.len() && (chars[i].is_ascii_alphanumeric() || chars[i] == '.') {
                    i += 1;
                }
                items.push(Item {
                    tok: Tok::Num,
                    line,
                });
            }
            c if is_word(c) => {
                let start = i;
                while i < chars.len() && is_word(chars[i]) {
                    i += 1;
                }
                let w: String = chars[start..i].iter().collect();
                items.push(Item {
                    tok: Tok::Ident(w),
                    line,
                });
            }
            c => {
                items.push(Item {
                    tok: Tok::Punct(c),
                    line,
                });
                i += 1;
            }
        }
    }
    (items, false)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> Tok {
        Tok::Ident(s.into())
    }

    #[test]
    fn top_level_and_bodies() {
        let c = scan("use a.{B}\nmod");
        assert!(c.at_top_level && c.partial == "mod" && c.stmt.is_empty());
        let c = scan("model A {\n  x: ");
        assert_eq!(c.block, Some(Block::Body(BodyKind::Model)));
        assert_eq!(c.stmt, vec![id("x"), Tok::Punct(':')]);
        let c = scan("model A {\n  x: int\n  y: Us");
        assert_eq!(c.stmt, vec![id("y"), Tok::Punct(':')]);
        assert_eq!(c.partial, "Us");
    }

    #[test]
    fn nested_blocks() {
        let c = scan("service S at \"/s\" {\n  get \"/\" f(a: int, b: ");
        assert_eq!(c.block, Some(Block::Params));
        assert_eq!(c.stmt, vec![id("b"), Tok::Punct(':')]);
        let c = scan("service S at \"/s\" {\n  get \"/\" f() -> A | ");
        assert_eq!(c.block, Some(Block::Body(BodyKind::Service)));
        assert!(c.stmt.contains(&Tok::Arrow));
        let c = scan("model A {\n  x: int @min(");
        assert_eq!(c.block, Some(Block::AnnotationArgs("min".into())));
        let c = scan("model A {\n  x: int @");
        assert_eq!(c.stmt.last(), Some(&Tok::Punct('@')));
    }

    #[test]
    fn use_statements() {
        let c = scan("use iam:users.{User, ");
        assert_eq!(c.block, Some(Block::Import));
        assert_eq!(
            c.use_stmt.unwrap(),
            vec![
                id("use"),
                id("iam"),
                Tok::Punct(':'),
                id("users"),
                Tok::Punct('.')
            ]
        );
        let c = scan("use iam:us");
        assert!(c.at_top_level);
        assert_eq!(c.partial, "us");
        assert_eq!(
            c.use_stmt.unwrap(),
            vec![id("use"), id("iam"), Tok::Punct(':')]
        );
        assert!(scan("use a.{B}\n").use_stmt.is_none());
        assert!(scan("use iam:us\nmodel M").use_stmt.is_none());
    }

    #[test]
    fn comments_and_strings_are_inert() {
        assert!(scan("model A {\n  // hello wor").inert);
        assert!(scan("service S at \"/ab").inert);
        assert!(!scan("service S at \"/ab\" ").inert);
        assert!(!scan("model A {\n  // c\n  x: ").inert);
    }

    #[test]
    fn braces_inside_strings_do_not_open_blocks() {
        let c = scan("service S at \"/s\" {\n  get \"/{id}\" f(");
        assert_eq!(c.block, Some(Block::Params));
    }

    #[test]
    fn an_endpoint_block_is_a_block_of_its_own_and_closes_cleanly() {
        let c = scan(
            "service S at \"/s\" {\n  post \"/\" f(a: int) -> A {\n    require a > 0 else E { x: 1 }\n    ",
        );
        assert_eq!(c.block, Some(Block::Endpoint));
        let c = scan("service S at \"/s\" {\n  post \"/\" f() -> A {\n    require true\n  }\n  ");
        assert_eq!(c.block, Some(Block::Body(BodyKind::Service)));
        let c =
            scan("service S at \"/s\" {\n  post \"/\" f() -> A {\n    require true else E { x: ");
        assert_eq!(c.block, Some(Block::Nested));
    }
}
