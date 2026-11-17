# 静默失败台账（退出 0，但结果已经错了）

> **产品线** repo ｜ **状态** current ｜ **最后核对** 2026-11-17
>
> 判据：**退出码 0、没有任何诊断、产物看起来正常，但值树或文本已经错了。**
> 相邻文档：`docs/swsml/ref/silent-divergences.md`（跨端差异，不是本台账）、
> `docs/swsml/ref/grammar-divergences.md`（语法维度）、`docs/repo/ref/unfinished.md`（还剩什么）。
>
> 本台账只收「该报错而没有报错」的条目。**有错误码、有警告的不算**——那些至少会喊。

## 0. 一览

| 编号 | 触发 | 实际结果 | 危害 | 归类 |
|---|---|---|---|---|
| S1 | 值位置写了多个裸词（中文句子最常见） | 多出来的词被当成**新键**，值被截断 | 静默错树 | 缺陷 |
| S2 | 键名里用全角 `：`（中文输入法默认） | 两行被并成一对键值；或报错但**指错行** | 静默错树 / 误导 | 缺陷 |
| S3 | 未知扩展名或 stdin | 一律按 SML 解析 | 静默错树 | 缺陷（归 I-34） |
| S4 | 文档形态不是该后端认识的块 | 产出**空 body** 的 HTML，exit 0 | 空产物 | 缺陷 |
| S5 | 根块名不在后端词表里 | 按普通字段渲染，混进 `- **children**:` 之类的噪音 | 结构错 | 缺陷 |
| S6 | `--to custom` 模板里没有字段占位符 | 用户给的参数被**静默丢弃**，输出恒定 | 静默错值 | 缺陷 |
| S7 | `math` 块里的公式字符串 | 原样透传，不平衡的 `{`、拼错的 `\omegga` 都不报 | 编译期才炸 | 设计取舍 |
| S8 | 同名块聚合成数组（SML 的正常语义） | `--to custom` 的 `{value}` / `{item}` 取到空串，输出 `\[  \]` | 静默空产物 | 缺陷 |

## 1. S1 · 值里的多个裸词变成额外键

```sml
h1 { text: 为什么用 SML 写，再交给 LaTeX 排 }
```

实际（`--to json`，exit 0，无诊断）：

```json
{"h1":{"LaTeX":"排","SML":"写，再交给","text":"为什么用"}}
```

原因：`scalar ::= STRING | WORD`（`grammar.sml:118`），**一个 WORD 不含空白**；
`field ::= key [ : ] value`（`grammar.sml:147`），冒号可省 ⇒ 后面的词自然被读成新键。

期望：值位置出现「裸词 + 空白 + 裸词」时直接报错，或至少给一条 W 级诊断
「值含空格，需要引号」。中文用户尤其容易撞：中英混排的句子天然带空格。

## 2. S2 · 全角冒号 + 冒号可省

```sml
配置 {
    端口：8080
    主机：127.0.0.1
}
```

实际：

```json
{"配置":{"端口：8080":"主机：127.0.0.1"}}
```

原因：只有 ASCII `:` 是分隔符（`grammar.sml:108` 的 token 表），整行 `端口：8080` 是一个 WORD；
再叠加上一条的「冒号可省」，下一行就成了它的值。

三个变体，行为不一致：

| 写法 | 结果 |
|---|---|
| 块内两行全角冒号 | 静默配成一对键值（上例） |
| 全角冒号夹在正常键之间 | `E-PARSE-006`，但位置报**第 4 行**，真凶在第 3 行 |
| 顶层单独一行全角冒号 | `E-PARSE-008`「顶层须为容器」——与全角冒号毫无关系 |

期望：键位置 WORD 里出现 `：` / `，` 时直接指出来（「疑似全角标点，请换成半角 `:`」）。
这是中文用户的第一号雷区：输入法默认就给全角。

## 3. S3 · 未知扩展名一律当 SML

