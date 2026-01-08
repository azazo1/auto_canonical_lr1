//! 一个最右推导的简单实现, 使用 LR(1) 文法.
//!
//! 使用给定的 CFG 文法 (固定不变), 分析给定的一段 tokens (从标准输入读取) 的语法结构, 并给出其最右推导, 并进行简单的错误处理.
//! 参考龙书中文第二版 P160
//! ```text
//! 令 a 为 w$ 的第一个符号;
//! while (1) { /* 永远重复 */
//!     令 s 是栈顶的状态;
//!     if (ACTION[s, a] = 移入 t) {
//!         将 t 压入栈中;
//!         令 a 为下一个输入符号;
//!     } else if (ACTION[s, a] = 归约 A -> beta) {
//!         从栈中弹出 | beta | 个符号;
//!         令 t 为当前的栈顶状态;
//!         将 GOTO[t, A] 压入栈中;
//!         输出产生式 A -> beta;
//!     } else if (ACTION[s, a] = 接受) break; /* 语法分析完成 */
//!     else 调用错误恢复例程;
//! }
//! ```
use std::io::{self};

use bumpalo::Bump;
use lr_analysis::{
    ActionCell, EOF, EPSILON, Family, Grammar, Table, Terminal, Token, panic::PanicAction,
};
use tracing::{debug, error, info, warn};

fn shift<'a, I>(
    // 要压入的状态
    state: usize,
    // 要压入的非终结符.
    term: Terminal<'a>,
    stack: &mut Vec<usize>,
    term_stream: &mut impl Iterator<Item = I>,
    step: &mut Vec<Token<'a>>,
    family: &Family<'a>,
) {
    term_stream.next();
    debug!("I_{state}: {:#?}", family.item_sets().get(state));
    stack.push(state);
    step.push(term.into());
    debug!("step after shift: {step:?}");
}

#[allow(clippy::too_many_arguments)]
fn reduce<'a>(
    // 归约产生式.
    prod: usize,
    // 当前的 token 指针
    cursor: usize,
    stack: &mut Vec<usize>,
    steps: &mut Vec<(Vec<Token<'a>>, usize)>,
    step: &mut Vec<Token<'a>>,
    grammar: &Grammar<'a>,
    family: &Family<'a>,
    table: &Table<'a>,
) {
    // 获取产生式 A -> beta
    let prod = grammar.prods().get(prod).unwrap();
    info!("reduce production: {prod}");
    // 记录当前的归约操作情况.
    steps.push((step.clone(), cursor));
    debug!("step before reduce: {step:?}");
    for tok in prod
        .tail()
        .iter()
        .filter(|t| !matches!(t, Token::Terminal(EPSILON)))
        .rev()
    {
        // 去除 token 栈中的 |beta| 个 token.
        let popen = step.pop().unwrap();
        debug!("\npoping  : {popen}\nexpected: {tok}");
        assert_eq!(popen, *tok);
        // 去除状态栈中的 | beta | 个状态, 因为是从 项 A -> dot beta
        // 一路走到 项 A -> beta dot 进行了归约, 其中栈新增了 | beta | 个状态.
        stack.pop().unwrap();
    }
    // 非终结符 A 入 token 栈.
    step.push(prod.head().into());
    debug!("step after reduce: {step:?}");
    // 栈不会为空.
    let top = *stack.last().unwrap();
    info!("goto check, top: I_{}, prod head: {}", top, prod.head());
    if let Some(new_state) = table.goto(top, prod.head()).unwrap() {
        info!("reduce goto {new_state}");
        debug!("I_{new_state}: {:#?}", family.item_sets().get(new_state));
        stack.push(new_state);
    }
}

fn main() {
    #[cfg(debug_assertions)]
    {
        use tracing::level_filters::LevelFilter;
        use tracing_subscriber::{
            Layer, fmt, layer::SubscriberExt, registry, util::SubscriberInitExt,
        };

        let layer = fmt::layer()
            .without_time()
            .with_writer(io::stderr)
            .with_filter(LevelFilter::DEBUG);
        registry().with(layer).init();
    }

    let bump = Bump::new();
    // 自动分析 LR(1) 语法 (规范 LR(1) 项)
    let symbol_start = "program".into();
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
simpleexpr -> ID | NUM | ( arithexpr )
"#,
        symbol_start,
        &bump,
    )
    .unwrap()
    .augmented();
    // 计算集族
    let family = Family::from_grammar(&grammar);
    // 计算语法分析表
    let table = Table::build_from(&family, &grammar);
    assert!(!table.conflict());

    // 输入程序, 这个程序在 ID = NUM 这行出错, 少了个 `;`.
    let input = r#"{
