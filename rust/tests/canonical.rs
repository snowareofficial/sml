// SPDX-License-Identifier: MulanPSL-2.0
//! Acceptance tests for canonical (`.ambr` T0) — rules in `docs/amberice/spec/format.md` §18 / §19.
//! canonical（`.ambr` 的 T0）验收用例。
//!
//! 判据看**值自身**：`parse()` 产出 `Map::Unordered`，`canonical_text` / `canonical_bytes`
//! 返回 `CanonicalError::KeyOrderUnavailable`（C2，绝不退化成「按排序写」）；
//! `parse_ordered()` / `Map::ordered()` 产出 `Map::Ordered`，可签；标量不受键序约束。
//! 两档构建（`default` 与 `--features preserve_order`）跑同一文件、结论相同 ——
//! `preserve_order` 是空 feature，不改变键序判定。
//! ｜ Key order is a property of the value, not of the build; both feature profiles run
//!    this file with the same expectations.
//!
//! The **expected bytes** do not live here but in `tests/vectors/amberice-canonical.v1.json`
//! (guarded by `amberice vectors --check`) — that is the contract JS/Lua/C/C++ align to.
//! 跨端字节的**期望值**不在本文件，而在 `tests/vectors/amberice-canonical.v1.json`。

use sml::{parse, CanonicalError, Value};

// ---------------------------------------------------------------------------
// C2（方案 2 / B 后）：判据看**对象自身是不是显式有序**，不看编译开关
// ---------------------------------------------------------------------------

#[test]
fn c2_unordered_object_must_refuse_not_sort() {
    // `parse()` 产出 `Map::Unordered`，故不能签；两档构建结论相同 —— 测的是数据，不是 feature。
    let v = parse("z: 1\na: 2\n").expect("parse failed");
    assert_eq!(v.canonical_text().err(), Some(CanonicalError::KeyOrderUnavailable));
    assert_eq!(v.canonical_bytes().err(), Some(CanonicalError::KeyOrderUnavailable));

    // 标量没有对象 ⇒ 没有键序问题 ⇒ **可以签**。旧设计「连标量也一律拒」是
    // 「正确性靠 feature」的副作用：把「这台机器没开开关」误当成数据问题。
    // 现在判定看值本身，标量在任何构建档都稳定可用。
    assert_eq!(Value::Int(1).canonical_text().unwrap(), "1\n");

    // 文案必须给出**可执行的下一步**，且不能误导成「开 feature 就能解决」。
    let msg = CanonicalError::KeyOrderUnavailable.to_string();
    assert!(
        msg.contains("Map::ordered") || msg.contains("parse_ordered"),
        "文案应指路显式有序入口：{msg}"
    );
    assert_eq!(CanonicalError::KeyOrderUnavailable.kind(), "key-order-unavailable");
}

// ---------------------------------------------------------------------------
// 内容判据
// ---------------------------------------------------------------------------

mod ordered {
    use super::*;
    use sml::Map;

    /// 保序内容判据统一入口：`parse_ordered()` 显式要源序（默认 `sml::parse_ordered()` 是字节序）。
    fn text(src: &str) -> String {
        sml::parse_ordered(src)
            .expect("parse failed")
            .canonical_text()
            .expect("canonical_text failed")
    }

    /// C2: key order = source order (**not** byte order — `a` must not jump before `z`).
    /// C2：键序 = 源序（**不是**字节序 —— `a` 不许跑到 `z` 前面）。
    #[test]
    fn c2_key_order_is_source_order() {
        assert_eq!(text("z: 1\na: 2\nm: 3\n"), "z: 1\na: 2\nm: 3\n");
        // 与排序后的结果必须不同，否则这条用例是恒真的
        assert_ne!(text("z: 1\na: 2\n"), text("a: 2\nz: 1\n"));
    }

    /// C1/C3: UTF-8 without BOM, `\n` line endings, 2-space indent, no trailing blanks,
    /// exactly one final `\n`.
    /// C1/C3：UTF-8 无 BOM、行尾 `\n`、2 空格缩进、无行尾空白、文件以单个 `\n` 结尾。
    #[test]
    fn c1_c3_layout_is_fixed() {
        let t = text("config {\n  port: 8080\n  hosts [a b]\n}\n");
        assert!(t.ends_with('\n') && !t.ends_with("\n\n"), "尾部应只有一个换行：{t:?}");
        assert!(!t.contains('\r'), "不许出现 CR：{t:?}");
        assert!(!t.contains('\t'), "不许出现制表符：{t:?}");
        assert!(t.lines().all(|l| l == l.trim_end()), "不许出现行尾空白：{t:?}");
        assert!(t.contains("\n  "), "嵌套缩进应为 2 空格：{t:?}");
    }

