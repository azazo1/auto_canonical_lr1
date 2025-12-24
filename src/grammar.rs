use bumpalo::Bump;
use std::{
    cell::RefCell,
    collections::{BTreeSet, HashMap, HashSet},
    fmt::{Debug, Display},
};

use crate::{
    NonTerminal, Terminal, Token,
    error::{Error, ParseProductionError},
    token::{EOF, EPSILON},
};

#[derive(Clone, Hash, PartialOrd, Ord)]
pub struct Production<'a> {
    // 产生式 `->` 左侧内容.
    head: NonTerminal<'a>,
    // 产生式 `->` 右侧内容.
    tail: Vec<Token<'a>>,
}

impl Debug for Production<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("Production")
            .field(&format_args!(
                "{:?} -> {}",
                self.head,
                self.tail
                    .iter()
                    .map(|t| format!("{:?} ", t))
                    .collect::<String>()
                    .trim_end()
            ))
            .finish()
    }
}

impl Display for Production<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&format!(
            "{} -> {}",
            self.head,
            self.tail
                .iter()
                .map(|t| format!("{} ", t))
                .collect::<String>()
                .trim_end()
        ))
    }
}

impl PartialEq for Production<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.head == other.head && self.tail == other.tail
    }
}

impl Eq for Production<'_> {}

impl<'a> Production<'a> {
    #[must_use]
    pub fn new(head: NonTerminal<'a>, tail: Vec<Token<'a>>) -> Self {
        Self { head, tail }
    }

    #[must_use]
    pub fn head(&self) -> NonTerminal<'a> {
        self.head
    }

    #[must_use]
    pub fn tail(&self) -> &[Token<'a>] {
        &self.tail
    }

    pub fn tail_without_eps(&self) -> impl Iterator<Item = &Token<'a>> {
        self.tail
            .iter()
            .filter(|tok| !matches!(tok, Token::Terminal(EPSILON)))
    }

    /// 产生式尾部的 tokens 数量, [`EPSILON`] 不算长度.
    #[must_use]
    pub fn len(&self) -> usize {
        self.tail_without_eps().count()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[derive(Debug, Clone, Default)]
enum FirstSet<'a> {
    Presense(HashSet<Terminal<'a>>),
    #[default]
    Calculating,
    NotPresense,
}

#[derive(Debug, Clone)]
pub struct Grammar<'a> {
    bump: &'a Bump,
    prods: Vec<&'a Production<'a>>,
    prod_indexes: HashMap<&'a Production<'a>, usize>,
    tokens: BTreeSet<Token<'a>>,
    start: NonTerminal<'a>,
    /// 缓存的各个非终结符的 first 集,
    /// 在 [`Grammar`] 创建的时候为每个 [`NonTerminal`] 初始化为 [`FirstSet::None`],
    first_sets: HashMap<NonTerminal<'a>, RefCell<FirstSet<'a>>>,
}

impl PartialEq for Grammar<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.prods == other.prods && self.start == other.start && self.tokens == other.tokens
    }
}

impl Eq for Grammar<'_> {}

impl<'a> Grammar<'a> {
    #[must_use]
    pub(crate) fn bump(&self) -> &Bump {
        self.bump
    }

    /// 按产生式编号遍历产生式.
    pub fn prods(&self) -> &[&'a Production<'a>] {
        &self.prods
    }

    /// 获取产生式的编号, 如果产生式在文法中不存在, 那么返回 [`None`].
    #[must_use]
    pub fn index_of_prod(&self, prod: &Production<'a>) -> Option<usize> {
        self.prod_indexes.get(prod).copied()
    }

