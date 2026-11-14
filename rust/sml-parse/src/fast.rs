//! 单遍解析的**快速路径**：覆盖「扁平标量 + 匿名嵌套块 + 引号串」的文档。
//!
//! 背景（见 `docs/swsml/ref/perf-baseline.md` 的「与 serde_json 的对拍」）：
//! 通用路径是**两遍** —— 先把文本切成 `Vec<Tok>`（48 B/token，约为输入体积的 8 倍），
//! 再在 token 游标上建树。词法这一遍占 20–23%，且 token 缓冲区把输入挤出 cache。
//!
//! 本模块对**一部分**文档做真正的单遍扫描：边读字节边建树，不留 token 流。
//!
//! # 能力边界（宁可不接，也不猜）
//!
//! 只要文本里出现下列任一字符就**整篇回落**通用路径：
//! `[ ] @ $ ! \` ' \ , ; #` —— 数组 / 指令 / 环境变量 / 模板插值 / 单引号串 / 分隔符 / 注释
//! 全不接。这几样恰好是语义分歧最容易发生的地方（`;` 会变成键、注释边界），
//! 留给通用路径处理。
//!
//! **块只接匿名块**（`key { … }`）：`Type name { … }` 这种**带名裸块**会注入
//! `__type` / `__name`，且 `Type` 命中契约时还会触发 typed-block —— 本路径不碰。
//! 判定方式是「键之后、`{` 之前不许再出现词」，出现即回落。
//!
//! **引号串**（`"…"`）接，但解码必须走 [`sml_lex::unescape_quoted`] —— 与词法器
//! **共用同一份转义表**。裸词里出现反斜杠一律回落（那种输入在通用路径里另有语义）。
//!
//! # 一致性的三条硬约束
//!
//! 1. **值转换复用** [`sml_lex::coerce_word`]、**串解码复用** [`sml_lex::unescape_quoted`]：
//!    `1` / `1.5` / `true` / `null` / 转义规则都只有一份实现；
//! 2. **遇到任何异常就返回 `None`**（重复键、缺冒号、带名裸块、未闭合、超深…），
//!    由通用路径产出**原本那条**错误消息 —— 快速路径绝不自己发明错误；
//! 3. 因此它对外的可见行为只有两种：要么产出与通用路径**逐字节相同**的树，要么不接管。

use std::borrow::Cow;

use sml_feature::FeatureSet;
use sml_lex::{coerce_word, unescape_quoted};
use sml_value::{Map, Value};

/// 空片段表（本路径不接 `@片段`）。
const NO_FRAGMENTS: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();

/// 是否在能力范围内。一次轻量字节扫描，命中即否决。
///
/// `{` `}` **不在**否决表里（匿名块要接）；`"` 也不在（引号串要接）。
/// 它们的边界在解析时判（带名裸块 / 未闭合串 / 裸词里的反斜杠）。
fn eligible(text: &str) -> bool {
    !text.bytes().any(|c| {
        matches!(
            c,
            b'[' | b']' | b'@' | b'$' | b'!' | b'`' | b'\''
            // `,` `;` `#` 也一律不接：它们在通用路径里的归属很反直觉
            // （实测 `a: 1\nb: 2 ;` 会让 `;` 变成**键**），不值得复刻一份。
            | b',' | b';' | b'#'
        )
    })
}

struct Fast<'a> {
    text: &'a str,
    b: &'a [u8],
    i: usize,
    feats: FeatureSet,
}

impl<'a> Fast<'a> {
    fn ws(&mut self) {
        while self.i < self.b.len() && matches!(self.b[self.i], b' ' | b'\t' | b'\r' | b'\n') {
            self.i += 1;
        }
    }

