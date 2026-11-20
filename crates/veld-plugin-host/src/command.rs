//! The `command` of a plugin: an executable plus optional arguments, resolved to a real file.

use std::path::{Path, PathBuf};

/// A parsed `command`: the first token is the executable (whose SHA-256 is pinned), the
/// rest are arguments. Double quotes group a token that contains spaces.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandLine {
    pub program: String,
    pub args: Vec<String>,
}

impl CommandLine {
    /// Splits `text`; `None` when it holds no token.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        let mut tokens = tokenize(text);
        if tokens.is_empty() {
            return None;
        }
        let program = tokens.remove(0);
        Some(Self { program, args: tokens })
    }

    #[must_use]
    pub fn from_parts(program: impl Into<String>, args: Vec<String>) -> Self {
        Self { program: program.into(), args }
    }

    /// Finds the executable: a path (absolute, or relative to `root`) or a name on `PATH`.
    pub fn resolve(&self, root: &Path) -> Result<PathBuf, String> {
        let p = &self.program;
        if p.contains('/') || p.contains('\\') {
            let path = if Path::new(p).is_absolute() { PathBuf::from(p) } else { root.join(p) };
            return with_extension_fallback(&path).ok_or_else(|| format!("executable `{p}` not found"));
        }
        search_path(p).ok_or_else(|| format!("executable `{p}` not found on PATH"))
    }
}

fn tokenize(text: &str) -> Vec<String> {
    let (mut tokens, mut cur, mut quoted, mut any) = (Vec::new(), String::new(), false, false);
    for c in text.chars() {
        match c {
            '"' => {
                quoted = !quoted;
                any = true;
            }
            c if c.is_whitespace() && !quoted => {
                if any {
                    tokens.push(std::mem::take(&mut cur));
                    any = false;
                }
            }
            c => {
                cur.push(c);
                any = true;
            }
        }
    }
    if any {
        tokens.push(cur);
    }
    tokens
}

fn search_path(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path).find_map(|dir| with_extension_fallback(&dir.join(name)))
}

/// `path` itself when it is a file; on Windows also `path` plus each `PATHEXT` extension.
fn with_extension_fallback(path: &Path) -> Option<PathBuf> {
    if path.is_file() {
        return Some(path.to_path_buf());
    }
    if cfg!(windows) {
        let exts = std::env::var("PATHEXT").unwrap_or_else(|_| ".COM;.EXE;.BAT;.CMD".into());
        for ext in exts.split(';').filter(|e| !e.is_empty()) {
            let mut name = path.as_os_str().to_os_string();
            name.push(ext);
            let candidate = PathBuf::from(name);
            if candidate.is_file() {
                return Some(candidate);
            }
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_splits_and_honours_quotes() {
        let c = CommandLine::parse(r#"node "dir with space/main.js" --x"#).unwrap();
        assert_eq!(c.program, "node");
        assert_eq!(c.args, ["dir with space/main.js", "--x"]);
        assert_eq!(CommandLine::parse("  tool  ").unwrap().args.len(), 0);
        assert!(CommandLine::parse("   ").is_none());
        assert_eq!(CommandLine::parse(r#""a b""#).unwrap().program, "a b");
    }

    #[test]
    fn resolves_relative_paths_against_the_root() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("plug"), "x").unwrap();
        let c = CommandLine::parse("./plug").unwrap();
        assert_eq!(c.resolve(tmp.path()).unwrap(), tmp.path().join("./plug"));
        assert!(CommandLine::parse("./missing").unwrap().resolve(tmp.path()).is_err());
        assert!(CommandLine::parse("veld-definitely-not-a-program").unwrap().resolve(tmp.path()).is_err());
    }
}
