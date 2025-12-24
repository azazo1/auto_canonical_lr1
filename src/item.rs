use std::{
    collections::{BTreeSet, HashMap},
    fmt::{Debug, Display},
    hash::Hash,
};

use crate::{
    Grammar, Production, Terminal, Token,
    error::Error,
    token::{EOF, EPSILON},
};

// hashset hash 的时候需要注意, 必须要按照特定的顺序进行 hash 计算,
// 不然相等对象由于哈希集合的无序性就会产生不同的 hash 结果.
// 但是如果进行临时的排序的话, 就会极度增大时间复杂度, 本来是 O(1) 的现在变成了 O(n log(n)).
// 于是可以替代地使用 BTreeSet 始终保持有序, 但是牺牲一点时间复杂度.

/// 规范 LR(1) 项
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Item<'a> {
    /// 项集对应的产生式.
    prod: &'a Production<'a>,
    /// dot 所处的位置, 在 `0..=prod.len()` 范围中, 产生式中的 epsilon 不算长度.
    dot: usize,
    /// 前瞻字符
    look_aheads: BTreeSet<Terminal<'a>>,
}

impl Debug for Item<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tail_s: String = self
            .prod
            .tail_without_eps()
            .enumerate()
            .map(|(i, t)| format!("{}{:?} ", if i == self.dot { "⋅ " } else { "" }, t))
            .collect();
        f.pad(&format!(
            "Item({:?} -> {} {:?})",
            self.prod.head(),
            format!(
                "{}{}",
                tail_s.trim_end(),
                if self.dot == self.prod.len() {
                    " ⋅"
                } else {
                    ""
                }
            )
            .trim(),
            &self.look_aheads
        ))
    }
}

impl Display for Item<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let tail_s: String = self
            .prod
            .tail_without_eps()
            .enumerate()
            .map(|(i, t)| format!("{}{} ", if i == self.dot { "⋅ " } else { "" }, t))
            .collect();
        let look_aheads: String = self.look_aheads.iter().map(|x| format!("{x}, ")).collect();
        f.pad(&format!(
            "{} -> {} 〈{}〉",
            self.prod.head(),
            format!(
                "{}{}",
                tail_s.trim_end(),
                if self.dot == self.prod.len() {
                    " ⋅"
                } else {
                    ""
                }
            )
            .trim(),
            look_aheads.trim_end_matches([',', ' '])
        ))
    }
}

impl<'a> Item<'a> {
    #[allow(dead_code)]
    #[must_use]
    pub(crate) fn new(
        prod: &'a Production<'a>,
        dot: usize,
        look_aheads: BTreeSet<Terminal<'a>>,
    ) -> Self {
        Self {
            prod,
            dot,
            look_aheads,
        }
    }

    #[must_use]
    pub(crate) fn initial(prod: &'a Production<'a>, look_aheads: BTreeSet<Terminal<'a>>) -> Self {
        Self {
            prod,
            dot: 0,
            look_aheads,
        }
    }

    #[must_use]
    fn with_dot(&self, dot: usize) -> Self {
        Self {
            prod: self.prod,
            dot,
            look_aheads: self.look_aheads.clone(),
        }
    }

    fn future_seq(&self) -> impl Iterator<Item = &Token<'a>> {
        self.prod.tail_without_eps().skip(self.dot + 1)
    }

    #[must_use]
    fn expected(&self) -> Option<Token<'a>> {
        self.prod.tail_without_eps().nth(self.dot).copied()
    }

    #[must_use]
    pub fn goto(&self, token: Token<'a>) -> Option<Self> {
        let Some(expected) = self.expected() else {
            None?
        };
        if expected != token {
            None?
        }
        Some(self.with_dot(self.dot + 1))
    }

    /// 返回可以执行 reduce 操作的终结符.
    /// 如果不能 reduce, 那么返回 None.
    #[must_use]
    pub fn reduces(&self) -> Option<impl Iterator<Item = Terminal<'a>>> {
        if self.expected().is_some() {
            return None;
        }
        Some(self.look_aheads.iter().copied())
    }

    #[must_use]
    fn core(&self) -> (&'a Production<'a>, usize) {
        (self.prod, self.dot)
    }

    #[must_use]
    pub fn prod(&self) -> &'a Production<'a> {
        self.prod
    }
}

