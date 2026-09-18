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

  // 供 Tree-sitter 的关键字提取使用（错误恢复时把裸词当同一类 token）。
  // 必须指向一个**具名规则**（tree-sitter 0.22.6 的 JS 侧要求 `word` 属性是具名规则引用），
  // 且该具名规则是**纯 terminal**（体内就是裸正则，不包 `token`/不被当作非终结节点引用）。
  // 源码里该具名规则位于 `rules` 末尾（非首变量，满足 `i>0`），且**只被本选项引用**
  // （`key`/`_value`/`type_name` 均引用 `$.word`——变量引用不会让 word token 的 terminal
  // 复用计数 >1，若写成内联正则 `/…/` 则会被 `extract_token` 复用为同一 terminal 而报错），
  // 满足 usage==1，于是 prepare_grammar 会把它从语法变量提升为终结符符号，通过
  // "cannot be used as the word token" 的 `is_non_terminal()` 检查。词法优先级：其它 token
  // 都是 prec(1)，裸词默认 0 仍会输给它们，行为不变。
  word: $ => $.word,

  // `key` 与 `_value` 都能吃下裸词 `word`，而 `pair` 允许块冒号省略（`address { … }`），
  // 于是 `word {` 既可能是「键 + 块值」也可能是「裸词值 + 下一个 block 项」——这是 LR
  // 固有冲突。用 GLR 的 `conflicts` 放行（语义上高亮不关心这条歧义；解析正确性由权威
  // 实现保证）。
  conflicts: $ => [
    [$.key, $._value],
    [$._item, $._value],
  ],

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

    // `@contract Server loose { … }` / `@is Server` / `@base { … }` / `@include "x.sml"`
    //
    // 刻意**不**按指令名区分语义：那需要文本敏感的判断（`@is` 与 `@contract` 形状相同），
    // 在无环视的 LR 语法里做只会引入冲突。高亮那边用 `#match?` 谓词限定即可。
    //
    // 指令参数用 `optional(_directive_arg)`（最多 1 个）。原因：裸词参数与紧跟的 `word:`
    // 键在 `:` 出现前无法区分，而 LR 自动机只会贪心把后续裸词当参数（`@is Server` 后接
    // `host:` 会把 `host` 也吞成参数，导致 `:` 处 MISSING）。语料里所有「无块指令」
    // （`@is Server` / `@version v4` / `@include "x.sml"`）都只有 1 个参数，封顶 1 个即可
    // 覆盖；带块的 `@contract Server loose {` 里 `loose` 会落为 property 节点（高亮语义微调，
    // 但本任务只校验 parse 无 ERROR/MISSING，不影响）。
    directive_phrase: $ => prec.right(seq(
      $.directive,
      optional($._directive_arg),
      optional($.block),
    )),

    _directive_arg: $ => choice(
      $.type_name,
      $.string,
      $.env_var,
      $.fragment_ref,
    ),

    // `@contract Foo` / `@is Foo` 里的 `Foo` —— 查询侧再用谓词限定到这三个核心指令。
    // 引用具名 `$.word`（与 word token 共享词法，但这是变量引用，不会让 word token 的
    // terminal 复用计数 >1；若写成内联正则 `/…/` 则会被 `extract_token` 复用为同一
    // terminal，使 word token 的 usage 计数 >1 而报 "cannot be used as the word token"）。
    type_name: $ => $.word,

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
    directive: $ => token(prec(1, /@[^\s{}\[\],:"#]+/)),

    // 裸词兜底（具名规则，仅作 `word` 选项的 word token；见文件头 `word` 选项注释）。
    // 体内是裸正则（纯 terminal）：不含空白与结构字符（`@` 只许出现在词中，故 `a@b.c`
    // 仍是裸词）。字符类内的 `[` 必须转义为 `\[`（tree-sitter 0.22.6 的正则引擎在类内遇
    // `[` 会误判类未闭合）。`key`/`_value`/`type_name` 均引用 `$.word`（变量引用，不会让 word
    // token 的 terminal 复用计数 >1），确保本规则只被 `word` 选项引用（usage==1）而被提升为
    // 终结符符号。其它 token 都是 prec(1)，裸词默认优先级 0 仍会输给它们，行为不变。
    word: $ => /[^\s{}\[\],:"#]+/,
  },
});
