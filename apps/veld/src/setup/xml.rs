//! A tiny XML element scanner: just enough structure (element spans) to edit a `pom.xml`
//! in place without re-serialising it. Text, attributes and namespaces are not interpreted.

/// One element with the byte spans needed to insert text into it.
#[derive(Debug, Clone)]
pub struct Elem {
    pub name: String,
    /// Offset of `<`.
    pub open_start: usize,
    /// First byte after the start tag.
    pub content_start: usize,
    /// Offset of the end tag's `<`.
    pub content_end: usize,
    /// First byte after the end tag.
    pub end: usize,
    pub children: Vec<Elem>,
}

impl Elem {
    #[must_use]
    pub fn child(&self, name: &str) -> Option<&Elem> {
        self.children.iter().find(|c| c.name == name)
    }

    /// The trimmed text of a leaf element.
    #[must_use]
    pub fn text<'a>(&self, src: &'a str) -> &'a str {
        src[self.content_start..self.content_end].trim()
    }
}

/// The document is not well formed enough to edit safely.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct XmlError(pub String);

impl std::fmt::Display for XmlError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

fn err<T>(msg: &str) -> Result<T, XmlError> {
    Err(XmlError(msg.to_string()))
}

/// Parses `src` and returns its root element.
pub fn parse(src: &str) -> Result<Elem, XmlError> {
    let b = src.as_bytes();
    let mut i = 0usize;
    let mut stack: Vec<Elem> = Vec::new();
    let mut root: Option<Elem> = None;
    while i < b.len() {
        if b[i] != b'<' {
            i += 1;
            continue;
        }
        let rest = &src[i..];
        if rest.starts_with("<!--") {
            i = skip_to(src, i, "-->")?;
        } else if rest.starts_with("<![CDATA[") {
            i = skip_to(src, i, "]]>")?;
        } else if rest.starts_with("<?") {
            i = skip_to(src, i, "?>")?;
        } else if rest.starts_with("<!") {
            i = skip_to(src, i, ">")?;
        } else if rest.starts_with("</") {
            let close = skip_to(src, i, ">")?;
            let name = src[i + 2..close - 1].trim();
            let Some(mut el) = stack.pop() else {
                return err("unbalanced end tag");
            };
            if el.name != name {
                return err("mismatched end tag");
            }
            el.content_end = i;
            el.end = close;
            match stack.last_mut() {
                Some(parent) => parent.children.push(el),
                None => root = Some(el),
            }
            i = close;
        } else {
            let tag_end = tag_end(b, i)?;
            let self_closing = b[tag_end - 2] == b'/';
            let name: String = src[i + 1..tag_end - 1]
                .chars()
                .take_while(|c| !c.is_whitespace() && *c != '/' && *c != '>')
                .collect();
            let el = Elem {
                name,
                open_start: i,
                content_start: tag_end,
                content_end: tag_end,
                end: tag_end,
                children: Vec::new(),
            };
            if self_closing {
                match stack.last_mut() {
                    Some(parent) => parent.children.push(el),
                    None => root = Some(el),
                }
            } else {
                stack.push(el);
            }
            i = tag_end;
        }
    }
    if !stack.is_empty() {
        return err("unclosed element");
    }
    root.ok_or_else(|| XmlError("no root element".to_string()))
}

fn skip_to(src: &str, from: usize, terminator: &str) -> Result<usize, XmlError> {
    src[from..]
        .find(terminator)
        .map(|n| from + n + terminator.len())
        .ok_or_else(|| XmlError(format!("missing `{terminator}`")))
}

/// Offset just past the `>` of the start tag at `i`, skipping quoted attribute values.
fn tag_end(b: &[u8], i: usize) -> Result<usize, XmlError> {
    let mut j = i + 1;
    let mut quote = None;
    while j < b.len() {
        match (quote, b[j]) {
            (Some(q), c) if c == q => quote = None,
            (None, b'"' | b'\'') => quote = Some(b[j]),
            (None, b'>') => return Ok(j + 1),
            _ => {}
        }
        j += 1;
    }
    err("unterminated tag")
}

/// The whitespace between the start of the line and `at`, if the line holds nothing else before it.
#[must_use]
pub fn line_indent(src: &str, at: usize) -> Option<String> {
    let start = src[..at].rfind('\n').map_or(0, |n| n + 1);
    let lead = &src[start..at];
    lead.chars()
        .all(|c| c == ' ' || c == '\t')
        .then(|| lead.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_nested_elements_and_skips_comments_and_cdata() {
        let src = "<?xml version=\"1.0\"?>\n<!-- <fake/> -->\n<project a=\"x>y\">\n  <build>\n    <plugins/>\n    <![CDATA[ <no> ]]>\n  </build>\n</project>\n";
        let root = parse(src).unwrap();
        assert_eq!(root.name, "project");
        let build = root.child("build").unwrap();
        assert_eq!(build.children.len(), 1);
        assert_eq!(
            build.child("plugins").unwrap().content_start,
            build.child("plugins").unwrap().end
        );
    }

    #[test]
    fn leaf_text_is_trimmed() {
        let src = "<a><id> x </id></a>";
        let root = parse(src).unwrap();
        assert_eq!(root.child("id").unwrap().text(src), "x");
    }

    #[test]
    fn malformed_documents_are_errors() {
        for bad in ["<a><b></a>", "<a>", "", "</a>", "<a><!-- x </a>"] {
            assert!(parse(bad).is_err(), "{bad}");
        }
    }
}
