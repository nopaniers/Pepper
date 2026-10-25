use std::fmt;

/// A half-open range of UTF-8 byte offsets into the original source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    pub message: String,
    pub span: Span,
}

impl Diagnostic {
    pub fn new(message: impl Into<String>, span: Span) -> Self {
        Self {
            message: message.into(),
            span,
        }
    }

    pub fn render(&self, filename: &str, source: &str) -> String {
        let mut start = self.span.start.min(source.len());
        while !source.is_char_boundary(start) {
            start -= 1;
        }
        let prefix = &source[..start];
        let line = prefix.bytes().filter(|b| *b == b'\n').count() + 1;
        let column = prefix.rsplit('\n').next().unwrap_or("").chars().count() + 1;
        let text = source[start - prefix.rsplit('\n').next().unwrap_or("").len()..]
            .lines()
            .next()
            .unwrap_or("");
        format!(
            "{filename}:{line}:{column}: error: {}\n  |\n{line:>2} | {text}\n  | {}^",
            self.message,
            " ".repeat(column - 1)
        )
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (bytes {}..{})",
            self.message, self.span.start, self.span.end
        )
    }
}
impl std::error::Error for Diagnostic {}
