//! Spans and diagnostic rendering.
//!
//! `Span` is a byte range: `Copy`, `u32` fields, merged with `to()`. Write it
//! first — the lexer cannot emit tokens without it. The rendering half can wait
//! until the lexer is producing real errors to point at.
//!
//! `docs/implementation.md` covers why byte offsets rather than line/column, and
//! the character-versus-byte trap in column counting.

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub start: u32,
    pub end: u32,
}

#[derive(Debug)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

pub struct SourceFile {
    pub name: String,
    text: String,
    line_starts: Vec<u32>,
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineColumn {
    pub line: u32,
    pub col: u32,
}

impl Span {
    pub fn to(self, other: Span) -> Span {
        Span {
            start: self.start,
            end: other.end,
        }
    }
}

impl SourceFile {
    pub fn new(name: String, text: String) -> Self {
        let line_starts = Self::compute_line_starts(&text);
        SourceFile {
            name,
            text,
            line_starts,
        }
    }

    pub fn text(&self) -> &str {
        &self.text
    }

    pub fn line_column(&self, offset: u32) -> LineColumn {
        let line = self.line_starts.partition_point(|&s| s <= offset) - 1;
        let ls = self.line_starts[line] as usize;
        let col = self.text[ls..offset as usize].chars().count();

        LineColumn {
            line: line as u32 + 1,
            col: col as u32 + 1,
        }
    }

    pub fn line_text(&self, line: u32) -> &str {
        let i = (line - 1) as usize;
        let start = self.line_starts[i] as usize;
        let end = if i + 1 < self.line_starts.len() {
            self.line_starts[i + 1] as usize
        } else {
            self.text.len()
        };
        self.text[start..end]
            .trim_end_matches('\n')
            .trim_end_matches('\r')
    }

    fn compute_line_starts(text: &str) -> Vec<u32> {
        let mut starts = vec![0];
        for (i, b) in text.bytes().enumerate() {
            if b == b'\n' {
                starts.push(i as u32 + 1);
            }
        }
        starts
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn starts(text: &str) -> Vec<u32> {
        SourceFile::compute_line_starts(text)
    }

    #[test]
    fn empty_source_has_one_line() {
        assert_eq!(starts(""), vec![0]);
    }

    #[test]
    fn single_line_without_newline() {
        assert_eq!(starts("cash >= 300"), vec![0]);
    }

    #[test]
    fn counts_from_after_each_newline() {
        //          0123 4567
        assert_eq!(starts("ab\ncd"), vec![0, 3]);
    }

    #[test]
    fn trailing_newline_opens_an_empty_last_line() {
        assert_eq!(starts("ab\n"), vec![0, 3]);
    }

    // ---- line_column ----

    /// Line 1 is 18 bytes, line 2 holds a three-byte em-dash, so line starts
    /// land at 0, 19, 33 and 48.
    fn sample() -> SourceFile {
        SourceFile::new(
            "sample.vy".into(),
            "rule build-power {\n  // \u{2014} here\n  require cash\n}".into(),
        )
    }

    fn at(src: &SourceFile, offset: u32) -> (u32, u32) {
        let lc = src.line_column(offset);
        (lc.line, lc.col)
    }

    #[test]
    fn first_byte_is_line_one_column_one() {
        // Also the `- 1` boundary: partition_point returns 1 here, never 0.
        assert_eq!(at(&sample(), 0), (1, 1));
    }

    #[test]
    fn offset_exactly_on_a_line_start() {
        assert_eq!(at(&sample(), 19), (2, 1));
    }

    #[test]
    fn column_counts_characters_not_bytes() {
        // Byte subtraction would say column 9; the em-dash is 3 bytes, 1 char.
        assert_eq!(at(&sample(), 27), (2, 7));
    }

    #[test]
    fn last_line_without_trailing_newline() {
        assert_eq!(at(&sample(), 48), (4, 1));
    }

    #[test]
    fn offset_at_end_of_file_does_not_panic() {
        // An "unexpected end of file" diagnostic carries exactly this span.
        let src = sample();
        let eof = src.text().len() as u32;
        assert_eq!(at(&src, eof), (4, 2));
    }

    #[test]
    fn empty_source_at_offset_zero() {
        let src = SourceFile::new("empty.vy".into(), String::new());
        assert_eq!(at(&src, 0), (1, 1));
    }

    // ---- line_text ----

    #[test]
    fn line_text_returns_the_line() {
        assert_eq!(sample().line_text(1), "rule build-power {");
    }

    #[test]
    fn line_text_excludes_the_trailing_newline() {
        // A rendered caret block gets a stray blank line otherwise.
        assert_eq!(sample().line_text(3), "  require cash");
    }

    #[test]
    fn line_text_handles_the_last_line() {
        assert_eq!(sample().line_text(4), "}");
    }

    #[test]
    fn multibyte_characters_do_not_shift_line_starts() {
        // The em-dash is three bytes, so line 2 starts at 5, not 3.
        assert_eq!(starts("a\u{2014}\nb"), vec![0, 5]);
    }
}