    /// C4/C5/C6: literal shapes of int / float / bool / null.
    /// C4/C5/C6：整数、浮点、布尔、空值的字面形态。
    #[test]
    fn c4_c5_c6_scalars() {
        assert_eq!(Value::Int(0).canonical_text().unwrap(), "0\n");
        assert_eq!(Value::Int(-42).canonical_text().unwrap(), "-42\n");
        assert_eq!(Value::float(1.0).canonical_text().unwrap(), "1.0\n");
        assert_eq!(Value::float(-0.0).canonical_text().unwrap(), "-0.0\n");
        assert_eq!(Value::float(2.5).canonical_text().unwrap(), "2.5\n");
        assert_eq!(Value::Bool(true).canonical_text().unwrap(), "true\n");
        assert_eq!(Value::Null.canonical_text().unwrap(), "null\n");
    }

    /// C5/C12: `raw` (the spelling seen by the parser) **never enters** canonical.
    /// C5/C12：`raw`（解析期的书写痕迹）**不进** canonical。
    ///
    /// 若让它进：`x: 1e10` 从文本读入与从 `.ambr` 读入会得到两串不同文本，
    /// 「同一逻辑数据同字节」当场不成立 —— 签名也就跟着分裂。
    #[test]
    fn c5_raw_literal_does_not_leak_into_canonical() {
        let a = sml::parse_ordered("x: 1e10\n").expect("parse failed");
        let b = sml::parse_ordered("x: 10000000000.0\n").expect("parse failed");
        assert_eq!(a.canonical_text().unwrap(), b.canonical_text().unwrap());
        assert_eq!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
        assert!(a.canonical_text().unwrap().contains("10000000000.0"));
    }

    /// C7: bare words / quoting / escaping, and the text must **parse back** (P5-d).
    /// C7：字符串裸词/引号/转义，且**能解析回去**（P5-d：canonical 必须是合法 SML）。
    ///
    /// 约束来自 parser 而非 canonical：**顶层必须是容器**
    /// （`E-PARSE-008`「顶层须为容器，单独的标量无法往返」）。故这里把每个字符串放进
    /// 一个单键对象里验往返 —— 「字符串怎么写」与「顶层能不能是标量」是两件事，
    /// 后者已记进 spec §18.2（实现澄清）。
    #[test]
    fn c7_strings_are_legal_sml() {
        let cases = [
            Value::Str("hello".into()),
            Value::Str("007".into()),
            Value::Str("a b".into()),
            Value::Str("he said \"hi\"".into()),
            Value::Str("line1\nline2\ttab".into()),
            Value::Str("u\0v".into()),
            Value::Str("\u{1}".into()),
            Value::Str("中文".into()),
            Value::Str(String::new()),
        ];
        for v in cases {
            let doc = {
                let mut m = Map::ordered();
                m.insert("s".to_string(), v.clone());
                Value::Object(m)
            };
            let t = doc.canonical_text().unwrap();
            let back =
                sml::parse_ordered(&t).unwrap_or_else(|e| panic!("canonical 文本不是合法 SML：{t:?} {e:?}"));
            assert_eq!(back, doc, "往返不一致：{t:?}");

            // 标量自身的 canonical 文本也照常产出（签名字节唯一），只是**不是**可回读的文档
            let solo = v.canonical_text().unwrap();
            assert!(solo.ends_with('\n'));
            assert!(sml::parse_ordered(&solo).is_err(), "顶层标量本就不该被接受：{solo:?}");
        }
    }

    /// C8/C9: one-line arrays with space separators; empty containers spelled `{ }` / `[ ]`.
    /// C8/C9：数组单行、空格分隔；空容器写 `{ }` / `[ ]`。
    #[test]
    fn c8_c9_containers() {
        assert_eq!(text("a: [1 2 3]\n"), "a: [ 1 2 3 ]\n");
        assert_eq!(text("a { }\n"), "a: { }\n");
        assert_eq!(text("a [ ]\n"), "a: [ ]\n");
        // 元素含容器 ⇒ 分行，且收尾括号回到键的缩进
        assert_eq!(
            text("a [[1]]\n"),
            "a: [\n  [ 1 ]\n]\n"
        );
    }

