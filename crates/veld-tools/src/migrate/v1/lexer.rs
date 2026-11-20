//! Lexer for the v1 contract syntax (`internal/lexer/lexer.go`).
//!
//! Unlike the v1 tool, it keeps comments, because the migration carries them
//! over.

#![allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "token scanning: only a few tokens matter, the rest are skipped"
)]

/// A v1 token.
#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Str(String),
    Num(String),
    /// A bare path such as `/accounts/{id}`.
    Path(String),
    LBrace,
    RBrace,
    LBracket,
    RBracket,
    LParen,
    RParen,
    Lt,
    Gt,
    Colon,
    Comma,
    Question,
    At,
    Pipe,
    Star,
    Eq,
    Eof,
}

impl Tok {
    pub fn describe(&self) -> String {
        match self {
            Tok::Ident(s) | Tok::Str(s) | Tok::Num(s) | Tok::Path(s) => format!("`{s}`"),
            Tok::Eof => "end of file".to_string(),
            other => format!("`{}`", other.punct()),
        }
    }

    fn punct(&self) -> &'static str {
        match self {
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Colon => ":",
            Tok::Comma => ",",
            Tok::Question => "?",
            Tok::At => "@",
            Tok::Pipe => "|",
            Tok::Star => "*",
            Tok::Eq => "=",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub line: usize,
}

/// A comment with the line it starts on. `text` has the comment markers removed.
#[derive(Debug, Clone, PartialEq)]
pub struct Comment {
    pub line: usize,
    pub text: String,
}

#[derive(Debug, Default)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
    pub errors: Vec<String>,
}

struct Lexer {
    src: Vec<char>,
    pos: usize,
    line: usize,
    out: Lexed,
}

pub fn lex(source: &str) -> Lexed {
    let mut lx = Lexer {
        src: source.chars().collect(),
        pos: 0,
        line: 1,
        out: Lexed::default(),
    };
    lx.run();
    lx.out
}

impl Lexer {
    fn peek(&self, n: usize) -> Option<char> {
        self.src.get(self.pos + n).copied()
    }

    fn push(&mut self, tok: Tok) {
        self.out.tokens.push(Token {
            tok,
            line: self.line,
        });
    }

    fn run(&mut self) {
        while let Some(ch) = self.peek(0) {
            match ch {
                '\n' => {
                    self.line += 1;
                    self.pos += 1;
                }
                c if c.is_whitespace() => self.pos += 1,
                '/' if self.peek(1) == Some('*') => self.block_comment(),
                '/' if self.peek(1) == Some('/') => self.line_comment(),
                '"' => self.string(),
                '/' => self.path(),
                c if c.is_ascii_digit()
                    || (c == '-' && self.peek(1).is_some_and(|d| d.is_ascii_digit())) =>
                {
                    self.number();
                }
                c if c.is_alphabetic() || c == '_' => self.ident(),
                c => self.punct(c),
            }
        }
        self.push(Tok::Eof);
    }

    fn punct(&mut self, c: char) {
        let tok = match c {
            '{' => Tok::LBrace,
            '}' => Tok::RBrace,
            '[' => Tok::LBracket,
            ']' => Tok::RBracket,
            '(' => Tok::LParen,
            ')' => Tok::RParen,
            '<' => Tok::Lt,
            '>' => Tok::Gt,
            ':' => Tok::Colon,
            ',' => Tok::Comma,
            '?' => Tok::Question,
            '@' => Tok::At,
            '|' => Tok::Pipe,
            '*' => Tok::Star,
            '=' => Tok::Eq,
            other => {
                self.out.errors.push(format!(
                    "line {}: unexpected character `{other}`",
                    self.line
                ));
                self.pos += 1;
                return;
            }
        };
        self.push(tok);
        self.pos += 1;
    }

    fn line_comment(&mut self) {
        let start = self.pos + 2;
        let mut end = start;
        while end < self.src.len() && self.src[end] != '\n' {
            end += 1;
        }
        let text: String = self.src[start..end].iter().collect();
        self.out.comments.push(Comment {
            line: self.line,
            text: text.trim_end_matches('\r').to_string(),
        });
        self.pos = end;
    }

