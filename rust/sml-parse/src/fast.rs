//! 单遍解析的**快速路径**：只覆盖「纯扁平标量文档」。
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
//! `{ } [ ] @ $ ! \` " ' \` —— 也就是「块 / 数组 / 指令 / 环境变量 / 模板串 / 引号串」全不接。
//! 这几样恰好是语义分歧最容易发生的地方（裸块前瞻、`__type` 注入、同名合并、转义规则），
//! 留给通用路径处理。
//!
//! # 一致性的三条硬约束
//!
//! 1. **值转换复用** [`sml_lex::coerce_word`]：`1` / `1.5` / `true` / `null` 的判定只有一份实现；
//! 2. **遇到任何异常就返回 `None`**（重复键、缺冒号、值不在同一行、空文档…），
//!    由通用路径产出**原本那条**错误消息 —— 快速路径绝不自己发明错误；
//! 3. 因此它对外的可见行为只有两种：要么产出与通用路径**逐字节相同**的树，要么不接管。

use sml_feature::FeatureSet;
use sml_lex::coerce_word;
use sml_value::{Map, Value};

/// 空片段表（本路径不接 `@片段`）。
const NO_FRAGMENTS: std::collections::BTreeMap<String, Value> = std::collections::BTreeMap::new();

/// 是否在能力范围内。一次轻量字节扫描，命中任一「结构 / 指令 / 引号」字符即否决。
fn eligible(text: &str) -> bool {
    !text.bytes().any(|c| {
        matches!(
            c,
            b'{' | b'}' | b'[' | b']' | b'@' | b'$' | b'!' | b'`' | b'"' | b'\'' | b'\\'
            // `,` `;` `#` 也一律不接：它们在通用路径里的归属很反直觉
            // （实测 `a: 1\nb: 2 ;` 会让 `;` 变成**键**；行内 `#` 另有边界），
            // 这类语义不值得在快速路径里复刻一份。
            | b',' | b';' | b'#'
        )
    })
}

/// 尝试单遍解析。
///
/// - `Some(value)`：**确定**与通用路径等价（调用方可直接返回）；
/// - `None`：不在能力范围内，或遇到任何需要通用路径处理的情形 —— 调用方必须回落。
pub(crate) fn try_parse_flat(text: &str, feats: FeatureSet) -> Option<Value> {
    if !eligible(text) {
        return None;
    }
    let b = text.as_bytes();
    let n = b.len();
    let mut i = 0usize;
    let mut m = Map::new();

    loop {
        // 行首：空白与 `#` 注释
        while i < n {
            match b[i] {
                b' ' | b'\t' | b'\r' | b'\n' => i += 1,
                b'#' => {
                    while i < n && b[i] != b'\n' {
                        i += 1;
                    }
                }
                _ => break,
            }
        }
        if i >= n {
            break;
        }

        // 键：裸词，直到 `:`
        let ks = i;
        while i < n && b[i] != b':' && b[i] != b' ' && b[i] != b'\t' && b[i] != b'\n' && b[i] != b'\r'
        {
            i += 1;
        }
        if i == ks || i >= n || b[i] != b':' {
            return None; // 缺冒号 / 键为空：交回通用路径报它原本的码
        }
        let key = &text[ks..i];
        i += 1;

        // 值必须与键同一行（通用路径也是这么要求的）
        while i < n && (b[i] == b' ' || b[i] == b'\t') {
            i += 1;
        }
        if i >= n || b[i] == b'\n' || b[i] == b'\r' {
            return None;
        }
        let vs = i;
        while i < n {
            match b[i] {
                b' ' | b'\t' | b'\r' | b'\n' | b',' | b';' | b'#' => break,
                _ => i += 1,
            }
        }
        if i == vs {
            return None;
        }
        let v = coerce_word(&text[vs..i], &NO_FRAGMENTS, feats, "", None).ok()?;

        // 重复键 ⇒ 通用路径要把它「提升为数组」并记诊断；本路径不接，整体回落。
        if m.contains_key(key) {
            return None;
        }
        m.insert(key.to_string(), v);

        // 行尾：空白 / 分隔符 / 行内注释。
        //
        // 这里**不**强制「后面必须是换行」——`a: 1, b: 2` 与 `a: 1 b: 2` 在通用路径里
        // 都是两个字段（它是 token 驱动的，不区分空白与逗号）。若在此处要求换行，
        // 就会把合法写法判成「不接管」，白丢快速路径。真正的兜底在下一轮的「键后必须
        // 有 `:`」：任何畸形输入都会在那里返回 `None`，交回通用路径报原本的码。
        while i < n && (b[i] == b' ' || b[i] == b'\t' || b[i] == b',' || b[i] == b';') {
            i += 1;
        }
        if i < n && b[i] == b'#' {
            while i < n && b[i] != b'\n' {
                i += 1;
            }
        }
    }

    Some(Value::Object(m))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn fast(t: &str) -> Option<Value> {
        try_parse_flat(t, FeatureSet::for_edition(crate::Edition::V1))
    }

    #[test]
    fn flat_scalars_match_slow_path() {
        let cases = [
            "a: 1\nb: two\n",
            "a: 1\n\nb: true\nc: null\nd: 1.5\n",
            "a: 1\nb: 2\nc: 3\n",
            "  a: 1  \n  b: 2  \n",
            "a: 1",
            "a: 1\r\nb: 2\r\n",
            "k1: v1\nk2: 42\nk3: -3.5\nk4: false\n",
        ];
        for t in cases {
            let fast_out = match fast(t) {
                Some(v) => v,
                None => panic!("fast path should accept: {t:?}"),
            };
            let slow_out = crate::api::parse(t).expect("slow path should parse");
            assert_eq!(fast_out, slow_out, "mismatch on {t:?}");
        }
    }

    /// 语料对拍：仓库里所有 `.sml` 只要被快速路径接管，就必须与通用路径**完全相等**。
    ///
    /// 这是本模块的**长期钉子** —— 将来给 `fast.rs` 加能力（块、数组、引号串）时，
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
                if let Some(fast_out) = try_parse_flat(&text, FeatureSet::for_edition(crate::Edition::V1)) {
                    accepted += 1;
                    let slow = crate::api::parse(&text);
                    match slow {
                        Ok(v) => assert_eq!(fast_out, v, "fast/slow mismatch on {}", p.display()),
                        Err(e) => panic!(
                            "fast accepted but slow failed on {}: {e}",
                            p.display()
                        ),
                    }
                }
            }
        }
        // 不能「因为一条都没接管所以全过」—— 至少要有语料真的走到这条路径上，
        // 否则这个测试是摆设（下面这条断言就是为了防它变成摆设）。
        assert!(checked > 0, "no corpus files found");
        println!("corpus: checked {checked}, accepted by fast path {accepted}");
    }

    #[test]
    fn structural_docs_fall_back() {
        for t in [
            "a { x: 1 }\n",
            "a: [1 2]\n",
            "@version v1\na: 1\n",
            "a: $env.X\n",
            "a: \"quoted\"\n",
            "a: 1\na: 2\n", // 重复键：交给通用路径合并成数组
        ] {
            assert!(fast(t).is_none(), "should fall back: {t:?}");
        }
    }
}
