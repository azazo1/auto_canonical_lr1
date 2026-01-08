//! 恐慌恢复

#[allow(unused_imports)]
use crate::Grammar;

use crate::{Table, Terminal, Token, error::Error};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanicAction<'a> {
    /// (被跳过的期望终结符, 压入的新状态)
    Shift(Terminal<'a>, usize),
    /// 归约的产生式
    Reduce(usize),
    Accept,
    Empty,
}

impl PanicAction<'_> {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}

impl<'a> Table<'a> {
    /// 恐慌模式获取下一个动作.
    ///
    /// 具体操作:
    /// - 项集中不能归约(reduce)的项, 忽略一个期望的终结符, 尝试 reduce, goto.
    /// - 项集中可以归约的项, 忽略 look_aheads 符, 直接 reduce.
    /// # Errors
    /// - [`Error::StateNotFound`] 项集状态不存在.
    /// - [`Error::AmbiguousGrammar`] 文法是二义性的.
    /// - 其他见: [`Grammar::first_set`].
    /// # Note
    /// 这个实现并不是时间复杂度 O(1) 的, 但是实际上一个文法的 `panic_action` 函数的输出只依赖与 state 和 term 输入,
    /// 因此可以提前建表以实现 O(1) 时间复杂度查询.
    pub fn panic_action(&self, state: usize, term: Terminal) -> Result<PanicAction<'a>, Error> {
        let is = self
            .family()
            .item_sets()
            .get(state)
            .ok_or(Error::StateNotFound(state))?;
        for i in is.items() {
            // 跳过下一个期望终结符, 尝试 reduce / goto.
            // 不考虑期望非终结符的 Item, 因为项集里面肯定有对应的闭包 Item.
            match i.expected() {
                Some(Token::Terminal(raw_expected)) => {
                    let panic_i = i.with_dot_inc();
                    // 到达新的项集状态.
                    let to = self
                        .family()
                        .gotos_of(state)
                        // unwrap: 这个状态一定在集族中存在, 并且有出边, 因为 i 有 expected != None.
                        .unwrap()
                        .get(&raw_expected.into())
                        // unwrap: 这个状态一定有 raw_expected 为 token 的 goto 出边, 因为 i.expected() == raw_expected.
                        .unwrap();
                    if to.len() != 1 {
                        // 文法是二义性的, 无法使用 LR(1) 表达.
                        Err(Error::AmbiguousGrammar)?
                    }
                    let to = *to.first().unwrap();
                    // 尝试 reduce
                    if panic_i.reduces().into_iter().flatten().any(|t| t == term) {
                        // 先移入这个终结符, 然后才能到达归约/接收状态, 后者为恢复之后的 actions.
                        return Ok(PanicAction::Shift(raw_expected, to));
                    // 尝试 goto
                    } else if self
                        .grammar()
                        .first_set_with_fallthrough(
                            panic_i.future_seq().copied(),
                            panic_i.look_aheads().iter().copied(),
                        )?
                        .contains(&term)
                    {
                        return Ok(PanicAction::Shift(raw_expected, to));
                    }
                }
                Some(_) => {}
                None => {
                    // 直接 reduce
                    let prod = self.grammar().index_of_prod(i.prod()).unwrap();
                    if prod == 0 {
                        return Ok(PanicAction::Accept);
                    } else {
                        return Ok(PanicAction::Reduce(prod));
                    }
                }
            }
        }
        Ok(PanicAction::Empty)
    }
}
