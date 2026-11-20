//! `file:` URIs <-> paths, and a comparison key that ignores drive-letter case.

use std::path::{Path, PathBuf};
use std::str::FromStr;

use lsp_types::Uri;

/// A stable map key for a path: forward slashes, lowercase on Windows.
#[must_use]
pub fn path_key(p: &Path) -> String {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s).to_string();
    if cfg!(windows) { s.to_lowercase() } else { s }
}

/// Percent-decodes `s`; invalid escapes are kept.
fn decode(s: &str) -> String {
    let bytes = s.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'%'
            && let Some(v) = bytes
                .get(i + 1..i + 3)
                .and_then(|h| std::str::from_utf8(h).ok())
                .and_then(|h| u8::from_str_radix(h, 16).ok())
        {
            out.push(v);
            i += 3;
            continue;
        }
        out.push(bytes[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// The file path of a `file:` URI; `None` for other schemes.
#[must_use]
pub fn uri_to_path(uri: &Uri) -> Option<PathBuf> {
    let rest = uri.as_str().strip_prefix("file://")?;
    let rest = rest.strip_prefix("localhost").unwrap_or(rest);
    let path = decode(rest);
    let bytes = path.as_bytes();
    let drive = bytes.len() >= 3 && bytes[0] == b'/' && bytes[2] == b':';
    Some(PathBuf::from(if drive { &path[1..] } else { &path }))
}

/// A `file:` URI for an absolute path.
#[must_use]
pub fn path_to_uri(p: &Path) -> Option<Uri> {
    let s = p.to_string_lossy().replace('\\', "/");
    let s = s.strip_prefix("//?/").unwrap_or(&s);
    let mut out = String::from("file://");
    if !s.starts_with('/') {
        out.push('/');
    }
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' | b'/' => {
                out.push(b as char);
            }
            b':' if out.len() == "file:///".len() + 1 => out.push(':'),
            _ => out.push_str(&format!("%{b:02X}")),
        }
    }
    Uri::from_str(&out).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_unix_and_windows_paths() {
        let u = Uri::from_str("file:///c%3A/Users/a%20b/x.veld").unwrap();
        let p = uri_to_path(&u).unwrap();
        assert_eq!(path_key(&p), path_key(Path::new("c:/Users/a b/x.veld")));
        let back = path_to_uri(Path::new("/home/a b/x.veld")).unwrap();
        assert_eq!(back.as_str(), "file:///home/a%20b/x.veld");
        assert_eq!(
            uri_to_path(&back).unwrap(),
            PathBuf::from("/home/a b/x.veld")
        );
        let win = path_to_uri(Path::new("C:\\w\\x.veld")).unwrap();
        assert_eq!(win.as_str(), "file:///C:/w/x.veld");
        assert!(uri_to_path(&Uri::from_str("untitled:Untitled-1").unwrap()).is_none());
    }
}
