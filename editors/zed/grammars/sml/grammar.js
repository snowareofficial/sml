/**
 * Tree-sitter grammar for SML (SNOWARE Markup Language).
 *
 * Zed 用 Tree-sitter 做语法高亮（不走 TextMate scope），所以 SML 的 Zed 扩展需要
 * 这份 grammar。它**不是**解析器替代品：`swsml`（Rust/C-ABI/JS/Lua）才是权威实现，
 * 这里只保证「词法/结构能被正确切分、且没有 ERROR 节点」，好让高亮与括号匹配工作。
 *
 * ## 与查询文件（highlights.scm）的契约 —— 改这儿就得同步改那儿
 *
 * 下面这些**具名节点**是高亮查询按名字捕获的，重命名会让高亮静默失效：
 *
 * | 节点 | 对应写法 | 用于 |
 * |---|---|---|
 * | `comment` | `# …` / `-- …` / `// …` / `/* … *​/` / `_* … *_` | `@comment` |
 * | `string` | `"…"`（含 `escape_sequence` 子节点） | `@string` |
 * | `number` | `27` / `-3` / `1.5` / `0x20` / `0b1010` / `1e10` | `@number` |
 * | `boolean` | `true` / `false` | `@boolean` |
 * | `null` | `null` | `@constant` |
 * | `env_var` | `$env.PORT` | `@variable` |
 * | `fragment_ref` | `&base` | `@variable` |
 * | `key` | `foo:` 里的 `foo` | `@property` |
 * | `type_name` | `@contract Foo` / `@is Foo` 里的 `Foo` | `@type` |
 * | `directive` | `@form` / `@contract`（**含 `@`**） | `@keyword` |
 *
 * ⚠️ `directive` 节点的文本**包含 `@`**（即 `@form` 而非 `form`）。`smltools --to
 * highlight` 生成的 `zed/highlights.scm` 里的谓词因此写成 `^@(form|policy)$`。
 * 谁改了这一条，谁的查询就会匹配不上。
 *
 * ## 已知与权威实现的差异（都是刻意的，均因 Tree-sitter 正则无环视）
 *
 * - `a--b`：`swsml` 的 lexer 会切成 `a` + 行注释；这里会整段当一个裸词。
 * - 数字仍会被 `word` 匹配，靠优先级让 `number` 胜出（`prec(1)`）。
 * - 块冒号可省（`address { }`）用 `prec.right` 让 shift 胜过 reduce，
 *   使 `{` 归属到前面的键，而不是变成并列的下一个块。
 */
module.exports = grammar({
  name: 'sml',

  // 空白与注释可以出现在任何地方
  extras: $ => [/\s/, $.comment],

  // 供 Tree-sitter 的关键字提取使用（错误恢复时把裸词当同一类 token）
  word: $ => $.word,

  rules: {
    source_file: $ => repeat($._item),

    _item: $ => choice(
      $.pair,
      $.directive_phrase,
      $.block,
      $.array,
      $._value,
    ),

    // `key: value`；值可省（`key:` 后跟缩进块，或 `key { }` 省略冒号）
    pair: $ => prec.right(seq(
      field('key', $.key),
      choice(
        seq(':', optional(field('value', $._value))),
        // 块冒号可省：`address { streetAddress: "21 2nd Street" }`
        field('value', $.block),
      ),
    )),

    key: $ => choice($.word, $.string),

    block: $ => seq('{', repeat(choice($._item, ',')), '}'),
    array: $ => seq('[', repeat(choice($._item, ',')), ']'),

    // `@contract Foo { … }` / `@is Foo` / `@base { … }` / `@include "x.sml"`
    //
    // 刻意**不**按指令名区分语义：那需要文本敏感的判断（`@is` 与 `@contract` 形状相同），
    // 在无环视的 LR 语法里做只会引入冲突。高亮那边用 `#match?` 谓词限定即可。
    directive_phrase: $ => prec.right(seq(
      $.directive,
      repeat($._directive_arg),
      optional($.block),
    )),

    _directive_arg: $ => choice(
      $.type_name,
      $.string,
      $.env_var,
      $.fragment_ref,
    ),

    // `@contract Foo` / `@is Foo` 里的 `Foo` —— 查询侧再用谓词限定到这三个核心指令
    type_name: $ => $._word_body,

    _value: $ => choice(
      $.string,
      $.number,
      $.boolean,
      $.null,
      $.env_var,
      $.fragment_ref,
      $.block,
      $.array,
      $.word,
    ),

    // ---- 词法 ----

    comment: $ => token(choice(
      seq('#', /.*/),
      seq('--', /.*/),
      seq('//', /.*/),
      seq('/*', /[^*]*\*+([^/*][^*]*\*+)*/, '/'),
      seq('_*', /[^*]*\*+([^_][^*]*\*+)*/, '_'),
    )),

    string: $ => seq(
      '"',
      repeat(choice($.escape_sequence, /[^"\\]+/)),
      '"',
    ),

    // 与 sml-lex 一致：\n \t \r \0 \" \\ \uXXXX \u{XXXX}
    escape_sequence: $ => token.immediate(seq(
      '\\',
      choice(
        /[ntr0"\\]/,
        /u\{[0-9a-fA-F]+\}/,
        /u[0-9a-fA-F]{4}/,
      ),
    )),

    number: $ => token(prec(1, seq(
      optional(choice('-', '+')),
      choice(
        /0[xX][0-9a-fA-F_]+/,
        /0[oO][0-7_]+/,
        /0[bB][01_]+/,
        /[0-9][0-9_]*/,
      ),
      optional(seq('.', /[0-9][0-9_]*/)),
      optional(seq(/[eE]/, optional(choice('-', '+')), /[0-9][0-9_]*/)),
    ))),

    boolean: $ => token(prec(1, choice('true', 'false'))),
    null: $ => token(prec(1, 'null')),

    env_var: $ => token(prec(1, /\$env\.[A-Za-z_][A-Za-z0-9_.-]*/)),
    fragment_ref: $ => token(prec(1, /&[A-Za-z_][A-Za-z0-9_.-]*/)),

    // 含 `@`：见文件头「与查询文件的契约」
    directive: $ => token(prec(1, /@[^\s{}[\],:"#]+/)),

    // 裸词兜底：不含空白与结构字符（`@` 只许出现在词中，故 `a@b.c` 仍是裸词）
    word: $ => $._word_body,
    _word_body: $ => token(prec(-1, /[^\s{}[\],:"#]+/)),
  },
});