```text
$ cat x.txt
{"name": "web", "port": 8080}
$ smltools -i x.txt --to json
{"name":"web","port":8080}
```

`InputFormat::detect`（`rust/smltools/src/main.rs:124`）只看扩展名，未知即回落 `Sml`。
JSON 恰好是 SML 的子集，所以这次「碰巧对了」；换成 TOML 就会得到一棵怪树。

修法与验收已登记为 **I-34**（自动识别），本台账只作证据。

## 4. S4 · 后端不认块形态 ⇒ 空产物

```sml
doc {
    children: [
        h1 { text: "标题" }
        p { text: "正文。" }
    ]
}
```

- `--to html`：`<body>\n</body>`，exit 0，无警告（`<title>未命名</title>` 也照写）
- `--to latex`：同一份文件正常产出 `\section{标题}` 与正文

原因：HTML 后端只在 `__type` 命中时渲染（`rust/src/emit/html.rs:192` 的 `match ty`），
而 `doc` / `h1` 这种「键 + 块」的写法没有 `__type`；LaTeX 后端则用键名兜底。
**同一份输入，一个后端出文档、另一个出空页，都不报错。**

正确形态（会渲染）是裸块：

```sml
topic 导言 {
    children: [ para { text: "正文。" } ]
}
```

期望：树非空而**零节点被消费**时给 W 级诊断（「N 个块未被该后端识别」），
或直接报错。空产物永远不该是成功状态。

## 5. S5 · 根块名不认识 ⇒ 按普通字段渲染

同一份 `doc { children: [...] }`，`--to md` 实际输出：

```text
    - **children**:
    # 标题

    正文。
```

标题与正文是对的，但凭空多出一条 `- **children**:` 列表项。
原因：Markdown 后端不认识 `doc` 这个根块，退回「按字段渲染」，于是把 `children` 当普通字段展开。

期望：与 S4 同一条诊断。同一处还会把元数据当正文输出：未识别的块在 `--to md` 下会把
`__featags` 渲染成 `- **__featags**:` 列表项（实测），元数据不该出现在产物里。

## 6. S6 · 模板没有字段占位符 ⇒ 参数被静默丢弃

规则表（`--to custom --custom-rules`）：

```sml
rules: [
    { match-key: "傅里叶变换" template: "\\[ F(\\omega) = \\int_{-\\infty}^{\\infty} f(t)\\,e^{-i\\omega t}\\,dt \\]\n" }
]
```

输入 `傅里叶变换 { 函数: g 变量: s }`，实际输出：

```tex
\[ F(\omega) = \int_{-\infty}^{\infty} f(t)\,e^{-i\omega t}\,dt \]
```

参数 `g` / `s` **一个都没用上**，也不报错。占位符只有
`{value}` / `{raw}` / `{key}` / `{item}` / `{nested}` / `{items:TPL}`（`rust/src/emit/custom.rs:22`），
没有字段级占位符，模板里也没地方写参数名。

期望二选一：① 补 `{field:名}`，缺字段时按契约报错；② 若模板未引用任何字段而输入块里有字段，
给 W 级诊断。**静默丢参数比报错严重**：用户会以为自己的 `g(s)` 生效了。

## 7. S7 · math 只透传，零校验（设计取舍）

```sml
破公式 {
    math { text: "\\frac{a}{b" }
}
```

实际（`--to latex --math`，exit 0）：

```tex
\item[math] $\frac{a}{b$
```

括号不平衡、`\omegga` 这类拼错，SML 一律不看——它只把字符串搬过去。这是**设计取舍**：
SML 不做数学排版。但对「不会 LaTeX 的人」而言，这等同于静默失败（要等 TeX 编译才炸）。
记录在此，是为了让下一条修法有据：真要服务这类用户，需要模板层，而不是给字符串加校验。

## 8. S8 · 同名块聚合成数组后，模板取到空串

