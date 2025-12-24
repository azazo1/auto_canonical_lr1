# auto LR

自动计算规范 LR(1) 文法的规范 LR(1) 项集族和语法分析表.

使用方法:

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

3. 获取输出(`output.txt`), 预期是能够输出项集族和语法分析表, 示例文法的输出见: [output](output.txt).

## 特殊终结符

- eof: 使用 "eof" 表示 token 流末尾.
- E: 使用 "E" 表示 $\epsilon$ 终结符.
