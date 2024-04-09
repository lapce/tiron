use rcl::source::Span;

pub struct Error {
    pub span: Option<Span>,
    pub msg: String,
}

impl Error {
    pub fn new(msg: impl Into<String>, span: Option<Span>) -> Self {
        Self {
            msg: msg.into(),
            span,
        }
    }

    pub fn err<T>(self) -> Result<T, Error> {
        Err(self)
    }
}
