use std::{collections::HashMap, fmt::Display, mem::swap};

use crate::{Family, Grammar, NonTerminal, Terminal, Token};

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum ActionCell {
    /// 移入项集状态编号.
    Shift(usize),
    /// 规约产生式编号.
    Reduce(usize),
    /// 包含冲突的两个或者多个表项(树状嵌套).
    Conflict(Box<ActionCell>, Box<ActionCell>),
    /// 接受
    Accept,
    #[default]
    Empty,
}

impl Display for ActionCell {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.pad(&match self {
            Self::Shift(s) => format!("s{s}"),
            Self::Reduce(r) => format!("r{r}"),
            Self::Conflict(_, _) => "[conflict]".to_string(),
            Self::Accept => "acc".to_string(),
            Self::Empty => "".to_string(),
        })
    }
}

impl ActionCell {
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }

    pub fn is_conflict(&self) -> bool {
        matches!(self, Self::Conflict(_, _))
    }

    /// 放入新的 cell 内容, 返回是否冲突
    fn update(&mut self, cell: ActionCell) -> bool {
        let mut conflict = false;
        let mut this = ActionCell::Empty;
        swap(&mut this, self);
        match (this, cell) {
            (Self::Empty, other) => *self = other,
            (this, Self::Empty) => *self = this,
            (Self::Conflict(ca, cb), other) => {
                *self = ActionCell::Conflict(Box::new(Self::Conflict(ca, cb)), Box::new(other));
                conflict = true;
            }
            (this, Self::Conflict(ca, cb)) => {
                *self = Self::Conflict(Box::new(this), Box::new(Self::Conflict(ca, cb)));
                conflict = true;
            }
            (a, b) => *self = Self::Conflict(Box::new(a), Box::new(b)),
        }
        conflict
    }

    /// 展开所有的叶子节点(非 [`ActionCell::Conflict`] 节点)(从树的左侧到右侧).
    #[must_use]
    pub fn flatten(&self) -> Box<dyn Iterator<Item = &ActionCell> + '_> {
        match self {
            Self::Conflict(left, right) => Box::new(left.flatten().chain(right.flatten())),
            _ => Box::new(std::iter::once(self)),
        }
    }
}

#[allow(dead_code)]
#[derive(Debug)]
pub struct Table<'a> {
    /// ACTION 表
    action: Vec<Vec<ActionCell>>,
    /// GOTO 表, 每个格子表示 GOTO 到的项集状态编号.
    goto: Vec<Vec<Option<usize>>>,
    /// [`Family::item_sets`] 中的顺序就是 GOTO 和 ACTION 表的状态顺序.
    family: &'a Family<'a>,
    grammar: &'a Grammar<'a>,
    /// ACTION 表中的终结符, 下标即为 ACTION 表中的列.
    terms: Vec<Terminal<'a>>,
    /// GOTO 表中的非终结符, 下标即为 GOTO 表中的列.
    non_terms: Vec<NonTerminal<'a>>,
    term_idxes: HashMap<Terminal<'a>, usize>,
    non_term_idxes: HashMap<NonTerminal<'a>, usize>,
    /// 文法在规范 LR(1) 分析中是否是冲突的.
    conflict: bool,
}

impl<'a> Table<'a> {
    #[must_use]
    pub fn build_from(family: &'a Family<'a>, grammar: &'a Grammar<'a>) -> Self {
        let tokens = grammar.tokens().iter();
        // 这里要求终结符一定要在非终结符排序的前面.
        let terms: Vec<_> = tokens.clone().map_while(|t| t.as_term()).copied().collect();
        let non_terms: Vec<_> = tokens
            .clone()
            .skip_while(|t| t.is_term())
            .map(|t| t.as_non_term().unwrap())
            .copied()
            .collect();
        let term_idxes: HashMap<Terminal<'a>, usize> =
            terms.iter().enumerate().map(|(a, b)| (*b, a)).collect();
        let non_term_idxes: HashMap<NonTerminal<'a>, usize> =
            non_terms.iter().enumerate().map(|(a, b)| (*b, a)).collect();
        let action_cols = terms.len();
        let goto_cols = non_terms.len();
        let rows = family.len();
        let mut action = vec![vec![ActionCell::Empty; action_cols]; rows];
        let mut goto = vec![vec![None; goto_cols]; rows];
        let mut conflict = false;
        for (row, is) in family.item_sets().iter().enumerate() {
            for (tok, &to) in family
                .gotos_of(row)
                .into_iter()
                .flatten()
                .flat_map(|(tok, dests)| dests.iter().map(move |to| (tok, to)))
            {
                match tok {
                    Token::Terminal(t) => {
                        let term_idx = *term_idxes.get(t).unwrap();
                        conflict |= action[row][term_idx].update(ActionCell::Shift(to));
                    }
                    Token::NonTerminal(nt) => {
                        let non_term_idx = *non_term_idxes.get(nt).unwrap();
                        goto[row][non_term_idx] = Some(to);
                    }
                }
            }
            for (item, t) in is.reduces() {
                let prod_idx = grammar.index_of_prod(item.prod()).unwrap();
                let term_idx = *term_idxes.get(&t).unwrap();
                if prod_idx == 0 && term_idx == terms.len() - 1 {
                    // 根据排序 EOF 是最后一个终结符.
                    // startprime -> start dot, EOF 也就是 acc 状态.
                    conflict |= action[row][term_idx].update(ActionCell::Accept);
                } else {
                    conflict |= action[row][term_idx].update(ActionCell::Reduce(prod_idx));
                }
            }
        }
        Self {
            action,
            goto,
            non_term_idxes,
            family,
            grammar,
            terms,
            non_terms,
            term_idxes,
            conflict,
        }
    }