    fn block_comment(&mut self) {
        let start_line = self.line;
        self.pos += 2;
        let mut text = String::new();
        let mut closed = false;
        while self.pos < self.src.len() {
            if self.src[self.pos] == '*' && self.peek(1) == Some('/') {
                self.pos += 2;
                closed = true;
                break;
            }
            if self.src[self.pos] == '\n' {
                self.line += 1;
            }
            text.push(self.src[self.pos]);
            self.pos += 1;
        }
        if !closed {
            self.out.errors.push(format!(
                "line {start_line}: unterminated block comment; v1 treated the rest of the file as a comment (`/**` in an import starts one)"
            ));
        }
        self.out.comments.push(Comment {
            line: start_line,
            text,
        });
    }

    fn string(&mut self) {
        self.pos += 1;
        let start = self.pos;
        while self.pos < self.src.len() && self.src[self.pos] != '"' && self.src[self.pos] != '\n' {
            self.pos += 1;
        }
        let value: String = self.src[start..self.pos].iter().collect();
        if self.peek(0) == Some('"') {
            self.pos += 1;
        }
        self.push(Tok::Str(value));
    }

    /// A path runs to the next whitespace. A `{param}` segment may not contain
    /// whitespace, and an unmatched `}` ends the path (it closes a block).
    fn path(&mut self) {
        let start = self.pos;
        while let Some(c) = self.peek(0) {
            if c.is_whitespace() {
                break;
            }
            match c {
                '{' => self.path_param(),
                '}' => break,
                _ => self.pos += 1,
            }
        }
        let value: String = self.src[start..self.pos].iter().collect();
        self.push(Tok::Path(value));
    }

    fn path_param(&mut self) {
        self.pos += 1;
        while let Some(c) = self.peek(0) {
            if c == '}' || c.is_whitespace() {
                break;
            }
            self.pos += 1;
        }
        if self.peek(0) == Some('}') {
            self.pos += 1;
        }
    }

    fn number(&mut self) {
        let start = self.pos;
        if self.peek(0) == Some('-') {
            self.pos += 1;
        }
        while self.peek(0).is_some_and(|c| c.is_ascii_digit() || c == '.') {
            self.pos += 1;
        }
        let value: String = self.src[start..self.pos].iter().collect();
        self.push(Tok::Num(value));
    }

    fn ident(&mut self) {
        let start = self.pos;
        while self
            .peek(0)
            .is_some_and(|c| c.is_alphanumeric() || c == '_')
        {
            self.pos += 1;
        }
        let value: String = self.src[start..self.pos].iter().collect();
        self.push(Tok::Ident(value));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(src: &str) -> Vec<Tok> {
        lex(src).tokens.into_iter().map(|t| t.tok).collect()
    }

    #[test]
    fn paths_keep_brace_params_and_stop_at_block_braces() {
        assert_eq!(
            toks("path: /a/{id}/b }"),
            vec![
                Tok::Ident("path".into()),
                Tok::Colon,
                Tok::Path("/a/{id}/b".into()),
                Tok::RBrace,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn comments_are_collected_with_their_lines() {
        let l = lex("// one\nmodel A {} /* two\nlines */\n");
        assert_eq!(l.comments.len(), 2);
        assert_eq!(
            l.comments[0],
            Comment {
                line: 1,
                text: " one".into()
            }
        );
        assert_eq!(l.comments[1].line, 2);
        assert!(l.comments[1].text.contains("lines"));
    }

    #[test]
    fn numbers_and_strings() {
        assert_eq!(
            toks("-5 3.25 \"hi\""),
            vec![
                Tok::Num("-5".into()),
                Tok::Num("3.25".into()),
                Tok::Str("hi".into()),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn unexpected_characters_are_reported_and_skipped() {
        let l = lex("a # b");
        assert_eq!(l.errors.len(), 1);
        assert_eq!(l.tokens.len(), 3);
    }
}
