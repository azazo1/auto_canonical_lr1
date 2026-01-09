# LR Analysis

自动计算规范 LR(1) 文法的规范 LR(1) 项集族和语法分析表.

给定 CFG 文法, 无需手动计算 first 集, follow 集, LR(1) 项集, LR(1) 项集族, GOTO 表, ACTION 表等, 全部自动生成,
并自动生成恐慌恢复机制.

## 项目结构

### 编译原理相关

- `src/token.rs`: 结构化终结符和非终结符, 语义化结构, 而不是简单地使用字符串切片统一代表终结符和非终结符.
  - token (Terminal / NonTerminal, <del>token 实际上应该叫做 symbol</del>) 的存储以及下面各个数据结构都使用借用的方式存储原始数据, 充分利用零拷贝提升效率.
- `src/grammar.rs`: 进行文法的解析, 计算产生式, 增广文法, first 集 (follow 集不需要计算, 可由具体的 symbol 序列的 first 集代替).
  - first 集的计算使用采用带状态标记的记忆化递归算法; 配合懒计算, 只有真正在用到时才会计算并存储 first 集.
- `src/item.rs`: 对文法解析结果进一步解析 LR(1) 项, 项集及项集闭包和项集族.
- `src/table.rs`: 基于项集族和文法产生语法分析表, 提供 action 表和 goto 表, 并自动判断文法是否为合法的 LR(1) 文法 (二义性, 不可表示).
- `src/panic.rs`: 对语法分析表进行拓展, 自动计算恐慌恢复动作.
- `src/main.rs`: 解析全过程可视化输出.
- `examples/rightmost_derivation.rs`: 适用于课程测试平台 LR parser 的程序, 已经提交验证通过.

### 项目构成相关

- `src/error.rs`: rusty 错误处理, 确保每步解析程序产生的错误直观可追溯.
- `src/lib.rs`: crate 入口.
- `src/macros.rs`: 暂未用到.
- 每部分解析代码都伴随单元测试, 确保结果正确性.
- 项目绝大多数使用语义化结构, 以明显的语义表示终结符(Terminal), 非终结符(NonTerminal), 产生式(Production), 文法(Grammar), 项(Item), 项集(ItemSet), 项集族(Family), 语法分析表(Table), 语法分析表 Action (ActionCell) 等, 消除 `Vec<String>` (cpp 中的 `vector<string>`) 结构带来的不明确语义, 确保了类型安全.

## 使用方法

需要准备 rust 环境: [安装](https://rust-lang.org/zh-CN/tools/install/).

1. 准备输入文法(`input.txt`):

   ```text
   program -> compoundstmt
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
   multex_prprime -> * simpleexpr multexprprime | / simpleexpr multexprprime | E
   simpleexpr -> ID | NUM | ( arithexpr )
   ```

2. 运行:

   ```shell
   cargo run -q -- --symbol-start program < input.txt &> output.txt
   ```

3. 获取输出(`output.txt`), 预期是能够输出 LR(1) 项集族和语法分析表, 示例文法的输出见: [output](output.txt).

## 特殊终结符

- eof: 使用 "eof" 表示 token 流末尾.
- E: 使用 "E" 表示 $\epsilon$ 终结符.

## 示例程序

`rightmost_derivation` 是一个简单的使用此 crate 进行最右推导分析的示例程序.

运行方法:

```shell
cargo run --example rightmost_derivation
```
