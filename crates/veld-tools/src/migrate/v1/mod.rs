//! A small, self-contained v1 front end: just enough lexer and parser to read
//! every contract the v1 tool accepted, so the migration needs no v1 code.

pub mod ast;
pub mod lexer;
mod parser;

#[cfg(test)]
mod tests;

pub use ast::V1File;

/// A parsed v1 file plus the lexer problems that did not stop parsing.
#[derive(Debug)]
pub struct Parsed {
    pub file: V1File,
    pub warnings: Vec<String>,
}

/// Parses one v1 source file.
///
/// # Errors
/// Returns a `line N: message` text for the first syntax error.
pub fn parse(source: &str) -> Result<Parsed, String> {
    let lexed = lexer::lex(source);
    let mut p = parser::Parser::new(lexed.tokens, lexed.comments);
    let file = p.parse_file()?;
    Ok(Parsed { file, warnings: lexed.errors })
}