同名块聚合成数组是 SML 的正常语义（`grammar.sml:162`），值树没错；错在消费侧：
`--to custom` 的 `{value}` 只认标量/单块。

一个块时正确（featag 落点写进 `text`，`{value}` 取到它）：

```sml
公式 !latex["\\int_0^1 x\\,dx = \\frac{1}{2}"], 积分 { }
```

```tex
\[ \int_0^1 x\,dx = \frac{1}{2} \]
```

两个同名块时（SML 自动聚合成 2 元素数组）：

```tex
\[  \]
```

两块都变空。换 `{items:\[ {item} \]}` 也不行：数组元素是对象，`{item}` 取「元素文本」，
同样是空串 —— 只得到两个 `\[  \]`。

期望：数组元素进 `{value}` 时取该元素的 `text`；取不到就报错。**空模板不该是成功状态。**

## 9. 共同根因

三条「默认成功」：

1. **解析器的边界假设**：冒号可省、裸词以空白断词 ⇒ 少写一个引号不报错，句子被拆成键值对。
2. **后端的类型假设**：不认识就跳过或按普通字段降级 ⇒ 整篇空、或混进噪音字段。
3. **模板的字段假设**：没有占位符就把字段丢掉、形状一变成数组就取到空串 ⇒ 参数写了等于没写（S6 / S8）。

一句话：**不认识就降级，不报错。** 降级本身没错，错在没有声音。

## 10. 修法（按性价比排）

| 优先级 | 条目 | 动作 | 规模 |
|---|---|---|---|
| P1 | S1 / S2 | 加 lint，不动语法：键位置 WORD 含全角 `：` `，`；值位置出现「多个裸词」⇒ 指明行号并给正确写法 | 小（诊断层） |
| P1 | S8 | 数组元素进 `{value}` 时取该元素的 `text`；取不到即报错 | 小 |
| P1 | S6 | `custom` 后端补 `{field:名}`：取字段值，缺失时报错（S8 修好后非必需，但能覆盖落点不叫 `text` 的情形） | 小（约 50 行 + 单测） |
| P2 | S4 / S5 | 「树非空、零节点被消费」⇒ W 级诊断；码需先在 `errors/codes.sml` 登记再生成五端 | 中（涉及码表生成） |
| P2 | S3 | 归 I-34 自动识别 | 见 I-34 |
| — | S7 | 不改行为，只补文档 | 文档 |

验收口径统一：**S1–S6、S8 的最小复现要么报错、要么有 W 级诊断；不得 exit 0 且无声。**

## 11. 复核命令

各节代码块另存为 `s1.sml` / `s2.sml` / `doc.sml` / `in.sml` / `rules.sml` / `s7.sml` / `f1.sml` / `f2.sml` / `f_rules.sml` 后，
**在 `rust/` 目录下**执行：

```bash
cargo run -q -p smltools -- -i s1.sml --to json                              # S1 错树
cargo run -q -p smltools -- -i s2.sml --to json                              # S2 错树
cargo run -q -p smltools -- -i doc.sml --to html                             # S4 <body> 为空
cargo run -q -p smltools -- -i doc.sml --to latex                            # S4 同输入有输出
cargo run -q -p smltools -- -i doc.sml --to md                               # S5 多出 - **children**:
cargo run -q -p smltools -- -i in.sml --to custom --custom-rules rules.sml   # S6 参数被丢
cargo run -q -p smltools -- -i s7.sml --to latex --math                      # S7 不平衡括号照过
cargo run -q -p smltools -- -i f1.sml --to custom --custom-rules f_rules.sml # S8 单块正确
cargo run -q -p smltools -- -i f2.sml --to custom --custom-rules f_rules.sml # S8 两块变空
```

判据：上面除 `f1.sml`（这是**正确基线**）之外，**目前全部 exit 0 且无诊断**；哪天它们开始报错或给警告，本台账就该删条目。
