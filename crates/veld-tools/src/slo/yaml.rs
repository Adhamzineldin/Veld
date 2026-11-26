//! A YAML writer for the fixed shape of the OpenSLO documents: nested mappings and lists of
//! one-member-per-line mappings, two spaces per level. Every string scalar is double-quoted, so
//! nothing a name or a query contains can be read as YAML syntax.

/// A document under construction.
pub struct Doc {
    lines: Vec<String>,
    depth: usize,
    /// For each open list: whether its next member starts a new item (`- `).
    items: Vec<bool>,
}

/// `"text"` with the escapes of a YAML double-quoted scalar (the JSON ones).
#[must_use]
pub fn quote(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 || c == '\u{7f}' => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

impl Doc {
    #[must_use]
    pub fn new() -> Doc {
        Doc {
            lines: Vec::new(),
            depth: 0,
            items: Vec::new(),
        }
    }

    fn pad(&self) -> String {
        "  ".repeat(self.depth)
    }

    fn push(&mut self, text: String) {
        let pad = self.pad();
        self.lines.push(format!("{pad}{text}"));
    }

    /// `key: "value"`.
    pub fn string(&mut self, key: &str, value: &str) {
        self.push(format!("{key}: {}", quote(value)));
    }

    /// `key: value` for a number or a boolean, written as it is.
    pub fn raw(&mut self, key: &str, value: &str) {
        self.push(format!("{key}: {value}"));
    }

    /// `key:` and a deeper level.
    pub fn open(&mut self, key: &str) {
        self.push(format!("{key}:"));
        self.depth += 1;
    }

    pub fn close(&mut self) {
        self.depth = self.depth.saturating_sub(1);
    }

    /// `key:` and a list whose items follow with `item_*`.
    pub fn list_open(&mut self, key: &str) {
        self.push(format!("{key}:"));
        self.depth += 1;
        self.items.push(true);
    }

    pub fn list_close(&mut self) {
        self.depth = self.depth.saturating_sub(1);
        self.items.pop();
    }

    /// A member of the current list item: the first one starts it with `- `.
    fn item(&mut self, text: String) {
        let first = self.items.last().copied().unwrap_or(false);
        let pad = self.pad();
        if first {
            self.lines.push(format!("{pad}- {text}"));
        } else {
            self.lines.push(format!("{pad}  {text}"));
        }
        if let Some(f) = self.items.last_mut() {
            *f = false;
        }
    }

    pub fn item_string(&mut self, key: &str, value: &str) {
        self.item(format!("{key}: {}", quote(value)));
    }

    pub fn item_raw(&mut self, key: &str, value: &str) {
        self.item(format!("{key}: {value}"));
    }

    /// The text of the document, with a final newline.
    #[must_use]
    pub fn finish(self) -> String {
        let mut text = self.lines.join("\n");
        text.push('\n');
        text
    }
}

impl Default for Doc {
    fn default() -> Self {
        Doc::new()
    }
}