    /// 读一个裸词（到空白 / 结构符号为止）。空或含反斜杠 ⇒ `None`（回落）。
    fn word(&mut self) -> Option<&'a str> {
        let s = self.i;
        while self.i < self.b.len() {
            match self.b[self.i] {
                b' ' | b'\t' | b'\r' | b'\n' | b':' | b'{' | b'}' | b'"' => break,
                _ => self.i += 1,
            }
        }
        if self.i == s {
            return None;
        }
        let w = &self.text[s..self.i];
        // 裸词里的反斜杠在通用路径里另有语义（续行 / 未转义），不复刻。
        if w.contains('\\') {
            return None;
        }
        Some(w)
    }

    /// 读一条引号串（`self.i` 指向开引号）。未闭合 / 转义非法 ⇒ `None`（回落）。
    fn quoted(&mut self) -> Option<Cow<'a, str>> {
        if self.i >= self.b.len() || self.b[self.i] != b'"' {
            return None;
        }
        self.i += 1;
        let start = self.i;
        let mut has_escape = false;
        loop {
            if self.i >= self.b.len() {
                return None; // 未闭合：交回通用路径报 E-LEX-001
            }
            match self.b[self.i] {
                b'"' => {
                    let end = self.i;
                    self.i += 1;
                    let content = &self.text[start..end];
                    return if has_escape {
                        // 转义表只有一份实现（与词法器共用）
                        unescape_quoted(content).ok().map(Cow::Owned)
                    } else {
                        Some(Cow::Borrowed(content))
                    };
                }
                b'\\' => {
                    has_escape = true;
                    self.i += 1;
                    // 跳过被转义的字符：按 UTF-8 边界走，避免切在多字节中间
                    let ch = self.text[self.i..].chars().next()?;
                    self.i += ch.len_utf8();
                }
                _ => {
                    let ch = self.text[self.i..].chars().next()?;
                    self.i += ch.len_utf8();
                }
            }
        }
    }

    /// 解析一个块（`top` 时解析到文件末尾）。
    ///
    /// 返回 `None` 表示「不接管」—— 所有失败都走这条路，绝不产生错误。
    fn block(&mut self, top: bool, depth: usize) -> Option<Value> {
        // 深度护栏：本路径不实现 E-LIMIT-001，超限一律交回通用路径
        // （那里既有「128 层放行 / 129 层报码」的判据，也有集成测试钉着）。
        if depth > sml_value::MAX_VALUE_DEPTH {
            return None;
        }
        let mut m = Map::new();
        loop {
            self.ws();
            if self.i >= self.b.len() {
                if top {
                    break;
                }
                return None; // 块未闭合：交回通用路径报 E-PARSE-001
            }
            if !top && self.b[self.i] == b'}' {
                self.i += 1;
                break;
            }
            let key: String = if self.b[self.i] == b'"' {
                self.quoted()?.into_owned()
            } else {
                self.word()?.to_string()
            };
            self.ws();
            if self.i >= self.b.len() {
                return None;
            }
            let v = match self.b[self.i] {
                b':' => {
                    self.i += 1;
                    self.ws();
                    self.value(depth)?
                }
                b'{' => {
                    // 匿名块：键与 `{` 之间**只允许空白**。带名裸块（`Type name {`）
                    // 在这里已经被 `word()` 吞进 key，后面紧跟的是词 → 落到 None。
                    self.i += 1;
                    self.block(false, depth + 1)?
                }
                _ => return None,
            };
            if m.contains_key(&key) {
                return None; // 重复键：通用路径要合并成数组并记诊断
            }
            m.insert(key, v);
        }
        Some(Value::Object(m))
    }

    fn value(&mut self, depth: usize) -> Option<Value> {
        if self.i >= self.b.len() {
            return None;
        }
        match self.b[self.i] {
            b'{' => {
                self.i += 1;
                self.block(false, depth + 1)
            }
            b'"' => self.quoted().map(|c| Value::Str(c.into_owned())),
            _ => {
                let w = self.word()?;
                coerce_word(w, &NO_FRAGMENTS, self.feats, "", None).ok()
            }
        }
    }
}

