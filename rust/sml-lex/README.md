# sml-lex

SML 词法分析：tokenize、裸词强制转换、环境变量内联。

**English below** ｜ 中文在上方，English 在下方

---

# 中文

`sml-lex` 把 SML 文本切成 `Tok` 序列，并把裸词强制转换为 `Value`。它只依赖 `sml-value` 与 `sml-feature`，不接触语法结构，便于单独测试与复用。

## 安装

```toml
[dependencies]
sml-lex = "0.1.0-alpha.1"
```

## 快速开始

```rust
use sml_lex::tokenize;

let toks = tokenize("name: John\nage: 27")?;
for tok in toks {
    println!("{:?}", tok);
}
```

## 许可

MulanPSL-2.0

---

# English

`sml-lex` tokenizes SML text into a `Tok` sequence and coerces bare words into `Value`s. It only depends on `sml-value` and `sml-feature`, staying clear of syntax structures so it can be tested and reused independently.

## Installation

```toml
[dependencies]
sml-lex = "0.1.0-alpha.1"
```

## Quick start

```rust
use sml_lex::tokenize;

let toks = tokenize("name: John\nage: 27")?;
for tok in toks {
    println!("{:?}", tok);
}
```

## License

MulanPSL-2.0
