use std::fmt;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    InvalidInput(String),
    LimitExceeded(String),
    DuplicateFormat(String),
    Malformed(String),
    DuplicateStrategy(String),
    UnknownStrategy(String),
    UnsupportedCombination(String),
    InvalidComposition(String),
}

impl Error {
    pub fn code(&self) -> &'static str {
        match self {
            Self::InvalidInput(_) => "ERR_INVALID_INPUT",
            Self::LimitExceeded(_) => "ERR_LIMIT_EXCEEDED",
            Self::DuplicateFormat(_) => "ERR_DUPLICATE_FORMAT",
            Self::Malformed(_) => "ERR_MALFORMED_BINARY",
            Self::DuplicateStrategy(_) => "ERR_DUPLICATE_STRATEGY",
            Self::UnknownStrategy(_) => "ERR_UNKNOWN_STRATEGY",
            Self::UnsupportedCombination(_) => "ERR_UNSUPPORTED_COMBINATION",
            Self::InvalidComposition(_) => "ERR_INVALID_COMPOSITION",
        }
    }
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let (Self::InvalidInput(s)
        | Self::LimitExceeded(s)
        | Self::DuplicateFormat(s)
        | Self::Malformed(s)
        | Self::DuplicateStrategy(s)
        | Self::UnknownStrategy(s)
        | Self::UnsupportedCombination(s)
        | Self::InvalidComposition(s)) = self;
        write!(f, "{}: {}", self.code(), s)
    }
}

impl std::error::Error for Error {}