    #[must_use]
    pub fn rows(&self) -> usize {
        self.family.len()
    }

    #[must_use]
    pub fn action_cols(&self) -> usize {
        self.terms.len()
    }

    #[must_use]
    pub fn goto_cols(&self) -> usize {
        self.non_terms.len()
    }

    #[must_use]
    pub fn conflict(&self) -> bool {
        self.conflict
    }

    /// 使用 markdown 形式输出表格.
    #[must_use]
    pub fn to_markdown(&self) -> String {
        let mut header_line = "| |".to_string();
        header_line += &self
            .terms
            .iter()
            .map(|t| format!(" `{}` |", t.as_str()))
            .chain(
                self.non_terms
                    .iter()
                    .map(|nt| format!(" `{}` |", nt.as_str())),
            )
            .collect::<String>();
        let sep_line: String = String::from("| - |")
            + &std::iter::repeat_n(" - |", self.terms.len() + self.non_terms.len())
                .collect::<String>();
        let mut data_lines = String::new();
        for (i, (action_row, goto_row)) in self.action.iter().zip(self.goto.iter()).enumerate() {
            let line = format!("| $I_{{{i}}}$ |")
                + &action_row
                    .iter()
                    .map(|act| format!(" {act} |"))
                    .chain(goto_row.iter().map(|to| {
                        if let Some(to) = to {
                            format!(" {to} |")
                        } else {
                            "  |".to_string()
                        }
                    }))
                    .collect::<String>();
            data_lines += &line;
            data_lines += "\n";
        }
        format!("{header_line}\n{sep_line}\n{}", data_lines.trim_end())
    }

    /// 查询 ACTION 表, 获取当前项集状态在某个终结符下的动作.
    /// # Returns
    /// 如果项集族中没有这个状态或者文法中没有这个终结符, 那么返回 [`None`].
    #[must_use]
    pub fn action(&self, state: usize, term: Terminal) -> Option<&ActionCell> {
        let term_idx = *self.term_idxes.get(&term)?;
        let row = self.action.get(state)?;
        Some(&row[term_idx])
    }

    /// 遍历一个项集状态的所有非 [`ActionCell::Empty`] actions.
    /// 如果这个项集状态不存在, 那么返回 [`None`].
    #[must_use]
    pub fn actions(
        &self,
        state: usize,
    ) -> Option<impl Iterator<Item = (Terminal<'a>, &ActionCell)>> {
        let v = self.action.get(state)?;
        Some(v.iter().enumerate().filter_map(|(i, a)| {
            if a.is_empty() {
                None
            } else {
                Some((self.terms[i], a))
            }
        }))
    }

    /// 查询 GOTO(state, non_term), 如果 state 或者 non_term 在 GOTO 表中不存在, 那么返回 [`None`].
    /// 如果 state 没有 non_term 这个出边, 那么返回 `Some(None)`.
    #[must_use]
    pub fn goto(&self, state: usize, non_term: NonTerminal) -> Option<Option<usize>> {
        let non_term_idx = *self.non_term_idxes.get(&non_term)?;
        let row = self.goto.get(state)?;
        Some(row[non_term_idx])
    }

    #[inline]
    #[must_use]
    pub(crate) fn family(&self) -> &Family<'a> {
        self.family
    }

    #[inline]
    #[must_use]
    pub(crate) fn grammar(&self) -> &Grammar<'a> {
        self.grammar
    }
}

#[cfg(test)]
mod test {
    use bumpalo::Bump;

    use crate::{Family, Grammar, table::Table};
    use pretty_assertions::assert_eq;

    #[test]
    fn markdown_table() {
        let input = "
            program -> compoundstmt
            stmt -> ifstmt | whilestmt | assgstmt
            compoundstmt -> { stmts }
        ";
        let bump = Bump::new();
        let grammar = Grammar::from_cfg(input, "program".into(), &bump)
            .unwrap()
            .augmented();
        let family = Family::from_grammar(&grammar);
        family.item_sets().iter().enumerate().for_each(|(idx, is)| {
            println!("I_{idx}:");
            is.items().for_each(|i| println!("{}", i));
            println!("reduces:");
            is.reduces()
                .for_each(|(i, t)| println!("{t} r {}", grammar.index_of_prod(i.prod()).unwrap()));
            println!("gotos:");
            family
                .gotos_of(idx)
                .into_iter()
                .flatten()
                .for_each(|(tok, dests)| {
                    dests
                        .iter()
                        .for_each(|to| println!("{idx} -- {tok} --> {to}"))
                });
            println!();
        });
        let table = Table::build_from(&family, &grammar);
        assert!(!table.conflict);
        assert_eq!(
            table.to_markdown(),
            r#"
| | `{` | `}` | `stmts` | `ifstmt` | `assgstmt` | `whilestmt` | `E` | `eof` | `compoundstmt` | `program` | `programprime` | `stmt` |
| - | - | - | - | - | - | - | - | - | - | - | - | - |
| $I_{0}$ | s1 |  |  |  |  |  |  |  | 2 | 3 |  |  |
| $I_{1}$ |  |  | s4 |  |  |  |  |  |  |  |  |  |
| $I_{2}$ |  |  |  |  |  |  |  | r1 |  |  |  |  |
| $I_{3}$ |  |  |  |  |  |  |  | acc |  |  |  |  |
| $I_{4}$ |  | s5 |  |  |  |  |  |  |  |  |  |  |
| $I_{5}$ |  |  |  |  |  |  |  | r5 |  |  |  |  |
"#
            .trim()
        );
    }
}
