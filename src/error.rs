#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum Error {
    #[error("Error parsing productions, line: {line}, cause: {cause:?}.")]
    ParseProductionError {
        line: usize,
        cause: ParseProductionError,
    },
    #[error("Grammar may be not augmented")]
    GrammarNotAugmented,
    #[error("First set state is calculating, maybe some errors occurred.")]
    InvalidFirstSetState,
    #[error("Grammar does not contain the non-terminal: {0}.")]
    NonTerminalNotFound(String),
    #[error("Grammar unresolvable first set, this should not present.")]
    UnresolvableFirstSet,
}

#[derive(thiserror::Error, Debug, Eq, PartialEq)]
pub enum ParseProductionError {
    #[error("No arrow in production line")]
    NoArrow,
    #[error("Expected terminal, found non-terminal: {0}")]
    TokenTypeMisMatch(String),
    #[error("Start symbol not found")]
    StartSymbolNotFound,
}

impl Error {
    pub(crate) fn parse_production_error(line: usize, cause: ParseProductionError) -> Self {
        Self::ParseProductionError { line, cause }
    }
}
