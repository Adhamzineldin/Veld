//! A format-preserving editor for `package.json`: adds one entry to a top-level object
//! (`dependencies`) without touching any other byte of the file.
//!
//! It scans the text, never re-serialises it, so key order, indentation, line endings and
//! unrelated keys survive. Anything it does not understand is an [`EditError`], and the caller
//! prints the line to add by hand instead.

/// What an edit did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Change {
    /// The entry was added.
    Added,
    /// The entry existed with another value; it now has the new one.
    Updated { from: String },
    /// The entry was already there with this value.
    Unchanged,
}

/// The text is not a JSON object this editor can edit safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EditError(pub String);

impl std::fmt::Display for EditError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn bad<T>(msg: &str) -> Result<T, EditError> {
    Err(EditError(msg.to_string()))
}

/// One `"key": value` member with the byte spans of its key (with quotes) and value.
#[derive(Debug, Clone)]
struct Member {
    key: String,
    key_start: usize,
    value_start: usize,
    value_end: usize,
}

/// An object: the offsets of `{` and `}` and its members.
#[derive(Debug, Clone)]
struct Object {
    open: usize,
    close: usize,
    members: Vec<Member>,
}

fn skip_ws(b: &[u8], mut i: usize) -> usize {
    while i < b.len() && matches!(b[i], b' ' | b'\t' | b'\r' | b'\n') {
        i += 1;
    }
    i
}

/// End (exclusive) of the string starting at `i` (a `"`), and its decoded content.
fn scan_string(text: &str, i: usize) -> Result<(usize, String), EditError> {
    let b = text.as_bytes();
    let mut j = i + 1;
    while j < b.len() {
        match b[j] {
            b'\\' => j += 2,
            b'"' => {
                let value: String = serde_json::from_str(&text[i..=j])
                    .map_err(|_| EditError("invalid string in package.json".to_string()))?;
                return Ok((j + 1, value));
            }
            _ => j += 1,
        }
    }
    bad("unterminated string in package.json")
}

/// End (exclusive) of the JSON value starting at `i`.
fn skip_value(text: &str, i: usize) -> Result<usize, EditError> {
    let b = text.as_bytes();
    match b.get(i) {
        Some(b'"') => Ok(scan_string(text, i)?.0),
        Some(b'{' | b'[') => {
            let mut depth = 0usize;
            let mut j = i;
            while j < b.len() {
                match b[j] {
                    b'"' => {
                        j = scan_string(text, j)?.0;
                        continue;
                    }
                    b'{' | b'[' => depth += 1,
                    b'}' | b']' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Ok(j + 1);
                        }
                    }
                    _ => {}
                }
                j += 1;
            }
            bad("unterminated object or array in package.json")
        }
        Some(_) => {
            let mut j = i;
            while j < b.len() && !matches!(b[j], b',' | b'}' | b']' | b' ' | b'\t' | b'\r' | b'\n')
            {
                j += 1;
            }
            Ok(j)
        }
        None => bad("unexpected end of package.json"),
    }
}

/// Parses the object whose `{` is at `open`.
fn parse_object(text: &str, open: usize) -> Result<Object, EditError> {
    let b = text.as_bytes();
    let mut members = Vec::new();
    let mut i = skip_ws(b, open + 1);
    loop {
        match b.get(i) {
            Some(b'}') => {
                return Ok(Object {
                    open,
                    close: i,
                    members,
                });
            }
            Some(b'"') => {
                let (key_end, key) = scan_string(text, i)?;
                let colon = skip_ws(b, key_end);
                if b.get(colon) != Some(&b':') {
                    return bad("expected `:` in package.json");
                }
                let value_start = skip_ws(b, colon + 1);
                let value_end = skip_value(text, value_start)?;
                members.push(Member {
                    key,
                    key_start: i,
                    value_start,
                    value_end,
                });
                i = skip_ws(b, value_end);
                if b.get(i) == Some(&b',') {
                    i = skip_ws(b, i + 1);
                }
            }
            _ => {
                return bad("package.json is not a plain JSON object (comments or trailing text?)");
            }
        }
    }
}

fn parse_root(text: &str) -> Result<Object, EditError> {
    let bom = if text.starts_with('\u{feff}') { 3 } else { 0 };
    let start = skip_ws(text.as_bytes(), bom);
    if text.as_bytes().get(start) != Some(&b'{') {
        return bad("package.json does not start with `{`");
    }
    parse_object(text, start)
}

/// The whitespace between the start of the line and `at`, if the line holds nothing else before it.
fn line_indent(text: &str, at: usize) -> Option<String> {
    let line_start = text[..at].rfind('\n').map_or(0, |n| n + 1);
    let lead = &text[line_start..at];
    lead.chars()
        .all(|c| c == ' ' || c == '\t')
        .then(|| lead.to_string())
}

