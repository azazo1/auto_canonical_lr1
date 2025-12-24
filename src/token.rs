use std::fmt::{Debug, Display};

#[derive(PartialEq, Eq, Clone, Hash, Copy, PartialOrd, Ord)]
pub struct Terminal<'a> {
    ident: &'a str,
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
    pub fn as_str(&self) -> &str {
        self.ident
    }
}

#[derive(Clone, Copy, Hash, PartialOrd, Ord)]
pub enum Token<'a> {
    Terminal(Terminal<'a>),
    NonTerminal(NonTerminal<'a>),
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

impl Token<'_> {
    pub fn as_str(&self) -> &str {
        match self {
            Self::Terminal(t) => t.as_str(),
            Self::NonTerminal(nt) => nt.as_str(),
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
