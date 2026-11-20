//! Hand-written, error-tolerant lexer.

#![allow(
    clippy::wildcard_enum_match_arm,
    clippy::match_wildcard_for_single_variants,
    reason = "token scanning: only a few tokens matter, the rest are skipped"
)]

use crate::diagnostics::{Diagnostic, FileId, Span};

#[derive(Debug, Clone, PartialEq)]
pub enum Tok {
    Ident(String),
    Int(i64),
    Float(f64),
    Str(String),
    /// `/// text` with the marker and one leading space removed.
    Doc(String),
    LBrace,
    RBrace,
    LParen,
    RParen,
    LBracket,
    RBracket,
    Lt,
    Gt,
    Comma,
    Colon,
    Dot,
    Question,
    Eq,
    Pipe,
    At,
    Arrow,
    Star,
    Slash,
    Eof,
}

impl Tok {
    /// Human-readable description for "expected X, found Y" messages.
    #[must_use]
    pub fn describe(&self) -> String {
        match self {
            Tok::Ident(s) => format!("`{s}`"),
            Tok::Int(n) => format!("`{n}`"),
            Tok::Float(f) => format!("`{f}`"),
            Tok::Str(_) => "a string".to_string(),
            Tok::Doc(_) => "a doc comment".to_string(),
            Tok::Eof => "end of file".to_string(),
            other => format!("`{}`", other.punct()),
        }
    }

    fn punct(&self) -> &'static str {
        match self {
            Tok::LBrace => "{",
            Tok::RBrace => "}",
            Tok::LParen => "(",
            Tok::RParen => ")",
            Tok::LBracket => "[",
            Tok::RBracket => "]",
            Tok::Lt => "<",
            Tok::Gt => ">",
            Tok::Comma => ",",
            Tok::Colon => ":",
            Tok::Dot => ".",
            Tok::Question => "?",
            Tok::Eq => "=",
            Tok::Pipe => "|",
            Tok::At => "@",
            Tok::Arrow => "->",
            Tok::Star => "*",
            Tok::Slash => "/",
            _ => "",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub tok: Tok,
    pub span: Span,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CommentKind {
    Line,
    Block,
}

/// An ordinary comment, kept for the formatter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Comment {
    pub kind: CommentKind,
    pub text: String,
    pub span: Span,
}

#[derive(Debug, Clone)]
pub struct Lexed {
    pub tokens: Vec<Token>,
    pub comments: Vec<Comment>,
    pub diagnostics: Vec<Diagnostic>,
}

struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    pos: usize,
    file: FileId,
    out: Lexed,
}

/// Lexes `text`. Never fails: invalid input produces diagnostics and lexing continues.
#[must_use]
pub fn lex(text: &str, file: FileId) -> Lexed {
    let mut lx = Lexer {
        src: text,
        bytes: text.as_bytes(),
        pos: if text.starts_with('\u{feff}') { 3 } else { 0 },
        file,
        out: Lexed {
            tokens: Vec::new(),
            comments: Vec::new(),
            diagnostics: Vec::new(),
        },
    };
    lx.run();
    lx.out
}