/// 尝试单遍解析。
///
/// - `Some(value)`：**确定**与通用路径等价（调用方可直接返回）；
/// - `None`：不在能力范围内，或遇到任何需要通用路径处理的情形 —— 调用方必须回落。
pub(crate) fn try_parse_flat(text: &str, feats: FeatureSet) -> Option<Value> {
    if !eligible(text) {
        return None;
    }
    let mut p = Fast {
        text,
        b: text.as_bytes(),
        i: 0,
        feats,
    };
    p.block(true, 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast(t: &str) -> Option<Value> {
        try_parse_flat(t, FeatureSet::for_edition(crate::Edition::V1))
    }

    fn same(t: &str) {
        let f = match fast(t) {
            Some(v) => v,
            None => panic!("fast path should accept: {t:?}"),
        };
        let slow = crate::api::parse(t).unwrap_or_else(|e| panic!("slow path failed on {t:?}: {e}"));
        assert_eq!(f, slow, "mismatch on {t:?}");
    }

    #[test]
    fn flat_scalars_match_slow_path() {
        for t in [
            "a: 1\nb: two\n",
            "a: 1\n\nb: true\nc: null\nd: 1.5\n",
            "a: 1\nb: 2\nc: 3\n",
            "  a: 1  \n  b: 2  \n",
            "a: 1",
            "a: 1\r\nb: 2\r\n",
            "k1: v1\nk2: 42\nk3: -3.5\nk4: false\n",
        ] {
            same(t);
        }
    }

    #[test]
    fn anonymous_blocks_match_slow_path() {
        for t in [
            "b0 { x: 1 y: s }\n",
            "b0 { x: 1 }\nb1 { y: 2 }\n",
            "outer { inner { x: 1 } y: 2 }\n",
            "a: 1\nb { }\nc: 2\n",
            "top { a: 1 }\n",
            "b { x: 1 y: 2 }\n",
        ] {
            same(t);
        }
    }

    #[test]
    fn quoted_strings_match_slow_path() {
        for t in [
            "a: \"hello world\"\n",
            "a: \"with: colon and { brace\"\n",
            "a: \"esc \\n \\t \\\" \\\\ done\"\n",
            "a: \"unicode \\u{4F60}\\u597D\"\n",
            "a: \"\"\n",
            "\"key with space\": 1\n",
            "blk { s: \"x y\" }\n",
            "a: \"多字节 \\u{1F600} 串\"\n",
        ] {
            same(t);
        }
    }

    #[test]
    fn structural_docs_fall_back() {
        for t in [
            "a: [1 2]\n",              // 数组未接
            "@version v1\na: 1\n",     // 指令
            "a: $env.X\n",             // 环境变量
            "a: 1\na: 2\n",            // 重复键
            "Wea prod { x: 1 }\n",     // 带名裸块（会注入 __type/__name）
            "a: 1, b: 2\n",            // 逗号
            "a: 1#x\n",                // 行内注释
            "b0 { x: 1\n",             // 未闭合块
            "a: \"unclosed\n",         // 未闭合串
            "a: \"bad \\q\"\n",        // 非法转义（由通用路径报 E-LEX-004）
        ] {
            assert!(fast(t).is_none(), "should fall back: {t:?}");
        }
    }

    /// 语料对拍：仓库里所有 `.sml` 只要被快速路径接管，就必须与通用路径**完全相等**。
    ///
    /// 这是本模块的**长期钉子** —— 将来给 `fast.rs` 加能力（数组）时，
    /// 只要有一条语料对不上，这个测试立刻红。
    #[test]
    fn corpus_equivalence() {
        let mut checked = 0usize;
        let mut accepted = 0usize;
        // 注意 cwd：单测的工作目录是 crate 根（`rust/sml-parse`）。
        for dir in [
            "../../examples",
            "../../tests/conformance/cases",
            "../tests/fixtures",
            "../../rust/tests",
        ] {
            let Ok(rd) = std::fs::read_dir(dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.extension().and_then(|s| s.to_str()) != Some("sml") {
                    continue;
                }
                let Ok(text) = std::fs::read_to_string(&p) else { continue };
                checked += 1;
                if let Some(fast_out) =
                    try_parse_flat(&text, FeatureSet::for_edition(crate::Edition::V1))
                {
                    accepted += 1;
                    match crate::api::parse(&text) {
                        Ok(v) => assert_eq!(fast_out, v, "fast/slow mismatch on {}", p.display()),
                        Err(e) => {
                            panic!("fast accepted but slow failed on {}: {e}", p.display())
                        }
                    }
                }
            }
        }
        assert!(checked > 0, "no corpus files found");
        println!("corpus: checked {checked}, accepted by fast path {accepted}");
    }
}