/// The indentation unit of the file: that of the first top-level member (default two spaces).
fn indent_unit(text: &str, root: &Object) -> String {
    root.members
        .first()
        .and_then(|m| line_indent(text, m.key_start))
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "  ".to_string())
}

fn quote(s: &str) -> String {
    serde_json::to_string(s).unwrap_or_else(|_| format!("\"{s}\""))
}

/// Inserts `"key": value` into `obj`. `base` is the indentation of the line the object's `{`
/// is on, `unit` one indentation step. Keeps the keys sorted when they already are.
fn insert_member(
    text: &str,
    obj: &Object,
    (key, value): (&str, &str),
    (base, unit, nl): (&str, &str, &str),
    keep_sorted: bool,
) -> String {
    let entry = format!("{}: {value}", quote(key));
    let mut out = text.to_string();
    let Some(first) = obj.members.first() else {
        let inner = format!("{nl}{base}{unit}{entry}{nl}{base}");
        out.replace_range(obj.open + 1..obj.close, &inner);
        return out;
    };
    let last = &obj.members[obj.members.len() - 1];
    if !text[obj.open..first.key_start].contains('\n') {
        // `{ "a": 1 }` style: stay on the line.
        out.insert_str(last.value_end, &format!(", {entry}"));
        return out;
    }
    let indent = line_indent(text, first.key_start).unwrap_or_else(|| format!("{base}{unit}"));
    let sorted = keep_sorted && obj.members.windows(2).all(|w| w[0].key <= w[1].key);
    let before = if sorted {
        obj.members.iter().find(|m| m.key.as_str() > key)
    } else {
        None
    };
    match before {
        Some(m) => out.insert_str(m.key_start, &format!("{entry},{nl}{indent}")),
        None => out.insert_str(last.value_end, &format!(",{nl}{indent}{entry}")),
    }
    out
}

/// Sets `section.name = spec` (for example `dependencies."@veld/api" = "file:src/generated"`).
/// Creates `section` when the file has none.
pub fn set_entry(
    text: &str,
    section: &str,
    name: &str,
    spec: &str,
) -> Result<(String, Change), EditError> {
    let root = parse_root(text)?;
    let nl = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let unit = indent_unit(text, &root);
    let Some(sec) = root.members.iter().find(|m| m.key == section) else {
        let inner = format!(
            "{{{nl}{unit}{unit}{}: {}{nl}{unit}}}",
            quote(name),
            quote(spec)
        );
        let out = insert_member(text, &root, (section, &inner), ("", &unit, nl), false);
        return Ok((out, Change::Added));
    };
    if text.as_bytes().get(sec.value_start) != Some(&b'{') {
        return bad(&format!("`{section}` in package.json is not an object"));
    }
    let obj = parse_object(text, sec.value_start)?;
    if let Some(m) = obj.members.iter().find(|m| m.key == name) {
        if text.as_bytes().get(m.value_start) != Some(&b'"') {
            return bad(&format!(
                "`{section}.{name}` in package.json is not a string"
            ));
        }
        let (_, current) = scan_string(text, m.value_start)?;
        if current == spec {
            return Ok((text.to_string(), Change::Unchanged));
        }
        let mut out = text.to_string();
        out.replace_range(m.value_start..m.value_end, &quote(spec));
        return Ok((out, Change::Updated { from: current }));
    }
    let base = line_indent(text, sec.key_start).unwrap_or_default();
    let out = insert_member(text, &obj, (name, &quote(spec)), (&base, &unit, nl), true);
    Ok((out, Change::Added))
}

/// The current value of `section.name`, if the file is a readable package.json with it.
#[must_use]
#[cfg(test)]
pub fn get_entry(text: &str, section: &str, name: &str) -> Option<String> {
    let root = parse_root(text).ok()?;
    let sec = root.members.iter().find(|m| m.key == section)?;
    let obj = parse_object(text, sec.value_start).ok()?;
    let m = obj.members.iter().find(|m| m.key == name)?;
    scan_string(text, m.value_start).ok().map(|(_, v)| v)
}

#[cfg(test)]
mod tests {
    use super::*;

    const NAME: &str = "@veld/api";
    const SPEC: &str = "file:src/generated";

    fn add(text: &str) -> (String, Change) {
        set_entry(text, "dependencies", NAME, SPEC).unwrap()
    }

