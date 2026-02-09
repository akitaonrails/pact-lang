use crate::lexer::Span;
use std::fmt;

#[derive(Debug, Clone)]
pub struct Diagnostic {
    pub kind: DiagnosticKind,
    pub message: String,
    pub span: Option<Span>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticKind {
    Error,
    Warning,
}

impl Diagnostic {
    pub fn error(message: impl Into<String>, span: Option<Span>) -> Self {
        Diagnostic {
            kind: DiagnosticKind::Error,
            message: message.into(),
            span,
        }
    }

    pub fn warning(message: impl Into<String>, span: Option<Span>) -> Self {
        Diagnostic {
            kind: DiagnosticKind::Warning,
            message: message.into(),
            span,
        }
    }
}

impl fmt::Display for Diagnostic {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let prefix = match self.kind {
            DiagnosticKind::Error => "error",
            DiagnosticKind::Warning => "warning",
        };
        if let Some(span) = &self.span {
            write!(f, "{}: {} (at byte {}..{})", prefix, self.message, span.start, span.end)
        } else {
            write!(f, "{}: {}", prefix, self.message)
        }
    }
}

pub fn format_diagnostics(source: &str, diagnostics: &[Diagnostic]) -> String {
    let mut output = String::new();
    for diag in diagnostics {
        let prefix = match diag.kind {
            DiagnosticKind::Error => "error",
            DiagnosticKind::Warning => "warning",
        };
        if let Some(span) = &diag.span {
            let (line, col) = byte_to_line_col(source, span.start);
            output.push_str(&format!("{}:{}:{}: {}: {}\n", "<input>", line, col, prefix, diag.message));
            if let Some(line_str) = get_source_line(source, line) {
                output.push_str(&format!("  | {}\n", line_str));
                output.push_str(&format!("  | {}^\n", " ".repeat(col.saturating_sub(1))));
            }
        } else {
            output.push_str(&format!("{}: {}\n", prefix, diag.message));
        }
    }
    output
}

fn byte_to_line_col(source: &str, byte_offset: usize) -> (usize, usize) {
    let mut line = 1;
    let mut col = 1;
    for (i, ch) in source.char_indices() {
        if i >= byte_offset {
            break;
        }
        if ch == '\n' {
            line += 1;
            col = 1;
        } else {
            col += 1;
        }
    }
    (line, col)
}

fn get_source_line(source: &str, line_number: usize) -> Option<&str> {
    source.lines().nth(line_number.saturating_sub(1))
}
