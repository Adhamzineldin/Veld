//! Byte offsets <-> LSP positions (line, UTF-16 column).

use lsp_types::{Position, Range};

/// Line table of one text.
#[derive(Debug, Clone, Default)]
pub struct LineIndex {
    text: String,
    starts: Vec<usize>,
}

impl LineIndex {
    #[must_use]
    pub fn new(text: &str) -> Self {
        let mut starts = vec![0];
        starts.extend(text.match_indices('\n').map(|(i, _)| i + 1));
        Self {
            text: text.to_string(),
            starts,
        }
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn position(&self, offset: usize) -> Position {
        let off = offset.min(self.text.len());
        let line = self.starts.partition_point(|&s| s <= off).saturating_sub(1);
        let start = self.starts.get(line).copied().unwrap_or(0);
        let slice = floor_slice(&self.text, start, off);
        let col: usize = slice.chars().map(char::len_utf16).sum();
        Position::new(to_u32(line), to_u32(col))
    }

    #[must_use]
    pub fn range(&self, start: usize, end: usize) -> Range {
        Range::new(self.position(start), self.position(end))
    }

    /// Byte offset of a position; positions past the end of a line clamp to it.
    #[must_use]
    pub fn offset(&self, pos: Position) -> usize {
        let line = pos.line as usize;
        let Some(&start) = self.starts.get(line) else {
            return self.text.len();
        };
        let end = self
            .starts
            .get(line + 1)
            .map_or(self.text.len(), |e| e.saturating_sub(1).max(start));
        let line_text = floor_slice(&self.text, start, end);
        let mut units = 0usize;
        for (i, c) in line_text.char_indices() {
            if units >= pos.character as usize {
                return start + i;
            }
            units += c.len_utf16();
        }
        start + line_text.len()
    }

    /// Range of the whole text.
    #[must_use]
    pub fn full_range(&self) -> Range {
        self.range(0, self.text.len())
    }
}

fn floor_slice(text: &str, start: usize, end: usize) -> &str {
    text.get(start..end).unwrap_or("")
}

pub(crate) fn to_u32(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(u32::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_round_trip_with_multibyte_text() {
        let ix = LineIndex::new("ab\ncé😀x\n");
        assert_eq!(ix.position(0), Position::new(0, 0));
        assert_eq!(ix.position(3), Position::new(1, 0));
        // `c` 1 byte, `é` 2 bytes, `😀` 4 bytes and 2 UTF-16 units
        assert_eq!(ix.position(3 + 1 + 2 + 4), Position::new(1, 1 + 1 + 2));
        assert_eq!(ix.offset(Position::new(1, 4)), 3 + 1 + 2 + 4);
        assert_eq!(ix.offset(Position::new(1, 99)), 3 + 1 + 2 + 4 + 1);
        assert_eq!(ix.offset(Position::new(9, 0)), ix.text().len());
    }

    #[test]
    fn crlf_lines_clamp_before_the_terminator() {
        let ix = LineIndex::new("ab\r\ncd");
        assert_eq!(
            ix.offset(Position::new(0, 50)),
            3.min(ix.text().len()).max(2)
        );
        assert_eq!(ix.position(4), Position::new(1, 0));
    }

    #[test]
    fn full_range_ends_at_the_last_position() {
        let ix = LineIndex::new("a\nbc");
        assert_eq!(ix.full_range().end, Position::new(1, 2));
    }
}
