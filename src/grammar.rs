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

#[derive(Debug, Clone)]
pub struct Grammar<'a> {
    bump: &'a Bump,
    prods: Vec<&'a Production<'a>>,
    prod_indexes: HashMap<&'a Production<'a>, usize>,
    tokens: BTreeSet<Token<'a>>,
    start: NonTerminal<'a>,
    // 各个非终结符的 first 集, 在文法创建的时候计算.
    first_sets: RefCell<HashMap<NonTerminal<'a>, HashSet<Terminal<'a>>>>,
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
        let raw_first_set = self.first_sets.borrow().get(&self.start).unwrap().clone();
        self.first_sets
            .borrow_mut()
            .insert(augmented_start, raw_first_set);
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
        let grammar = Grammar {
            prod_indexes,
            prods,
            start,
            bump,
            tokens,
            first_sets: RefCell::new(HashMap::new()),
        };
        grammar.compute_all_first_sets();
        Ok(grammar)
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

    /// 不动点迭代计算所有 FIRST 集
    fn compute_all_first_sets(&self) {
        let mut sets = self.first_sets.borrow_mut();
        sets.clear();
        let mut changed = true;

        while changed {
            changed = false;

            for prod in &self.prods {
                let head = prod.head();
                let mut item_changed = false;

                // head 的 first 集是否有 eps.
                let mut derive_epsilon = true;
                // 模拟扫描产生式右部
                for token in prod.tail() {
                    match token {
                        Token::Terminal(t) => {
                            if *t == EPSILON {
                                // 继续看下一个符号
                                continue;
                            }
                            // 遇到终结符, 加入到 head 的 FIRST 集
                            if sets.entry(head).or_default().insert(*t) {
                                item_changed = true;
                            }
                            derive_epsilon = false;
                            break;
                        }
                        Token::NonTerminal(nt) => {
                            // 防止借用冲突, 直接克隆.
                            let nt_first = sets.entry(*nt).or_default().clone();
                            let head_first = sets.entry(head).or_default();

                            // 如果 head == *nt, 那么直接跳过, 因为不会添加任何东西.
                            if *nt != head {
                                for &f_token in nt_first.iter() {
                                    if f_token != EPSILON && head_first.insert(f_token) {
                                        item_changed = true;
                                    }
                                }
                            }

                            if !nt_first.contains(&EPSILON) {
                                derive_epsilon = false;
                                break;
                            }
                        }
                    }
                }

                if derive_epsilon && sets.entry(head).or_default().insert(EPSILON) {
                    item_changed = true;
                }

                if item_changed {
                    changed = true;
                }
            }
        }
    }

    /// 获取一个 symbol 序列的 first 集, 如果序列为空, 那么返回一个包含 [`EPSILON`] 的 HashSet.
    /// # Errors
    /// - [`Error::FirstSetNotCalc`]: 文法的 first 集没有预先计算.
    pub fn first_set(
        &self,
        seq: impl Iterator<Item = Token<'a>>,
    ) -> Result<HashSet<Terminal<'a>>, Error> {
        let mut result = HashSet::new();
        let mut derive_epsilon = true;

        for token in seq {
            match token {
                Token::Terminal(t) => {
                    if t == EPSILON {
                        continue;
                    }
                    result.insert(t);
                    derive_epsilon = false;
                    break;
                }
                Token::NonTerminal(nt) => {
                    let fsets = self.first_sets.borrow();
                    let nt_set = fsets.get(&nt).ok_or(Error::FirstSetNotCalc)?;
                    for &f in nt_set {
                        if f != EPSILON {
                            result.insert(f);
                        }
                    }
                    if !nt_set.contains(&EPSILON) {
                        derive_epsilon = false;
                        break;
                    }
                }
            }
        }
        if derive_epsilon {
            result.insert(EPSILON);
        }
        Ok(result)
    }

    /// 计算 seq 的 first 集, 如果 seq 的 first 集中有 [`EPSILON`] 或者 first 集为空,
    /// 那么附加 fallthrough 提供的终结符.
    pub fn first_set_with_fallthrough(
        &self,
        seq: impl Iterator<Item = Token<'a>>,
        fallthrough: impl Iterator<Item = Terminal<'a>>,
    ) -> Result<HashSet<Terminal<'a>>, Error> {
        let mut set = self.first_set(seq)?;
        if set.is_empty() || set.contains(&EPSILON) {
            set.remove(&EPSILON);
            set.extend(fallthrough)
        }
        Ok(set)
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

    // fixme: 直接左递归无法获取正确的结果.
    #[test]
    fn first_with_left_recursive() {
        let bump = Bump::new();
        let grammar =
            Grammar::from_cfg("program -> program good | nice", "program".into(), &bump).unwrap();
        assert_eq!(
            grammar
                .first_set([NonTerminal::from("program").into()].into_iter())
                .unwrap(),
            [Terminal::from("nice")].into()
        );
    }

    #[test]
    fn first_with_left_recursive_epsilon() {
        let bump = Bump::new();
        let grammar = Grammar::from_cfg("A -> A a | E", "A".into(), &bump).unwrap();

        let result = grammar
            .first_set([NonTerminal::from("A").into()].into_iter())
            .unwrap();
        assert_eq!(result, [Terminal::from("a"), EPSILON].into());
    }

    #[test]
    fn first_with_complex_indirect_left_recursion() {
        let bump = Bump::new();
        // 构造一个复杂的间接左递归文法:
        // 依赖链: S -> A -> B -> C -> S
        //
        // 关键点解析:
        // 1. C -> E, 导致 C 可空.
        // 2. B -> C, 导致 B 可空.
        // 3. A -> B, 导致 A 可空.
        // 4. S -> A x. 因为 A 可空, 所以 'x' 必须加入 FIRST(S).
        // 5. C -> S. 因为 S 包含 'x', 所以 'x' 必须传播回 FIRST(C).
        // 6. 进而 'x' 传播回 FIRST(B) 和 FIRST(A).
        let grammar = Grammar::from_cfg(
            "
                S -> A x
                A -> B | y
                B -> C | z
                C -> S | E
                ",
            "S".into(),
            &bump,
        )
        .unwrap();

        let s = NonTerminal::from("S");
        let a = NonTerminal::from("A");
        let b = NonTerminal::from("B");
        let c = NonTerminal::from("C");

        let term_x = Terminal::from("x");
        let term_y = Terminal::from("y");
        let term_z = Terminal::from("z");

        // 验证 S (S 不包含 EPSILON，因为 S -> A x, x 是终结符且不可空)
        // FIRST(S) = {y, z, x}
        assert_eq!(
            grammar.first_set([s.into()].into_iter()).unwrap(),
            [term_x, term_y, term_z].into()
        );

        // 验证 A (继承自 B, 且包含 y, 且可空)
        // FIRST(A) = {y, z, x, E}
        assert_eq!(
            grammar.first_set([a.into()].into_iter()).unwrap(),
            [term_x, term_y, term_z, EPSILON].into()
        );

        // 验证 B (继承自 C, 且包含 z, 且可空)
        // FIRST(B) = {y, z, x, E}
        assert_eq!(
            grammar.first_set([b.into()].into_iter()).unwrap(),
            [term_x, term_y, term_z, EPSILON].into()
        );

        // 验证 C (继承自 S, 且包含 E)
        // FIRST(C) = {y, z, x, E}
        assert_eq!(
            grammar.first_set([c.into()].into_iter()).unwrap(),
            [term_x, term_y, term_z, EPSILON].into()
        );
    }
}