#[derive(Clone)]
pub struct ItemSet<'a> {
    grammar: &'a Grammar<'a>,
    items: BTreeSet<Item<'a>>,
}

impl Debug for ItemSet<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ItemSet")
            .field("items", &self.items)
            .finish()
    }
}

impl PartialEq for ItemSet<'_> {
    fn eq(&self, other: &Self) -> bool {
        self.items == other.items
    }
}

impl Eq for ItemSet<'_> {}

impl PartialOrd for ItemSet<'_> {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for ItemSet<'_> {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.items.cmp(&other.items)
    }
}

impl Hash for ItemSet<'_> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.items.hash(state);
    }
}

impl<'a> ItemSet<'a> {
    /// 获取 I_0 项集.
    ///
    /// `grammar` 需要是已经增广的文法.
    ///
    /// 如果 grammar 的 [`Grammar::symbol_start`] 没有对应的产生式, 那么返回 [`Error::GrammarNotAugmented`]
    pub(crate) fn initial(grammar: &'a Grammar<'a>) -> Result<Self, Error> {
        let start_prod = grammar.prods_of(grammar.symbol_start());
        if start_prod.len() != 1 {
            Err(Error::GrammarNotAugmented)?
        }
        let item = Item::initial(start_prod.into_iter().next().unwrap(), [EOF].into());
        Ok(Self {
            grammar,
            items: [item].into(),
        }
        .closure())
    }

    /// 合并具有相同核心, 但是不同 [`look_aheads`] 的项
    #[must_use]
    fn merge(self) -> Self {
        let mut map: HashMap<(&Production<'_>, usize), Vec<_>> = HashMap::new();
        for item in self.items {
            map.entry(item.core()).or_default().push(item);
        }
        let items = map
            .into_values()
            .filter_map(|v| {
                v.into_iter().reduce(|mut a, b| {
                    a.look_aheads.extend(b.look_aheads);
                    a
                })
            })
            .collect();
        Self {
            grammar: self.grammar,
            items,
        }
    }

    /// 获取当前项集的闭包项集
    #[must_use]
    fn closure(self) -> Self {
        let mut items = self.items.clone();
        loop {
            let mut new_items = BTreeSet::new();
            for item in &items {
                let Some(Token::NonTerminal(nt)) = item.expected() else {
                    continue;
                };
                let mut look_aheads: BTreeSet<_> = self
                    .grammar
                    .first_set(item.future_seq().copied())
                    .unwrap()
                    .into_iter()
                    .collect();
                if look_aheads.contains(&EPSILON) {
                    look_aheads.remove(&EPSILON);
                    look_aheads.extend(&item.look_aheads);
                }
                let prods = self.grammar.prods_of(nt);
                new_items.insert(item.clone());
                for prod in prods {
                    new_items.insert(Item::initial(prod, look_aheads.clone()));
                }
            }
            if new_items.difference(&items).next().is_none() {
                break;
            }
            items.extend(new_items);
        }
        Self {
            items,
            grammar: self.grammar,
        }
        .merge()
    }

    #[must_use]
    pub(crate) fn goto(&self, token: Token<'a>) -> Option<Self> {
        let items: BTreeSet<Item<'a>> = self.items.iter().filter_map(|i| i.goto(token)).collect();
        if items.is_empty() {
            None
        } else {
            Some(
                Self {
                    grammar: self.grammar,
                    items,
                }
                .closure(),
            )
        }
    }

    pub fn items(&self) -> impl Iterator<Item = &Item<'a>> {
        self.items.iter()
    }