impl Lexer<'_> {
    fn run(&mut self) {
        while self.pos < self.bytes.len() {
            let b = self.bytes[self.pos];
            match b {
                b' ' | b'\t' | b'\r' | b'\n' => self.pos += 1,
                b'/' if self.peek_at(1) == Some(b'/') => self.line_comment(),
                b'/' if self.peek_at(1) == Some(b'*') => self.block_comment(),
                b'"' => self.string(),
                b'0'..=b'9' => self.number(self.pos),
                b'-' if self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) => {
                    self.number(self.pos)
                }
                b'-' if self.peek_at(1) == Some(b'>') => self.push_n(Tok::Arrow, 2),
                b'A'..=b'Z' | b'a'..=b'z' | b'_' => self.ident(),
                _ => self.punct(b),
            }
        }
        let end = self.bytes.len();
        self.out.tokens.push(Token {
            tok: Tok::Eof,
            span: Span::new(self.file, end, end),
        });
    }

    fn peek_at(&self, n: usize) -> Option<u8> {
        self.bytes.get(self.pos + n).copied()
    }

    fn span(&self, start: usize, end: usize) -> Span {
        Span::new(self.file, start, end)
    }

    fn push_n(&mut self, tok: Tok, len: usize) {
        let span = self.span(self.pos, self.pos + len);
        self.out.tokens.push(Token { tok, span });
        self.pos += len;
    }

    fn error(&mut self, msg: impl Into<String>, start: usize, end: usize) {
        let span = self.span(start, end);
        self.out
            .diagnostics
            .push(Diagnostic::error("E0002", msg, span));
    }

    fn punct(&mut self, b: u8) {
        let tok = match b {
            b'{' => Tok::LBrace,
            b'}' => Tok::RBrace,
            b'(' => Tok::LParen,
            b')' => Tok::RParen,
            b'[' => Tok::LBracket,
            b']' => Tok::RBracket,
            b'<' => Tok::Lt,
            b'>' => Tok::Gt,
            b',' => Tok::Comma,
            b':' => Tok::Colon,
            b'.' => Tok::Dot,
            b'?' => Tok::Question,
            b'=' => Tok::Eq,
            b'|' => Tok::Pipe,
            b'@' => Tok::At,
            b'*' => Tok::Star,
            b'/' => Tok::Slash,
            _ => return self.invalid_char(),
        };
        self.push_n(tok, 1);
    }

    fn invalid_char(&mut self) {
        let ch = self
            .src
            .get(self.pos..)
            .and_then(|s| s.chars().next())
            .unwrap_or('\u{fffd}');
        let len = ch.len_utf8();
        self.error(
            format!("unexpected character `{ch}`"),
            self.pos,
            self.pos + len,
        );
        self.pos += len;
    }

    fn line_comment(&mut self) {
        let start = self.pos;
        let end = self.bytes[start..]
            .iter()
            .position(|&b| b == b'\n')
            .map_or(self.bytes.len(), |i| start + i);
        let raw = self.src.get(start..end).unwrap_or("");
        let raw_trim = raw.trim_end_matches('\r');
        let is_doc = raw_trim.starts_with("///") && !raw_trim.starts_with("////");
        if is_doc {
            let body = &raw_trim[3..];
            let text = body.strip_prefix(' ').unwrap_or(body).to_string();
            self.out.tokens.push(Token {
                tok: Tok::Doc(text),
                span: self.span(start, start + raw_trim.len()),
            });
        } else {
            let text = raw_trim[2..].to_string();
            self.out.comments.push(Comment {
                kind: CommentKind::Line,
                text,
                span: self.span(start, start + raw_trim.len()),
            });
        }
        self.pos = end;
    }

    fn block_comment(&mut self) {
        let start = self.pos;
        let rest = self.src.get(start + 2..).unwrap_or("");
        match rest.find("*/") {
            Some(i) => {
                let end = start + 2 + i + 2;
                let text = self
                    .src
                    .get(start + 2..start + 2 + i)
                    .unwrap_or("")
                    .to_string();
                self.out.comments.push(Comment {
                    kind: CommentKind::Block,
                    text,
                    span: self.span(start, end),
                });
                self.pos = end;
            }
            None => {
                self.error("unterminated block comment", start, start + 2);
                self.pos = self.bytes.len();
            }
        }
    }

    fn ident(&mut self) {
        let start = self.pos;
        while self.pos < self.bytes.len()
            && (self.bytes[self.pos].is_ascii_alphanumeric() || self.bytes[self.pos] == b'_')
        {
            self.pos += 1;
        }
        let name = self.src.get(start..self.pos).unwrap_or("").to_string();
        self.out.tokens.push(Token {
            tok: Tok::Ident(name),
            span: self.span(start, self.pos),
        });
    }

    fn digits(&mut self) {
        while self.pos < self.bytes.len() && self.bytes[self.pos].is_ascii_digit() {
            self.pos += 1;
        }
    }

    fn number(&mut self, start: usize) {
        if self.bytes[self.pos] == b'-' {
            self.pos += 1;
        }
        self.digits();
        let mut is_float = false;
        if self.peek_at(0) == Some(b'.') && self.peek_at(1).is_some_and(|c| c.is_ascii_digit()) {
            is_float = true;
            self.pos += 1;
            self.digits();
            if matches!(self.peek_at(0), Some(b'e' | b'E')) {
                self.exponent();
            }
        }
        let text = self.src.get(start..self.pos).unwrap_or("0");
        let tok = if is_float {
            self.float(text, start)
        } else {
            match text.parse::<i64>() {
                Ok(n) => Tok::Int(n),
                Err(_) => {
                    self.error("integer literal is out of range", start, self.pos);
                    Tok::Int(0)
                }
            }
        };
        self.out.tokens.push(Token {
            tok,
            span: self.span(start, self.pos),
        });
    }

    /// Parses a float literal. A literal that overflows to infinity has no JSON representation
    /// (it would serialise as `null`), so it is rejected here, wherever a literal is allowed.
    fn float(&mut self, text: &str, start: usize) -> Tok {
        match text.parse::<f64>() {
            Ok(f) if f.is_finite() => Tok::Float(f),
            _ => {
                let span = self.span(start, self.pos);
                self.out.diagnostics.push(Diagnostic::error(
                    "E0305",
                    format!(
                        "float literal `{text}` is out of range: it is not a finite 64-bit float"
                    ),
                    span,
                ));
                Tok::Float(0.0)
            }
        }
    }

    fn exponent(&mut self) {
        let save = self.pos;
        self.pos += 1;
        if matches!(self.peek_at(0), Some(b'+' | b'-')) {
            self.pos += 1;
        }
        if self.peek_at(0).is_some_and(|c| c.is_ascii_digit()) {
            self.digits();
        } else {
            self.pos = save;
        }
    }

    fn string(&mut self) {
        let start = self.pos;
        self.pos += 1;
        let mut value = String::new();
        loop {
            let Some(ch) = self.src.get(self.pos..).and_then(|s| s.chars().next()) else {
                self.error("unterminated string", start, start + 1);
                break;
            };
            match ch {
                '"' => {
                    self.pos += 1;
                    break;
                }
                '\n' => {
                    self.error("unterminated string", start, start + 1);
                    break;
                }
                '\\' => self.escape(&mut value),
                _ => {
                    value.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
        self.out.tokens.push(Token {
            tok: Tok::Str(value),
            span: self.span(start, self.pos),
        });
    }

    fn escape(&mut self, value: &mut String) {
        let esc_start = self.pos;
        self.pos += 1;
        let Some(c) = self.src.get(self.pos..).and_then(|s| s.chars().next()) else {
            return;
        };
        self.pos += c.len_utf8();
        match c {
            '\\' => value.push('\\'),
            '"' => value.push('"'),
            'n' => value.push('\n'),
            't' => value.push('\t'),
            'r' => value.push('\r'),
            'u' => self.unicode_escape(value, esc_start),
            '\n' => {
                self.pos -= 1;
                self.error("unterminated string", esc_start, esc_start + 1);
            }
            other => self.error(format!("unknown escape `\\{other}`"), esc_start, self.pos),
        }
    }

    fn unicode_escape(&mut self, value: &mut String, esc_start: usize) {
        let rest = self.src.get(self.pos..).unwrap_or("");
        let parsed = rest
            .strip_prefix('{')
            .and_then(|r| r.find('}').map(|i| (&r[..i], i + 2)));
        let Some((hex, consumed)) = parsed else {
            self.error("expected `\\u{XXXX}`", esc_start, self.pos);
            return;
        };
        self.pos += consumed;
        match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
            Some(c) if !hex.is_empty() && hex.len() <= 6 => value.push(c),
            _ => self.error("invalid unicode escape", esc_start, self.pos),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn toks(s: &str) -> Vec<Tok> {
        lex(s, FileId(0))
            .tokens
            .into_iter()
            .map(|t| t.tok)
            .collect()
    }

    #[test]
    fn lexes_punctuation_and_arrow() {
        assert_eq!(
            toks("-> | ? : < > , . @ * / { } ( ) [ ] ="),
            vec![
                Tok::Arrow,
                Tok::Pipe,
                Tok::Question,
                Tok::Colon,
                Tok::Lt,
                Tok::Gt,
                Tok::Comma,
                Tok::Dot,
                Tok::At,
                Tok::Star,
                Tok::Slash,
                Tok::LBrace,
                Tok::RBrace,
                Tok::LParen,
                Tok::RParen,
                Tok::LBracket,
                Tok::RBracket,
                Tok::Eq,
                Tok::Eof
            ]
        );
    }

    #[test]
    fn lexes_numbers() {
        assert_eq!(
            toks("1 -2 3.5 -0.25 1e3 4.0e-2"),
            vec![
                Tok::Int(1),
                Tok::Int(-2),
                Tok::Float(3.5),
                Tok::Float(-0.25),
                Tok::Int(1),
                Tok::Ident("e3".into()),
                Tok::Float(0.04),
                Tok::Eof
            ]
        );
    }

    #[test]
    fn int_dot_ident_is_not_a_float() {
        assert_eq!(
            toks("2.x"),
            vec![Tok::Int(2), Tok::Dot, Tok::Ident("x".into()), Tok::Eof]
        );
    }

    #[test]
    fn lexes_string_escapes() {
        assert_eq!(
            toks(r#""a\n\t\"\\\u{e9}""#),
            vec![Tok::Str("a\n\t\"\\\u{e9}".into()), Tok::Eof]
        );
    }

    #[test]
    fn doc_comments_are_tokens_and_others_are_side_table() {
        let l = lex("/// doc\n// line\n/* block */ x //// four", FileId(0));
        assert_eq!(l.tokens[0].tok, Tok::Doc("doc".into()));
        assert_eq!(l.tokens[1].tok, Tok::Ident("x".into()));
        assert_eq!(l.comments.len(), 3);
        assert_eq!(l.comments[0].kind, CommentKind::Line);
        assert_eq!(l.comments[1].kind, CommentKind::Block);
        assert_eq!(l.comments[1].text, " block ");
        assert_eq!(l.comments[2].text, "// four");
    }

    #[test]
    fn invalid_character_reports_and_continues() {
        let l = lex("a # b $ c", FileId(0));
        assert_eq!(l.diagnostics.len(), 2);
        assert_eq!(l.diagnostics[0].code, "E0002");
        let idents = l
            .tokens
            .iter()
            .filter(|t| matches!(t.tok, Tok::Ident(_)))
            .count();
        assert_eq!(idents, 3);
    }

    #[test]
    fn unterminated_string_stops_at_newline() {
        let l = lex("\"abc\nx", FileId(0));
        assert_eq!(l.diagnostics.len(), 1);
        assert!(l.tokens.iter().any(|t| t.tok == Tok::Ident("x".into())));
    }

    #[test]
    fn unknown_escape_and_unterminated_block_comment() {
        assert_eq!(lex(r#""\q""#, FileId(0)).diagnostics.len(), 1);
        assert_eq!(lex("/* open", FileId(0)).diagnostics.len(), 1);
    }

    #[test]
    fn bom_is_ignored_and_crlf_is_whitespace() {
        let l = lex("\u{feff}a\r\nb", FileId(0));
        assert!(l.diagnostics.is_empty());
        assert_eq!(l.tokens[0].span.start, 3);
        assert_eq!(l.tokens.len(), 3);
    }

    #[test]
    fn lone_minus_is_an_error() {
        assert_eq!(lex("a - b", FileId(0)).diagnostics.len(), 1);
    }

    #[test]
    fn integer_overflow_is_reported() {
        assert_eq!(lex("99999999999999999999", FileId(0)).diagnostics.len(), 1);
    }
}
