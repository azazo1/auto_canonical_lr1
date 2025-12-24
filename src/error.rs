#[derive(thiserror::Error, Debug)]
pub enum Error {
    #[error("Error parsing productions, line: {line}, cause: {cause}.")]
    ParseProductionError { line: usize, cause: &'static str },
    #[error("Grammar may be not augmented")]
    GrammarNotAugmented,
    #[error("First set state is calculating, maybe some errors occurred.")]
    InvalidFirstSetState,
    #[error("Grammar does not contain the non-terminal: {0}.")]
    NonTerminalNotFound(String),
    #[error("Grammar unresolvable first set, this should not present.")]
    UnresolvableFirstSet,
}

impl Error {
    pub(crate) fn parse_production_error(line: usize, cause: &'static str) -> Self {
        Self::ParseProductionError { line, cause }
    }
}