while ( ID == NUM )
{
ID = NUM
}
}"#;
    // Vec<(行号, Terminal)>
    let mut terms: Vec<_> = input
        .lines()
        .enumerate()
        .flat_map(|(ln, s)| {
            s.split_whitespace()
                .map(move |part| (ln, Terminal::from(part)))
        })
        .collect();
    // iter -> (Terminal 编号, (Terminal 行号, Terminal))
    let term_stream: Box<dyn Iterator<Item = (usize, (usize, Terminal))>> =
        Box::new(terms.iter().copied().enumerate());
    let mut term_stream = term_stream.peekable();

    // 检查文法分析情况.
    // debug!("{:#?}", &grammar);

    // 状态栈
    let mut stack = vec![0]; // 放入初始项集

    // 记录归约的过程, 翻转过来就是最右推导的过程.
    // 每个单元表示:
    // (
    //     当前归约状态 step,
    //     当前归约状态没有读取的输入 term 起始位置 (可能大于等于 terms 的长度, 也就是说后面没有未被读取的输入 term)
    // )
    let mut steps: Vec<(Vec<Token>, usize)> = Vec::new();
    // 记录当前步的 tokens.
    let mut step: Vec<Token> = Vec::new();
    // 语法分析
    loop {
        // 栈不会为空, 因为 pop 之前一定要有对应数量的状态被压入 (产生式尾部的 token 数量压入, 同样数量弹出).
        let top = *stack.last().unwrap();
        let (cursor, (ln, term)) = term_stream
            .peek()
            .copied()
            .unwrap_or((usize::MAX, (usize::MAX, EOF)));
        let action = table.action(top, term).unwrap();
        info!("top: I_{top}, term: {term}, cursor: {cursor}, action: {action:?}");
        match action {
            ActionCell::Shift(state) => {
                shift(
                    *state,
                    term,
                    &mut stack,
                    &mut term_stream,
                    &mut step,
                    &family,
                );
            }
            ActionCell::Reduce(prod) => {
                reduce(
                    *prod, cursor, &mut stack, &mut steps, &mut step, &grammar, &family, &table,
                );
            }
            ActionCell::Conflict(_, _) => unreachable!(),
            ActionCell::Accept => {
                reduce(
                    0, cursor, &mut stack, &mut steps, &mut step, &grammar, &family, &table,
                );
                break;
            }
            ActionCell::Empty => {
                error!("error on I_{top}, term: {term}");
                debug!(
                    "actions of I_{top}: {:#?}",
                    table.actions(top).unwrap().collect::<Vec<_>>()
                );
                // 错误处理
                let panic_action = table.panic_action(top, term).unwrap();
                if !panic_action.is_empty() {
                    info!("panic recover: {action:?}");
                }
                match panic_action {
                    PanicAction::Reduce(prod) => {
                        // 在此处忽略错误, 延迟报告.
                        // println!("语法错误，第{}行，非预期的\"{}\"", ln, term);
                        reduce(
                            prod, cursor, &mut stack, &mut steps, &mut step, &grammar, &family,
                            &table,
                        );
                    }
                    PanicAction::Shift(skipped, to) => {
                        println!("语法错误，第{}行，缺少\"{}\"", ln, skipped);
                        // 尝试添加 skipped 终结符来修正整个程序结构.
                        drop(term_stream);
                        terms.insert(cursor, (ln, skipped));
                        // 重新构建 terms 流, 相当与把程序当成原本就是被修正过的版本.
                        let tmp_term_stream: Box<dyn Iterator<Item = (usize, (usize, Terminal))>> =
                            Box::new(terms.iter().copied().enumerate().skip(cursor));
                        term_stream = tmp_term_stream.peekable();
                        shift(
                            to,
                            skipped,
                            &mut stack,
                            &mut term_stream,
                            &mut step,
                            &family,
                        );
                    }
                    PanicAction::Accept => {
                        reduce(
                            0, cursor, &mut stack, &mut steps, &mut step, &grammar, &family, &table,
                        );
                        break;
                    }
                    PanicAction::Empty if term == EOF => {
                        error!("panic escaped.");
                        break;
                    }
                    PanicAction::Empty => {
                        warn!("panic continued, term skipped: {term}");
                        // 无法从恐慌状态下恢复, 跳过这个 token.
                        term_stream.next();
                    }
                }
            }
        }
    }

    // 输出最右推导 (规约步骤翻转过来).
    for (idx, (step, cursor)) in steps.into_iter().enumerate().rev() {
        let line = step
            .into_iter()
            .map(|tok| format!("{tok} "))
            .collect::<String>();
        let supplement = terms
            .iter()
            .skip(cursor)
            .map(|t| format!(" {}", t.1))
            .collect::<String>();
        if idx == 0 {
            println!("{}{}", line.trim_end(), supplement);
        } else {
            println!("{}{} =>", line.trim_end(), supplement);
        }
    }
}
