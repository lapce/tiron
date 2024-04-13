use std::{ops::Range, path::PathBuf};

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
            let start_col = span.start - line_begin;
            let end_col = start_col + span.len();
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
}