    pub fn reduces(&self) -> impl Iterator<Item = (&Item<'a>, Terminal<'a>)> {
        self.items
            .iter()
            .filter_map(|i| i.reduces().map(|r| (i, r)))
            .flat_map(|(i, r)| r.map(move |t| (i, t)))
    }
}

#[derive(Debug)]
pub struct Family<'a> {
    item_sets: Vec<&'a ItemSet<'a>>,
    #[allow(dead_code)]
    item_sets_idx: HashMap<&'a ItemSet<'a>, usize>,
    /// 描述了 goto 动作.
    /// GOTO(key, value.0) = value.1
    gotos: HashMap<usize, BTreeSet<(Token<'a>, usize)>>,
}

impl<'a> Family<'a> {
    /// 从 `grammar` 构建规范 LR(1) 项集族.
    #[must_use]
    pub fn from_grammar(grammar: &'a Grammar<'a>) -> Self {
        let bump = grammar.bump();
        let i0 = &*bump.alloc(ItemSet::initial(grammar).unwrap());
        #[allow(clippy::mutable_key_type)]
        let mut item_sets_idx = HashMap::new();
        let mut item_sets = Vec::new();
        let mut gotos: HashMap<usize, BTreeSet<(Token<'a>, usize)>> = HashMap::new();
        item_sets_idx.insert(i0, 0);
        item_sets.push(i0);
        loop {
            let mut new_item_sets = Vec::new();
            for (from, is) in item_sets.iter().enumerate() {
                for &tok in grammar.tokens() {
                    let Some(nis) = is.goto(tok) else {
                        continue;
                    };
                    let nis = &*bump.alloc(nis);
                    if let Some(&to) = item_sets_idx.get(&nis) {
                        gotos.entry(from).or_default().insert((tok, to));
                    } else {
                        // 新加入的项集: nis
                        // GOTO(is, tok) = nis
                        let to = item_sets.len() + new_item_sets.len();
                        gotos.entry(from).or_default().insert((tok, to));
                        // println!("{:?}, {}, {}", tok, from, to);
                        new_item_sets.push(nis);
                        item_sets_idx.insert(nis, to);
                    }
                }
            }
            // 没有新项集会被加入之后, 收敛, 结束.
            if new_item_sets.is_empty() {
                break;
            }
            item_sets.extend(new_item_sets);
        }
        Self {
            item_sets_idx,
            item_sets,
            gotos,
        }
    }

    /// 按照 I_i (i = 0, 1, 2, 3...) 顺序获取项集.
    #[must_use]
    pub fn item_sets(&self) -> &[&'a ItemSet<'a>] {
        &self.item_sets
    }

    /// 遍历 gotos (起始项集, 转换 Token, 到达项集).
    pub fn gotos(&self) -> impl Iterator<Item = (usize, Token<'a>, usize)> {
        self.gotos
            .iter()
            .flat_map(|(&from, v)| v.iter().map(move |&(tok, to)| (from, tok, to)))
    }

    /// 获取一个项集的 gotos: (转换 Token, 到达项集).
    /// 如果 item_set, 没有对应项集, 或者项集没有出边, 那么返回 [`None`]
    #[must_use]
    pub fn gotos_of(&self, item_set: usize) -> Option<impl Iterator<Item = (Token<'a>, usize)>> {
        self.gotos.get(&item_set).map(|v| v.iter().copied())
    }

    /// 获取项集族数量
    #[must_use]
    pub fn len(&self) -> usize {
        self.item_sets.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

#[cfg(test)]
mod test {
    use std::collections::BTreeSet;

    use bumpalo::Bump;

    use crate::{
        Family, Grammar, NonTerminal, Production, Terminal, Token,
        item::{Item, ItemSet},
        token::{EOF, EPSILON},
    };
    use pretty_assertions::assert_eq;

    #[test]
    fn closure() {
        let bump = Bump::new();
        let grammar = Grammar::from_cfg("program -> stmts\nstmts -> good", "program".into(), &bump)
            .unwrap()
            .augmented();
        let i0 = ItemSet::initial(&grammar).unwrap();
        let eof_la: fn() -> BTreeSet<Terminal<'static>> = || [EOF].into();
        let prod_programprime_program = Production::new(
            "programprime".into(),
            [NonTerminal::from("program").into()].into(),
        );
        let prod_program_stmts =
            Production::new("program".into(), [NonTerminal::from("stmts").into()].into());
        let prod_stmts_good =
            Production::new("stmts".into(), [Terminal::from("good").into()].into());
        assert_eq!(
            i0,
            ItemSet {
                grammar: &grammar,
                items: [
                    Item::initial(&prod_programprime_program, eof_la()),
                    Item::initial(&prod_program_stmts, eof_la()),
                    Item::initial(&prod_stmts_good, eof_la())
                ]
                .into()
            }
        );
    }

    #[test]
    fn goto_basic_transition() {
        let bump = Bump::new();
        // 原文法:
        // S -> E
        // E -> a
        //
        // 增广后:
        // Sprime -> S
        let grammar = Grammar::from_cfg("S -> E\nE -> a", "S".into(), &bump)
            .unwrap()
            .augmented();

        let i0 = ItemSet::initial(&grammar).unwrap();

        let eof_la: fn() -> BTreeSet<_> = || [EOF].into();

        // 手动构造产生式对象以便验证
        let prod_sprime_s =
            Production::new("Sprime".into(), [NonTerminal::from("S").into()].into());
        let prod_s_e = Production::new("S".into(), [NonTerminal::from("E").into()].into());
        let prod_e_a = Production::new("E".into(), [Terminal::from("a").into()].into());

        // 测试 1: 针对原始起始符 S 的 Goto (测试增广产生式的移动)
        // I0 包含:
        // - Sprime -> . S {EOF}
        // - S -> . E {EOF}
        // - E -> . a {EOF}
        // 期望结果: Sprime -> S . {EOF}
        let token_s = Token::from(NonTerminal::from("S"));
        let next_state_s = i0.goto(token_s).expect("Should goto S");

        assert_eq!(
            next_state_s,
            ItemSet {
                grammar: &grammar,
                items: [Item::new(&prod_sprime_s, 1, eof_la())].into() // Sprime -> S .
            }
        );

        // 测试 2: 针对非终结符 E 的 Goto
        // I0 包含: S -> . E {EOF} (来自于 Sprime -> . S 的闭包)
        // 期望结果: S -> E . {EOF}
        let token_e = NonTerminal::from("E");
        let next_state_e = i0.goto(token_e.into()).expect("Should goto E");

        assert_eq!(
            next_state_e,
            ItemSet {
                grammar: &grammar,
                items: [Item::new(&prod_s_e, 1, eof_la())].into() // S -> E .
            }
        );

        // 测试 3: 针对终结符 a 的 Goto
        // I0 包含: E -> . a {EOF}
        // 期望结果: E -> a . {EOF}
        let token_a = Terminal::from("a");
        let next_state_a = i0.goto(token_a.into()).expect("Should goto a");

        assert_eq!(
            next_state_a,
            ItemSet {
                grammar: &grammar,
                items: [Item::new(&prod_e_a, 1, eof_la())].into() // E -> a .
            }
        );
    }

    #[test]
    fn goto_triggers_closure_and_recursion() {
        let bump = Bump::new();
        // 原文法:
        // program -> stmts
        // stmts -> stmt stmts | stmt
        //
        // 增广后:
        // programprime -> program
        let grammar = Grammar::from_cfg(
            "program -> stmts\nstmts -> stmt stmts | stmt",
            "program".into(),
            &bump,
        )
        .unwrap()
        .augmented();

        let i0 = ItemSet::initial(&grammar).unwrap();
        let stmt = Terminal::from("stmt");

        // I0 推导逻辑:
        // 1. programprime -> . program {EOF}
        // 2. program -> . stmts {EOF}
        // 3. stmts -> . stmt stmts {EOF}
        // 4. stmts -> . stmt {EOF}

        // 执行 GOTO(I0, stmt)
        // 移动的是项 3 和 4
        let i1 = i0.goto(stmt.into()).expect("Should goto stmt");

        let stmts = NonTerminal::from("stmts");
        let eof_la: fn() -> BTreeSet<_> = || [EOF].into();

        let prod_stmts_recursive = Production::new(stmts, [stmt.into(), stmts.into()].into());
        let prod_stmts_single = Production::new(stmts, [stmt.into()].into());

        assert_eq!(
            i1,
            ItemSet {
                grammar: &grammar,
                items: [
                    // 核心项 (移动后的项):
                    Item::new(&prod_stmts_recursive, 1, eof_la()), // stmts -> stmt . stmts {EOF}
                    Item::new(&prod_stmts_single, 1, eof_la()),    // stmts -> stmt . {EOF}
                    // 闭包项 (由 stmts -> stmt . stmts 触发):
                    // 注意：由于 programprime -> program 的 Lookahead 是 EOF，
                    // 这里传递下来的 Lookahead 依然是 EOF
                    Item::new(&prod_stmts_recursive, 0, eof_la()), // stmts -> . stmt stmts {EOF}
                    Item::new(&prod_stmts_single, 0, eof_la()),    // stmts -> . stmt {EOF}
                ]
                .into()
            }
        );
    }

    #[test]
    fn goto_preserves_lookahead() {
        let bump = Bump::new();
        // 原文法:
        // S -> A b
        // A -> a
        //
        // 增广后:
        // Sprime -> S
        let grammar = Grammar::from_cfg("S -> A b\nA -> a", "S".into(), &bump)
            .unwrap()
            .augmented();
        let i0 = ItemSet::initial(&grammar).unwrap();

        let a_term = Terminal::from("a");
        let b_term = Terminal::from("b");
        let a_nt = NonTerminal::from("A");

        // 手动构造产生式
        let prod_a_a = Production::new(a_nt, [a_term.into()].into());
        // Lookahead 为 {b} 的原因:
        // 1. Sprime -> . S {EOF}
        // 2. S -> . A b {EOF}  (这里 A 后面紧跟 b)
        // 3. A -> . a {b}     (所以这里 lookahead 是 b)
        let lookahead_b: BTreeSet<_> = [b_term].into();

        // 1. 验证 I0 确实包含了正确的 Lookahead (这一步验证闭包算法对 Lookahead 的计算)
        let expected_initial_item = Item::new(&prod_a_a, 0, lookahead_b.clone());
        assert!(
            i0.items.contains(&expected_initial_item),
            "I0 should contain A -> . a {{b}}"
        );

        // 2. 测试 GOTO(I0, a)
        // 从 A -> . a {b} 移动
        let i_next = i0.goto(a_term.into()).unwrap();

        // 期望: A -> a . {b}
        // 即使有 Sprime -> S，也不应该影响这里底层的 Lookahead 传递
        assert_eq!(
            i_next,
            ItemSet {
                grammar: &grammar,
                items: [Item::new(&prod_a_a, 1, lookahead_b)].into()
            }
        );
    }
    #[test]
    fn family_of_itemsets() {
        (0..10).for_each(|_| family_of_itemsets_repeaten());
    }

    fn family_of_itemsets_repeaten() {
        let bump = Bump::new();
        let grammar = Grammar::from_cfg(
            "program -> stmts
            stmts -> stmt stmts | stmt",
            "program".into(),
            &bump,
        )
        .unwrap()
        .augmented();
        let family = Family::from_grammar(&grammar);
        let program = NonTerminal::from("program");
        let programprime = NonTerminal::from("programprime");
        let stmts = NonTerminal::from("stmts");
        let stmt = Terminal::from("stmt");
        let eof_la: fn() -> BTreeSet<_> = || [EOF].into();
        // 这里使用 Vec, 就是要确保项集状态顺序的不变性, 不能每次运行都是随机的编号.
        assert_eq!(
            family.item_sets,
            [
                ItemSet {
                    grammar: &grammar,
                    items: [
                        Item::initial(
                            &Production::new(programprime, [program.into()].into()),
                            eof_la()
                        ),
                        Item::initial(&Production::new(program, [stmts.into()].into()), eof_la()),
                        Item::initial(
                            &Production::new(stmts, [stmt.into(), stmts.into()].into()),
                            eof_la()
                        ),
                        Item::initial(&Production::new(stmts, [stmt.into()].into()), eof_la())
                    ]
                    .into()
                },
                ItemSet {
                    grammar: &grammar,
                    items: [
                        Item::new(
                            &Production::new(stmts, [stmt.into(), stmts.into()].into()),
                            1,
                            eof_la()
                        ),
                        Item::new(&Production::new(stmts, [stmt.into()].into()), 1, eof_la()),
                        Item::new(&Production::new(stmts, [stmt.into()].into()), 0, eof_la()),
                        Item::new(
                            &Production::new(stmts, [stmt.into(), stmts.into()].into()),
                            0,
                            eof_la()
                        ),
                    ]
                    .into()
                },
                ItemSet {
                    grammar: &grammar,
                    items: [Item::new(
                        &Production::new(programprime, [program.into()].into()),
                        1,
                        eof_la()
                    )]
                    .into()
                },
                ItemSet {
                    grammar: &grammar,
                    items: [Item::new(
                        &Production::new(program, [stmts.into()].into()),
                        1,
                        eof_la()
                    )]
                    .into()
                },
                ItemSet {
                    grammar: &grammar,
                    items: [Item::new(
                        &Production::new(stmts, [stmt.into(), stmts.into()].into()),
                        2,
                        eof_la()
                    )]
                    .into()
                }
            ]
            .iter()
            .collect::<Vec<_>>()
        );
    }

    #[test]
    fn epsilon_prod() {
        let prod = Production::new("head".into(), [EPSILON.into()].into());
        let item = Item::initial(&prod, [EOF].into());
        assert_eq!(item.expected(), None);
        assert_eq!(item.goto(EPSILON.into()), None);
        assert_eq!(format!("{}", item), r#"head -> ⋅ 〈eof〉"#);
    }

    #[test]
    fn family_of_complex_cfg() {
        let bump = Bump::new();
        let grammar = Grammar::from_cfg(
            r#"program -> compoundstmt
stmt -> ifstmt | whilestmt | assgstmt | compoundstmt
compoundstmt -> { stmts }
stmts -> stmt stmts | E
ifstmt -> if ( boolexpr ) then stmt else stmt
whilestmt -> while ( boolexpr ) stmt
assgstmt -> ID = arithexpr ;
boolexpr -> arithexpr boolop arithexpr
boolop -> < | > | <= | >= | ==
arithexpr -> multexpr arithexprprime
arithexprprime -> + multexpr arithexprprime | - multexpr arithexprprime | E
multexpr -> simpleexpr multexprprime
multexprprime -> * simpleexpr multexprprime | / simpleexpr multexprprime | E
simpleexpr -> ID | NUM | ( arithexpr )"#,
            "program".into(),
            &bump,
        )
        .unwrap();
        let family = Family::from_grammar(&grammar);
        assert_eq!(
            family.gotos_of(42).map(|it| it.collect::<Vec<_>>()),
            Some(
                [
                    (Terminal::from("(").into(), 20,),
                    (Terminal::from("ID").into(), 21,),
                    (Terminal::from("NUM").into(), 22,),
                    (NonTerminal::from("multexpr").into(), 71,),
                    (NonTerminal::from("simpleexpr").into(), 25,),
                ]
                .into()
            )
        );
    }
}