    /// C10: `__type` → `__name` → `__args` → `__featags` always first (source order ignored).
    /// C10：四个元数据键固定最前（不跟源序）。
    #[test]
    fn c10_metadata_keys_come_first() {
        // 源序把 `__type` 写在最后，输出必须提到最前
        let t = text("name: wea\n__type: Rule\n");
        assert_eq!(t, "__type: Rule\nname: wea\n");

        let t = text("b: 1\n__featags: [ ]\n__name: n\n__type: T\n__args: [ ]\n");
        assert_eq!(t, "__type: T\n__name: n\n__args: [ ]\n__featags: [ ]\nb: 1\n");
    }

    /// Criterion: `dump → load → dump` is **byte-identical** (T1's acceptance, byte layer pinned here).
    /// 判据：`dump → load → dump` **逐字节相同**（§17 T1 的验收，T0 先钉住字节层）。
    #[test]
    fn dump_load_dump_is_byte_identical() {
        let src = "z: 1\na: [1 2 3]\nnested { p: 2.5 q: { } }\ns: \"a b\"\nneg: -7\n";
        let v = sml::parse_ordered(src).expect("parse failed");
        let b1 = v.canonical_bytes().unwrap();
        let loaded = Value::from_canonical_bytes(&b1).expect("load failed");
        assert_eq!(loaded, v, "值必须相等");
        assert!(loaded.eq_ordered(&v), "键序必须原样回来");
        let b2 = loaded.canonical_bytes().unwrap();
        assert_eq!(b1, b2, "dump→load→dump 必须逐字节相同");

        // 文本侧同样稳定
        let t1 = v.canonical_text().unwrap();
        assert_eq!(loaded.canonical_text().unwrap(), t1);
    }

    /// Criterion: `Int(1)` and `Float(1.0)` stay distinguishable (different tag, different text).
    /// 判据：`Int(1)` 与 `Float(1.0)` 可区分（tag 不同、文本不同）。
    #[test]
    fn int_and_float_are_distinguishable() {
        let i = Value::Int(1);
        let f = Value::float(1.0);
        assert_ne!(i.canonical_bytes().unwrap(), f.canonical_bytes().unwrap());
        assert_ne!(i.canonical_text().unwrap(), f.canonical_text().unwrap());

        // 加载回来也各归各位
        let li = Value::from_canonical_bytes(&i.canonical_bytes().unwrap()).unwrap();
        let lf = Value::from_canonical_bytes(&f.canonical_bytes().unwrap()).unwrap();
        assert!(matches!(li, Value::Int(1)));
        assert!(matches!(lf, Value::Float(..)));
    }

    /// Criterion: `__type` / `__featags` round-trip verbatim (bytes keep them ordinary,
    /// text hoists them).
    /// 判据：`__type` / `__featags` 原样往返（字节侧不特化它们，文本侧前置）。
    #[test]
    fn metadata_roundtrips_verbatim() {
        let mut featag = Map::ordered();
        featag.insert("name".to_string(), Value::Str("obey".into()));
        featag.insert(
            "args".to_string(),
            Value::Array(vec![Value::Int(1), Value::Int(2)]),
        );
        let mut m = Map::ordered();
        m.insert("name".to_string(), Value::Str("wea".into()));
        m.insert(
            "__featags".to_string(),
            Value::Array(vec![Value::Object(featag)]),
        );
        m.insert("__type".to_string(), Value::Str("Rule".into()));
        let v = Value::Object(m);

        let b = v.canonical_bytes().unwrap();
        let loaded = Value::from_canonical_bytes(&b).unwrap();
        assert!(loaded.eq_ordered(&v), "__featags 必须原样回来（含键序）");
        assert_eq!(loaded.canonical_bytes().unwrap(), b);

        // 文本侧：`__type` 前置，但内容一字不差；且能解析回去
        let t = v.canonical_text().unwrap();
        assert!(t.starts_with("__type: Rule\n"), "{t}");
        assert!(t.contains("__featags: ["), "{t}");
        assert_eq!(sml::parse_ordered(&t).expect("canonical 文本应可解析"), v);
    }

