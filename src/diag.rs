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

    #[test]
    fn multibyte_characters_do_not_shift_line_starts() {
        // The em-dash is three bytes, so line 2 starts at 5, not 3.
        assert_eq!(starts("a\u{2014}\nb"), vec![0, 5]);
    }
}
