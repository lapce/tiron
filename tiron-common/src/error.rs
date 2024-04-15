use std::{io::Write, ops::Range, path::PathBuf};

use anyhow::Result;

/// The runbook file path and content
pub struct Origin {
    pub cwd: PathBuf,
    pub path: PathBuf,
    pub data: String,
}

impl Origin {
    pub fn error(&self, message: impl Into<String>, span: &Option<Range<usize>>) -> Error {
        Error::new(message.into()).with_origin(self, span)
    }
}

pub struct Error {
    pub message: String,
    pub location: Option<ErrorLocation>,
}

pub struct ErrorLocation {
    pub path: PathBuf,
    pub line_content: String,
    pub line: usize,
    pub start_col: usize,
    pub end_col: usize,
}

impl Error {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            location: None,
        }
    }

    pub fn with_origin(mut self, origin: &Origin, span: &Option<Range<usize>>) -> Self {
        if let Some(span) = span {
            let line_begin = origin.data[..span.start]
                .as_bytes()
                .iter()
                .rev()
                .position(|&b| b == b'\n')
                .map_or(0, |pos| span.start - pos);

            let line_content = origin.data[line_begin..]
                .as_bytes()
                .iter()
                .position(|&b| b == b'\n')
                .map_or(&origin.data[line_begin..], |pos| {
                    &origin.data[line_begin..line_begin + pos]
                });

            let line = origin.data[..span.start]
                .as_bytes()
                .iter()
                .filter(|&&b| b == b'\n')
                .count()
                + 1;
            let start_col = span.start - line_begin + 1;
            let end_col = span.start - line_begin + span.len();
            self.location = Some(ErrorLocation {
                path: origin.path.clone(),
                line_content: line_content.to_string(),
                line,
                start_col,
                end_col,
            });
        }
        self
    }

    pub fn from_hcl(err: hcl_edit::parser::Error, path: PathBuf) -> Error {
        Error {
            message: err.message().to_string(),
            location: Some(ErrorLocation {
                path,
                line_content: err.line().to_string(),
                line: err.location().line(),
                start_col: err.location().column(),
                end_col: err.location().column(),
            }),
        }
    }

    pub fn err<T>(self) -> Result<T, Error> {
        Err(self)
    }

    pub fn report_stderr(&self) -> Result<()> {
        let mut result = Vec::new();
        result.push(Segment::from("Error: ").with_markup(Markup::Error));
        result.push(self.message.clone().into());
        result.push("\n".into());
        if let Some(location) = &self.location {
            let line_len = location.line.to_string().len();

            result.push(" ".repeat(line_len + 1).into());
            result.push(Segment::from("--> ").with_markup(Markup::Error));
            let path = location.path.to_string_lossy();
            result.push(path.as_ref().into());
            let line_col = format!(":{}:{}\n", location.line, location.start_col);
            result.push(line_col.as_str().into());

            result.push(" ".repeat(line_len + 2).into());
            result.push(Segment::from("╷\n").with_markup(Markup::Error));
            result.push(Segment::from(format!(" {} ", location.line)).with_markup(Markup::Error));
            result.push(Segment::from("│ ").with_markup(Markup::Error));
            result.push(location.line_content.clone().into());
            result.push("\n".into());
            result.push(" ".repeat(line_len + 2).into());
            result.push(Segment::from("╵").with_markup(Markup::Error));
            result.push(" ".repeat(location.start_col).into());
            result.push("^".into());
            for _ in location.start_col..location.end_col {
                result.push("~".into());
            }
            result.push("\n".into());
        }

        let stderr = std::io::stderr();
        let mut out = stderr.lock();
        let mut markup = Markup::None;
        for seg in result {
            if markup != seg.markup {
                markup = seg.markup;
                out.write_all(switch_ansi(markup).as_bytes())?;
            }
            out.write_all(seg.s.as_bytes())?;
        }

        std::process::exit(1);
    }
}

struct Segment {
    s: String,
    markup: Markup,
}

impl Segment {
    pub fn with_markup(mut self, markup: Markup) -> Self {
        self.markup = markup;
        self
    }
}

impl From<&str> for Segment {
    fn from(value: &str) -> Segment {
        Segment {
            s: value.to_string(),
            markup: Markup::None,
        }
    }
}

impl From<String> for Segment {
    fn from(value: String) -> Segment {
        Segment {
            s: value,
            markup: Markup::None,
        }
    }
}

/// A markup hint, used to apply color and other markup to output.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Markup {
    /// No special markup applied, default formatting.
    None,

    /// Used for error message reporting, styled in bold.
    Error,
    /// Used for error message reporting, styled in bold.
    Warning,
    /// Used for trace message reporting, styled in bold.
    Trace,

    /// Make something stand out in error messages.
    ///
    /// We use this to play a similar role as backticks in Markdown,
    /// to clarify visually where the boundaries of a quotation are.
    Highlight,

    // These are meant for syntax highlighting.
    Builtin,
    Comment,
    Escape,
    Field,
    Keyword,
    Number,
    String,
    Type,
}

/// Return the ANSI escape code to switch to style `markup`.
pub fn switch_ansi(markup: Markup) -> &'static str {
    let reset = "\x1b[0m";
    let bold_blue = "\x1b[34;1m";
    let bold_green = "\x1b[32;1m";
    let bold_red = "\x1b[31;1m";
    let bold_yellow = "\x1b[33;1m";
    let blue = "\x1b[34m";
    let cyan = "\x1b[36m";
    let magenta = "\x1b[35m";
    let red = "\x1b[31m";
    let white = "\x1b[37m";
    let yellow = "\x1b[33m";

    match markup {
        Markup::None => reset,
        Markup::Error => bold_red,
        Markup::Warning => bold_yellow,
        Markup::Trace => bold_blue,
        Markup::Highlight => white,
        Markup::Builtin => red,
        Markup::Comment => white,
        Markup::Field => blue,
        Markup::Keyword => bold_green,
        Markup::Number => cyan,
        Markup::String => red,
        Markup::Escape => yellow,
        Markup::Type => magenta,
    }
}
