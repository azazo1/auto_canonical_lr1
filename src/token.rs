use std::fmt::{Debug, Display};

#[derive(PartialEq, Eq, Clone, Hash, Copy)]
pub struct Terminal<'a> {
    ident: &'a str,
}

impl PartialOrd for Terminal<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

/// ..., EPSILON, EOF 的顺序, 其中 ... 的排序是短的在前(字节数量), 同样长度的按照字符串排序.
impl Ord for Terminal<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering::*;
        match (*self, *other) {
            (EOF, EOF) => Equal,
            (_, EOF) => Less,
            (EOF, _) => Greater,
            (EPSILON, EPSILON) => Equal,
            (_, EPSILON) => Less,
            (EPSILON, _) => Greater,
            (this, other) => this
                .ident
                .len()
                .cmp(&other.ident.len())
                .then(this.ident.cmp(other.ident)),
        }
    }
}

impl Debug for Terminal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!(r#"t{:?}"#, self.ident))
    }
}

impl Display for Terminal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.ident)
    }
}

impl<'a> From<&'a str> for Terminal<'a> {
    fn from(ident: &'a str) -> Self {
        Terminal { ident }
    }
}

impl<'a> Terminal<'a> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.ident
    }
}

#[derive(PartialEq, Eq, Clone, Hash, Copy, PartialOrd, Ord)]
pub struct NonTerminal<'a> {
    ident: &'a str,
}

impl Debug for NonTerminal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!(r#"nt{:?}"#, self.ident))
    }
}

impl Display for NonTerminal<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(self.ident)
    }
}

pub const EPSILON: Terminal<'static> = Terminal { ident: "E" };
pub const EOF: Terminal<'static> = Terminal { ident: "eof" };

impl<'a> From<&'a str> for NonTerminal<'a> {
    fn from(ident: &'a str) -> Self {
        Self { ident }
    }
}

impl<'a> NonTerminal<'a> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        self.ident
    }
}

#[derive(Clone, Copy, Hash)]
pub enum Token<'a> {
    Terminal(Terminal<'a>),
    NonTerminal(NonTerminal<'a>),
}

impl PartialOrd for Token<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Token<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        use std::cmp::Ordering;
        match (self, other) {
            (Self::Terminal(t1), Self::Terminal(t2)) => t1.cmp(t2),
            (Self::Terminal(_), Self::NonTerminal(_)) => Ordering::Less,
            (Self::NonTerminal(_), Self::Terminal(_)) => Ordering::Greater,
            (Self::NonTerminal(nt1), Self::NonTerminal(nt2)) => nt1.cmp(nt2),
        }
    }
}

impl Debug for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(arg0) => f.pad(&format!("{:?}", arg0)),
            Self::NonTerminal(arg0) => f.pad(&format!("{:?}", arg0)),
        }
    }
}

impl Display for Token<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Terminal(arg0) => f.pad(&format!("{}", arg0)),
            Self::NonTerminal(arg0) => f.pad(&format!("{}", arg0)),
        }
    }
}

impl PartialEq for Token<'_> {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::Terminal(l0), Self::Terminal(r0)) => l0 == r0,
            (Self::NonTerminal(l0), Self::NonTerminal(r0)) => l0 == r0,
            _ => false,
        }
    }
}

impl Eq for Token<'_> {}

impl<'a> Token<'a> {
    #[must_use]
    pub fn as_str(&self) -> &str {
        match self {
            Self::Terminal(t) => t.as_str(),
            Self::NonTerminal(nt) => nt.as_str(),
        }
    }

    #[must_use]
    pub fn is_term(&self) -> bool {
        matches!(self, Token::Terminal(_))
    }

    #[must_use]
    pub fn is_non_term(&self) -> bool {
        matches!(self, Token::NonTerminal(_))
    }

    #[must_use]
    pub fn as_term(&self) -> Option<&Terminal<'a>> {
        match self {
            Self::Terminal(t) => Some(t),
            Self::NonTerminal(_) => None,
        }
    }

    #[must_use]
    pub fn as_non_term(&self) -> Option<&NonTerminal<'a>> {
        match self {
            Self::Terminal(_) => None,
            Self::NonTerminal(nt) => Some(nt),
        }
    }
}

impl<'a> From<Terminal<'a>> for Token<'a> {
    fn from(value: Terminal<'a>) -> Self {
        Self::Terminal(value)
    }
}

impl<'a> From<NonTerminal<'a>> for Token<'a> {
    fn from(value: NonTerminal<'a>) -> Self {
        Self::NonTerminal(value)
    }
}
