use std::path::Path;

use colored::Colorize;
use strum::Display as EnumDisplay;

use super::{directive::Stream, span::Span};

#[derive(Debug, PartialEq, Clone)]
pub struct Diagnostic {
    pub span: Span,
    pub kind: DiagKind,
}

impl Diagnostic {
    pub fn emit(&self, raw: &str, path: &Path) {
        assert!(self.span.end <= raw.len());

        let (line_number, line_start) = self.get_line_info(raw);
        let line_number = line_number.to_string();
        let line_end = raw
            .get(line_start..)
            .and_then(|rest| rest.find('\n').map(|offset| line_start + offset))
            .unwrap_or(raw.len());
        let header = format!("{}: {}", "ERROR".red().bold(), self.kind).bold();
        let file_path = format!(
            " {}{} {}:{line_number}",
            " ".repeat(line_number.len()),
            "-->".bold().blue(),
            path.display()
        );
        let prefix = format!(" {} |", " ".repeat(line_number.len()))
            .blue()
            .bold();
        let details = format!(
            "{}{}",
            format!(" {line_number} | ").blue().bold(),
            raw.get(line_start..line_end).unwrap()
        );
        let caret_width = raw
            .get(self.span.start..self.span.end)
            .map_or(1, |text| text.chars().count().max(1));
        let leading = raw
            .get(line_start..self.span.start)
            .map_or(0, |text| text.chars().count());
        let highlight = format!(
            "{prefix} {}{}",
            " ".repeat(leading),
            "^".repeat(caret_width).red()
        );
        eprintln!("{header}\n{file_path}\n{prefix}\n{details}\n{highlight}\n{prefix}");
    }

    fn get_line_info(&self, raw: &str) -> (usize, usize) {
        raw.char_indices()
            .take_while(|(index, _)| index < &self.span.start)
            .fold((1, 0), |(line, line_start), (index, ch)| {
                if ch == '\n' {
                    (line + 1, index + 1)
                } else {
                    (line, line_start)
                }
            })
    }
}

#[derive(Debug, PartialEq, Clone, EnumDisplay)]
pub enum DiagKind {
    #[strum(to_string = "not a directive (plain comments must use /* */)")]
    UnknownDirective,
    #[strum(to_string = "expected {expected}")]
    Expected { expected: &'static str },
    #[strum(to_string = "unterminated string")]
    UnterminatedString,
    #[strum(to_string = "unknown escape '\\{0}'")]
    UnknownEscape(char),
    #[strum(to_string = "unknown signal name")]
    UnknownSignal,
    #[strum(to_string = "not a valid integer")]
    InvalidInteger,
    #[strum(to_string = "duplicate ARGS directive")]
    DuplicateArgs,
    #[strum(to_string = "duplicate STATUS directive")]
    DuplicateStatus,
    #[strum(to_string = "duplicate NO-TTY directive for {0}")]
    DuplicateNoTty(Stream),
    #[strum(to_string = "exit code out of range (0-255)")]
    ExitCodeRange,
    #[strum(to_string = "signal number out of range (1-64)")]
    SignalRange,
}