    /// C12: same logical data ⇒ same bytes (whatever the origin or the layout used).
    /// C12：同一逻辑数据 ⇒ 同一串字节（不同来源、不同书写排版都一样）。
    #[test]
    fn c12_same_logical_data_same_bytes() {
        let a = sml::parse_ordered("a: 1\nb: [1 2]\n").expect("parse failed");
        let b = sml::parse_ordered("a: 1 b: [1, 2]").expect("parse failed");
        assert_eq!(a, b);
        assert_eq!(a.canonical_bytes().unwrap(), b.canonical_bytes().unwrap());
        assert_eq!(a.canonical_text().unwrap(), b.canonical_text().unwrap());
    }

    /// C13: non-finite floats **error out**, never faked into a literal or a string.
    /// C13：非有限浮点**报错**，不许伪造成字面量/字符串。
    #[test]
    fn c13_non_finite_float_errors() {
        for f in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
            let v = Value::float(f);
            assert_eq!(v.canonical_text().err(), Some(CanonicalError::NonFiniteFloat));
            assert_eq!(v.canonical_bytes().err(), Some(CanonicalError::NonFiniteFloat));
            assert_eq!(CanonicalError::NonFiniteFloat.kind(), "non-finite-float");
        }
    }

    /// C11: `Value` holds no comments, so canonical emits none (not a single `/* */`).
    /// C11：`Value` 里没有注释，canonical 也不产出注释。
    #[test]
    fn c11_no_comments() {
        let t = text("-- 注释\n// 另一种注释\na: 1 # 行尾注释\n");
        assert!(!t.contains("注释"), "{t}");
        assert!(t.contains("a: 1"), "{t}");
    }

    /// An empty document (empty object) must be `{ }`, not an empty string — otherwise
    /// “ends with exactly one `\n`” has nothing to attach to.
    /// 空文档（空对象）必须是 `{ }` 而不是空串。
    #[test]
    fn empty_document() {
        let v = sml::parse_ordered("").expect("空文档应解析为空对象");
        assert_eq!(v.canonical_text().unwrap(), "{ }\n");
        assert_eq!(Value::from_canonical_bytes(&v.canonical_bytes().unwrap()).unwrap(), v);
    }
}

// ---------------------------------------------------------------------------
// 顶层标量：默认拒绝（W16），显式 opt-in 后精确往返（spec §23.3）
// Top-level scalars: rejected by default, exact round-trip behind an opt-in.
// ---------------------------------------------------------------------------
mod top_level_scalar {
    use sml::{parse, parse_with, ParseOptions, Value};

    #[test]
    fn rejected_by_default() {
        for src in ["42", "-7", "1.5", "true", "null", "\"hi\""] {
            let e = parse(src).expect_err(src);
            assert!(e.to_string().contains("E-PARSE-008"), "{src}: {e}");
        }
    }

    #[test]
    fn opt_in_round_trips_exactly() {
        let cases: [(&str, Value); 6] = [
            ("42", Value::Int(42)),
            ("-7", Value::Int(-7)),
            ("1.5", Value::float(1.5)),
            ("true", Value::Bool(true)),
            ("null", Value::Null),
            ("\"hi\"", Value::Str("hi".into())),
        ];
        for (src, want) in cases {
            let got = parse_with(src, ParseOptions::new().allow_top_scalar())
                .unwrap_or_else(|e| panic!("{src}: {e}"))
                .value;
            assert_eq!(got, want, "{src}");

            // 与发射器对称：canonical_text 再解析回来必须还是同一个值。
            // Symmetric with the emitter: canonical_text must parse back to the same value.
            let text = got.canonical_text().expect("canonical_text");
            let back = parse_with(&text, ParseOptions::new().allow_top_scalar())
                .unwrap_or_else(|e| panic!("{text:?}: {e}"))
                .value;
            assert_eq!(back, want, "{text:?}");
        }
    }

    #[test]
    fn ambiguous_forms_still_rejected() {
        // 有歧义的形态照旧报错：把标量当块类型名会改写数据形状（T9 / W16）。
        // Ambiguous forms still error: reinterpreting a scalar as a block type rewrites the shape.
        for src in ["42 a { b: 1 }", "42\nb: 1", "42\n[1,2]"] {
            let e = parse_with(src, ParseOptions::new().allow_top_scalar()).expect_err(src);
            assert!(e.to_string().contains("E-PARSE-008"), "{src}: {e}");
        }
    }
}