    #[test]
    fn adds_to_an_existing_sorted_section_in_place() {
        let text = "{\n  \"name\": \"app\",\n  \"dependencies\": {\n    \"express\": \"4.19.2\",\n    \"zod\": \"3.0.0\"\n  },\n  \"scripts\": {}\n}\n";
        let (out, change) = add(text);
        assert_eq!(change, Change::Added);
        assert_eq!(
            out,
            "{\n  \"name\": \"app\",\n  \"dependencies\": {\n    \"@veld/api\": \"file:src/generated\",\n    \"express\": \"4.19.2\",\n    \"zod\": \"3.0.0\"\n  },\n  \"scripts\": {}\n}\n"
        );
        let (out2, change2) = add(&out);
        assert_eq!((out2.as_str(), change2), (out.as_str(), Change::Unchanged));
    }

    #[test]
    fn appends_when_keys_are_not_sorted_and_keeps_tabs() {
        let text = "{\n\t\"dependencies\": {\n\t\t\"zod\": \"1\",\n\t\t\"express\": \"2\"\n\t}\n}";
        let (out, _) = add(text);
        assert_eq!(
            out,
            "{\n\t\"dependencies\": {\n\t\t\"zod\": \"1\",\n\t\t\"express\": \"2\",\n\t\t\"@veld/api\": \"file:src/generated\"\n\t}\n}"
        );
    }

    #[test]
    fn creates_the_section_and_keeps_every_other_byte() {
        let text = "{\n  \"name\": \"web\",\n  \"scripts\": {\n    \"dev\": \"vite\"\n  }\n}\n";
        let (out, change) = add(text);
        assert_eq!(change, Change::Added);
        assert_eq!(
            out,
            "{\n  \"name\": \"web\",\n  \"scripts\": {\n    \"dev\": \"vite\"\n  },\n  \"dependencies\": {\n    \"@veld/api\": \"file:src/generated\"\n  }\n}\n"
        );
        assert!(serde_json::from_str::<serde_json::Value>(&out).is_ok());
    }

    #[test]
    fn handles_empty_objects() {
        let (out, _) = add("{}");
        assert_eq!(
            out,
            "{\n  \"dependencies\": {\n    \"@veld/api\": \"file:src/generated\"\n  }\n}"
        );
        let (out, _) = add("{\n  \"dependencies\": {}\n}\n");
        assert_eq!(
            out,
            "{\n  \"dependencies\": {\n    \"@veld/api\": \"file:src/generated\"\n  }\n}\n"
        );
    }

    #[test]
    fn updates_a_changed_value_and_reports_the_old_one() {
        let text = "{\n  \"dependencies\": {\n    \"@veld/api\": \"file:old\",\n    \"x\": \"1\"\n  }\n}\n";
        let (out, change) = add(text);
        assert_eq!(
            change,
            Change::Updated {
                from: "file:old".into()
            }
        );
        assert!(out.contains("\"@veld/api\": \"file:src/generated\",\n    \"x\": \"1\""));
    }

    #[test]
    fn keeps_crlf_line_endings() {
        let text = "{\r\n  \"dependencies\": {\r\n    \"a\": \"1\"\r\n  }\r\n}\r\n";
        let (out, _) = add(text);
        assert!(out.contains("\r\n    \"a\": \"1\""), "{out:?}");
        assert!(!out.replace("\r\n", "").contains('\n'), "{out:?}");
    }

    #[test]
    fn single_line_objects_stay_on_one_line() {
        let (out, _) = add("{ \"dependencies\": { \"a\": \"1\" } }");
        assert_eq!(
            out,
            "{ \"dependencies\": { \"a\": \"1\", \"@veld/api\": \"file:src/generated\" } }"
        );
    }

    #[test]
    fn braces_and_quotes_inside_strings_do_not_confuse_the_scanner() {
        let text = "{\n  \"description\": \"a } \\\" { b\",\n  \"dependencies\": {\n    \"a\": \"{\"\n  }\n}\n";
        let (out, _) = add(text);
        assert!(
            serde_json::from_str::<serde_json::Value>(&out).is_ok(),
            "{out}"
        );
        assert!(out.contains("\"description\": \"a } \\\" { b\""));
    }

    #[test]
    fn rejects_what_it_cannot_edit() {
        assert!(set_entry("[1]", "dependencies", NAME, SPEC).is_err());
        assert!(set_entry("{ // c\n}", "dependencies", NAME, SPEC).is_err());
        assert!(set_entry("{\"dependencies\": 3}", "dependencies", NAME, SPEC).is_err());
    }

    #[test]
    fn reads_an_entry() {
        let text = "{\"dependencies\": {\"@veld/api\": \"file:x\"}}";
        assert_eq!(
            get_entry(text, "dependencies", NAME).as_deref(),
            Some("file:x")
        );
        assert_eq!(get_entry(text, "dependencies", "nope"), None);
    }
}