    #[must_use]
    pub fn symbol_start(&self) -> NonTerminal<'a> {
        self.start
    }

    #[must_use]
    pub fn tokens(&self) -> &BTreeSet<Token<'a>> {
        &self.tokens
    }

    #[must_use]
    pub fn augmented(mut self) -> Self {
        let new_start = self.bump.alloc(format!("{}prime", self.start.as_str()));
        let augmented_start = NonTerminal::from(new_start.as_str());
        self.prod_indexes.values_mut().for_each(|x| *x += 1);
        let augmented_prod = &*self
            .bump
            .alloc(Production::new(augmented_start, vec![self.start.into()]));
        self.prods.insert(0, augmented_prod);
        self.prod_indexes.insert(augmented_prod, 0);
        self.tokens.insert(augmented_start.into());
        self.first_sets
            .insert(augmented_start, RefCell::new(FirstSet::NotPresense));
        Self {
            bump: self.bump,
            prods: self.prods,
            prod_indexes: self.prod_indexes,
            tokens: self.tokens,
            start: augmented_start,
            first_sets: self.first_sets,
        }
    }

    pub fn from_cfg(s: &'a str, start: NonTerminal<'a>, bump: &'a Bump) -> Result<Self, Error> {
        let mut tokens: BTreeSet<Token<'_>> = [EPSILON.into(), EOF.into()].into();
        let mut non_terminals = HashSet::new();
        let mut splitted: Vec<(&str, &str)> = Vec::new();
        // 找出所有的非终结符.
        for (line_num, line) in s
            .lines()
            .enumerate()
            .filter(|(_, s)| !s.is_empty() && s.chars().any(|c| !c.is_whitespace()))
        {
            let parts = line.split_once("->").ok_or(Error::parse_production_error(
                line_num,
                ParseProductionError::NoArrow,
            ))?;
            let head_ident = parts.0.trim();
            splitted.push((head_ident, parts.1));
            non_terminals.insert(head_ident);
            tokens.insert(NonTerminal::from(head_ident).into());
        }
        // 验证是否有起始符.
        if !non_terminals.contains(&start.as_str()) {
            Err(Error::parse_production_error(
                0,
                ParseProductionError::StartSymbolNotFound,
            ))?
        }
        // 解析所有产生式.
        let mut prods = Vec::new();
        let mut prod_indexes = HashMap::new();
        for (head_ident, tails) in splitted {
            for tail_s in tails.split('|') {
                let tail = tail_s
                    .split_ascii_whitespace()
                    .map(|s| {
                        let s = s.trim();
                        if non_terminals.contains(&s) {
                            Token::from(NonTerminal::from(s))
                        } else {
                            Token::from(Terminal::from(s))
                        }
                    })
                    .inspect(|tok| {
                        tokens.insert(*tok);
                    })
                    .collect();
                let prod = &*bump.alloc(Production::new(NonTerminal::from(head_ident), tail));
                prod_indexes.insert(prod, prods.len());
                prods.push(prod);
            }
        }
        let first_sets = tokens
            .iter()
            .copied()
            .filter_map(|t| match t {
                Token::NonTerminal(nt) => Some(nt),
                _ => None,
            })
            .map(|t| (t, RefCell::new(FirstSet::NotPresense)))
            .collect();
        Ok(Grammar {
            prod_indexes,
            prods,
            start,
            bump,
            tokens,
            first_sets,
        })
    }

    /// 获取以某个非终结符为头部的所有产生式, 结果可能为空.
    #[must_use]
    pub(crate) fn prods_of(&self, nt: NonTerminal<'a>) -> HashSet<&'a Production<'a>> {
        self.prods
            .iter()
            .copied()
            .filter(|p| p.head == nt)
            .collect()
    }

    /// 计算一个非终结符的 first 集.
    /// # Parameters
    /// - `recalc`: 是否重新计算.
    /// # Returns
    /// (是否需要重新计算, first 集).
    fn calc_first(
        &self,
        nt: NonTerminal<'a>,
        recalc: bool,
    ) -> Result<(bool, HashSet<Terminal<'a>>), Error> {
        let mut first_set = self
            .first_sets
            .get(&nt)
            .ok_or(Error::NonTerminalNotFound(nt.as_str().to_string()))?
            .borrow_mut();
        match &*first_set {
            FirstSet::Calculating => Err(Error::InvalidFirstSetState)?,
            FirstSet::Presense(first_set) => {
                // 如果是正在重新计算, 那么跳过缓存.
                if !recalc {
                    return Ok((false, first_set.clone()));
                }
            }
            _ => (),
        }
        *first_set = FirstSet::Calculating;
        drop(first_set);
        let mut first_set = HashSet::new();
        let mut should_recalc = false; // 标记自身 first 集是否需要重新计算.
        let mut need_recalc = HashSet::new(); // 需要重新计算 first 集的 productions.
        for prod in self.prods_of(nt) {
            let mut tail = prod.tail().iter();
            let mut should_break = false;
            while !should_break {
                should_break = true;
                match tail.next() {
                    None => {
                        first_set.insert(EPSILON);
                    }
                    Some(Token::Terminal(EPSILON)) => {
                        // pass through
                        should_break = false;
                    }
                    Some(Token::Terminal(t)) => {
                        first_set.insert(*t);
                    }
                    Some(Token::NonTerminal(nt)) => match self.calc_first(*nt, false) {
                        Ok((recalc, s)) => {
                            first_set.extend(s.iter().filter(|t| **t != EPSILON));
                            if s.contains(&EPSILON) {
                                should_break = false;
                            }
                            if recalc {
                                need_recalc.insert(prod);
                            }
                        }
                        Err(Error::InvalidFirstSetState) => {
                            // 遇到了左递归, 暂时不使用这个产生式的内容, 延迟计算 first 集.
                            should_recalc = true;
                        }
                        Err(e) => Err(e)?,
                    },
                }
            }
        }

        // 先提供一个临时的 first set 给子递归使用.
        *self.first_sets.get(&nt).unwrap().borrow_mut() = FirstSet::Presense(first_set.clone());

        for prod in need_recalc {
            let mut tail = prod.tail().iter();
            let mut should_break = false;
            while !should_break {
                should_break = true;
                match tail.next() {
                    None => {
                        first_set.insert(EPSILON);
                    }
                    Some(Token::Terminal(EPSILON)) => {
                        // pass through
                        should_break = false;
                    }
                    Some(Token::Terminal(t)) => {
                        first_set.insert(*t);
                    }
                    Some(Token::NonTerminal(nt)) => match self.calc_first(*nt, true) {
                        Ok((recalc, s)) => {
                            first_set.extend(s.iter().filter(|t| **t != EPSILON));
                            if s.contains(&EPSILON) {
                                should_break = false;
                            }
                            if recalc {
                                // 已经给这个非终结符 (nt) 提供了自身的 first 集, 但是其还是说自身需要重新计算,
                                // 那么说明问题不出在自身, 无法在此处解决, 标记自身需要重新计算, 等待 caller 重新计算.
                                should_recalc = true;
                            }
                        }
                        Err(Error::InvalidFirstSetState) => {
                            // 遇到了左递归, 暂时不使用这个产生式的内容, 延迟计算 first 集.
                            should_recalc = true;
                        }
                        Err(e) => Err(e)?,
                    },
                }
            }
        }
        *self.first_sets.get(&nt).unwrap().borrow_mut() = FirstSet::Presense(first_set.clone());
        Ok((should_recalc, first_set))
    }

    /// 计算一个 token 序列的 first 集
    ///
    /// 如果 `seq` 为空, 那么会返回空的 [`HashSet`], 这和只有 [`EPSILON`] 的 HashSet 并不同,
    /// 前者表示并非任何元素的 first 集, 后者表示某个 token 的 first 只能为空字符串 (tok -> epsilon).
    pub(crate) fn first_set(
        &self,
        mut seq: impl Iterator<Item = Token<'a>>,
    ) -> Result<HashSet<Terminal<'a>>, Error> {
        let mut should_break = false;
        let mut first_set = HashSet::new();
        while !should_break {
            should_break = true;
            match seq.next() {
                None => {
                    first_set.insert(EPSILON);
                }
                Some(Token::Terminal(EPSILON)) => {
                    should_break = false;
                }
                Some(Token::Terminal(t)) => {
                    first_set.insert(t);
                }
                Some(Token::NonTerminal(nt)) => {
                    let (recalc, mut fs) = self.calc_first(nt, false)?;
                    if recalc {
                        let (recalc, fs_) = self.calc_first(nt, true)?;
                        if recalc {
                            Err(Error::UnresolvableFirstSet)?
                        }
                        fs = fs_;
                    }
                    first_set.extend(fs.iter().filter(|t| **t != EPSILON));
                    if fs.contains(&EPSILON) {
                        should_break = false;
                    }
                }
            }
        }
        Ok(first_set)
    }

    /// 使用当前的 CFG 语法解析一个产生式字符串.
    ///
    /// 如果产生式头部符号在语法中为非终结符, 那么返回 [`Error::ParseProductionError`] 中的 [`ParseProductionError::TokenTypeMisMatch`].
    ///
    /// 新的符号会被解析成终结符.
    pub fn parse_production<'b>(&self, line: &'b str) -> Result<Production<'b>, Error> {
        let parts = line.split_once("->").ok_or(Error::parse_production_error(
            0,
            ParseProductionError::NoArrow,
        ))?;
        let head = parts.0.trim();
        if let Some(tok) = self.get_token(head)
            && tok.is_term()
        {
            Err(Error::parse_production_error(
                0,
                ParseProductionError::TokenTypeMisMatch(head.to_string()),
            ))?
        }
        let head = NonTerminal::from(head);
        let tail = parts
            .1
            .split_ascii_whitespace()
            .map(|s| {
                let s = s.trim();
                // 之所以这么绕着写是为了契合生命周期判断.
                if let Some(tok) = self.get_token(s)
                    && tok.is_non_term()
                {
                    NonTerminal::from(s).into()
                } else {
                    Terminal::from(s).into()
                }
            })
            .collect();
        Ok(Production::new(head, tail))
    }

    pub fn get_token<'b>(&self, tok: &'b str) -> Option<Token<'a>> {
        // 这里的返回值并不会引用输入参数 tok, 函数返回之后就结束对 tok 的使用, 因此无视此处生命周期的编译报错.
        let tok = unsafe { std::mem::transmute::<&'b str, &'a str>(tok) };
        self.tokens
            .get(&NonTerminal::from(tok).into())
            .or_else(|| self.tokens.get(&Terminal::from(tok).into()))
            .copied()
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use crate::{
        NonTerminal, Production, Terminal, Token,
        error::{Error, ParseProductionError},
        grammar::Grammar,
        token::{EOF, EPSILON},
    };
    use bumpalo::Bump;
    use pretty_assertions::assert_eq;

    #[test]
    fn parse_productions() {
        let input = "
            program -> compoundstmt
            stmt -> ifstmt | whilestmt | assgstmt
            compoundstmt -> { stmts }
        ";
        let bump = Bump::new();
        let grammar = Grammar::from_cfg(input, "program".into(), &bump)
            .unwrap()
            .augmented();

        let prods = [
            Production::new(
                "programprime".into(),
                vec![NonTerminal::from("program").into()],
            ),
            Production::new(
                "program".into(),
                vec![NonTerminal::from("compoundstmt").into()],
            ),
            Production::new("stmt".into(), vec![Terminal::from("ifstmt").into()]),
            Production::new("stmt".into(), vec![Terminal::from("whilestmt").into()]),
            Production::new("stmt".into(), vec![Terminal::from("assgstmt").into()]),
            Production::new(
                "compoundstmt".into(),
                vec![
                    Terminal::from("{").into(),
                    Terminal::from("stmts").into(),
                    Terminal::from("}").into(),
                ],
            ),
        ];

        let tokens: BTreeSet<Token<'static>> = [
            NonTerminal::from("programprime").into(),
            NonTerminal::from("program").into(),
            NonTerminal::from("compoundstmt").into(),
            NonTerminal::from("stmt").into(),
            EPSILON.into(),
            EOF.into(),
            Terminal::from("ifstmt").into(),
            Terminal::from("whilestmt").into(),
            Terminal::from("assgstmt").into(),
            Terminal::from("{").into(),
            Terminal::from("}").into(),
            Terminal::from("stmts").into(),
        ]
        .into();

        assert_eq!(grammar.start, "programprime".into());
        assert_eq!(grammar.prods, prods.iter().collect::<Vec<_>>());
        assert_eq!(grammar.tokens, tokens);
        assert_eq!(
            grammar.parse_production("S -> a b c"),
            Ok(Production::new(
                "S".into(),
                vec![
                    Terminal::from("a").into(),
                    Terminal::from("b").into(),
                    Terminal::from("c").into()
                ]
            ))
        );
        assert_eq!(
            grammar.parse_production("ifstmt -> a"),
            Err(Error::ParseProductionError {
                line: 0,
                cause: ParseProductionError::TokenTypeMisMatch("ifstmt".into())
            })
        )
    }

    #[test]
    fn first() {
        let bump = Bump::new();
        let grammar = Grammar::from_cfg(
            "program -> stmts
            stmts -> { stmt stmts } | stmt | E | program",
            "program".into(),
            &bump,
        )
        .unwrap()
        .augmented();
        dbg!(&grammar);
        let stmt = Terminal::from("stmt");
        let stmts = NonTerminal::from("stmts");
        let programprime = NonTerminal::from("programprime");
        let brace_l = Terminal::from("{");
        println!("--- 1 ---");
        assert_eq!(
            grammar.first_set([stmts.into()].into_iter()).unwrap(),
            [brace_l, stmt, EPSILON].into()
        );
        println!("--- 2 ---");
        assert_eq!(
            grammar
                .first_set([programprime.into()].into_iter())
                .unwrap(),
            [brace_l, stmt, EPSILON].into()
        );
    }
}
