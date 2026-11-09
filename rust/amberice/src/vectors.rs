// SPDX-License-Identifier: MulanPSL-2.0
//! Cross-end test vectors, in two layers (T0 / T7, spec §17 / §22).
//! 跨端测试向量，分两层：
//!
//! - `amberice-canonical.v1.json` — **node encoding** (`STRTAB ‖ VALUE`), pins §19.2 tags;
//! - `amberice-container.v1.json` — **the whole `.ambr` file** (header, table, alignment,
//!   manifest), pins §19.1/§19.6/§19.7/§19.9 and is what the JS / C / C++ / Lua decoders
//!   (T7 / N03) align to: decode `ambr_hex`, compare `canonical_text`, skip unknown segments.
//!
//! ｜ 一份钉节点编码，一份钉整个文件 —— 四端照后者写解码器，不必读 Rust 实现。
//!
//! **Why they exist** / **为什么需要它**: canonical's correctness cannot be proven by Rust
//! comparing against itself — that is circular (the same implementation agreeing with the
//! same implementation is always true). The vectors freeze the **expected values into a
//! file**, so from then on:
//!
//! - a layout tweak or a re-numbered tag turns `--check` red instead of silently changing
//!   bytes; ｜ Rust 改了排版/换 tag 编号 ⇒ `--check` 立刻红；
//! - the JS / Lua / C / C++ implementations (T7) align to this file **without reading Rust
//!   code**; ｜ 四端照这份文件对齐，不需要读 Rust 代码；
//! - signatures (T4) reference it: cross-end verification starts from “both ends sign the
//!   same thing”. ｜ 签名以它为基准。
//!
//! The output is JSON (all five ends have a JSON reader), but **this crate never parses it**:
//! `--check` regenerates the string and compares it line by line with what is on disk — the
//! same trick as `errors/gen_json.py` + `git diff --exit-code`. Hence zero dependencies (no
//! `serde_json`) and no chance of the generator and the checker disagreeing.
//! 输出是 JSON，但**本模块不解析它**：`--check` 重新生成并与磁盘逐行比对。
//!
//! **Do not hand-edit that JSON** — it is produced by `amberice vectors --gen`; edits turn
//! `--check` red. What you actually want to change is the case table below or canonical itself.
//! **别手改那份 JSON**：真正要改的是本文件的用例表或 canonical 实现。

use std::path::{Path, PathBuf};

use sml_value::{CanonicalError, CanonicalValue, Map, Value};

/// Format marker — cross-end scripts use it to decide “is this mine to read?”.
/// 向量文件的格式标识（跨端脚本据它判断「这是不是我能读的东西」）。
pub const FORMAT: &str = "amberice/canonical-vectors";
/// Format version: bumped only when fields are added/removed (new expected values do not count).
/// 格式版本：字段增删才 +1（改期望值不算）。
pub const VERSION: u32 = 1;
/// Default location, relative to the repo root. ｜ 默认落点（相对仓库根）。
pub const DEFAULT_PATH: &str = "tests/vectors/amberice-canonical.v1.json";

/// Container-level vector format marker. ｜ 容器级向量的格式标识。
///
/// 与 canonical 向量分开两件事：那份钉「节点编码」，这份钉「整个 `.ambr` 文件」——
/// 头部 40 B、段表 41 B × n、段体对齐、MANIFEST。四端（JS/C/C++/Lua）照 `ambr_hex`
/// 解出 `canonical_text` 即可，不必读 Rust 实现。
/// ｜ Two layers, two files: node encoding vs. the whole container (header, table, alignment).
pub const CONTAINER_FORMAT: &str = "amberice/container-vectors";
/// Container vector format version (bumped only when fields change).
/// 容器向量格式版本（只有字段增删才 +1）。
pub const CONTAINER_VERSION: u32 = 1;
/// Default location of the container vectors. ｜ 容器向量默认落点。
pub const CONTAINER_PATH: &str = "tests/vectors/amberice-container.v1.json";

/// One vector case. ｜ 一个向量用例。
pub struct Case {
    /// Stable id; cross-end scripts join on it — **renaming it means replacing the case**.
    /// 稳定 id（跨端脚本靠它对齐；**改名等于换用例**）。
    pub id: &'static str,
    /// The equivalent `.sml` source text — for humans; every end should be able to parse it
    /// into the same value. It is **not** the input: `build` is, so that “canonical is wrong”
    /// and “that end's parser is wrong” stay distinguishable.
    /// 等价的 `.sml` 源文本（给人看）；**不是**输入 —— 输入是 `build`，这样能把
    /// 「canonical 错」与「解析器错」分开。
    pub source: &'static str,
    /// Which rule this case pins down. ｜ 这个用例想钉住哪条规则。
    pub note: &'static str,
    /// Build the same value programmatically. ｜ 程序构造同一个值。
    pub build: fn() -> Value,
}

fn s(x: &str) -> Value {
    Value::Str(x.to_string())
}

fn obj(pairs: &[(&str, Value)]) -> Value {
    let mut m = Map::ordered();
    for (k, v) in pairs {
        m.insert((*k).to_string(), v.clone());
    }
    Value::Object(m)
}

/// The case table. **Order is output order** — append new cases at the end (reordering the
/// existing ones turns every diff into noise).
/// 用例表。**顺序即输出顺序**，新增请追加到末尾。
///
/// Coverage (rule ids from §18.2) / 覆盖面：C4 int / C5 float / C6 bool+null / C7 strings and
/// escapes / C8 one-line arrays / C9 empty containers / C10 metadata first / C12 determinism /
/// §19.2 tags and STRTAB dedup.
pub const CASES: &[Case] = &[
    Case {
        id: "scalar-null",
        source: "null",
        note: "C6 空值",
        build: || Value::Null,
    },
    Case {
        id: "scalar-bools",
        source: "[true false]",
        note: "C6 布尔；tag 1/2 区分真假、无 payload",
        build: || Value::Array(vec![Value::Bool(true), Value::Bool(false)]),
    },
    Case {
        id: "scalar-int",
        source: "1",
        note: "C4 整数；zigzag 后 1 B",
        build: || Value::Int(1),
    },
    Case {
        id: "scalar-int-negative",
        source: "-1",
        note: "C4 负数（zigzag 与无符号 LEB128 的区别在这里暴露）",
        build: || Value::Int(-1),
    },
    Case {
        id: "scalar-int-i64-min",
        source: "-9223372036854775808",
        note: "C4 i64 下界；zigzag 上界 10 B 变长整数",
        build: || Value::Int(i64::MIN),
    },
    Case {
        id: "float-integral",
        source: "1.0",
        note: "C5 整数值浮点必须带 `.0`（否则 load 回来变 Int）",
        build: || Value::float(1.0),
    },
    Case {
        id: "float-negative-zero",
        source: "-0.0",
        note: "C5 `-0.0` 保留符号（不归一成 `0.0`）",
        build: || Value::float(-0.0),
    },
    Case {
        id: "float-fraction",
        source: "2.5",
        note: "C5 普通小数",
        build: || Value::float(2.5),
    },
    Case {
        id: "float-scientific",
        source: "1e10",
        note: "C5 raw 不进 canonical：`1e10` 与 `10000000000.0` 是同一串字节",
        build: || Value::float(1e10),
    },
    Case {
        id: "str-bare",
        source: "hello",
        note: "C7 能裸就裸",
        build: || s("hello"),
    },
    Case {
        id: "str-cjk-bare",
        source: "中文键值",
        note: "C7 汉字属 alphabetic，裸词直出",
        build: || s("中文键值"),
    },
    Case {
        id: "str-numeric-looking",
        source: "\"007\"",
        note: "C7 长得像数字的字符串必须加引号（否则回读成 Int）",
        build: || s("007"),
    },
    Case {
        id: "str-escapes",
        source: "\"he said \\\"hi\\\"\\n\\t\\\\\\u0001\"",
        note: "C7 必须转义 `\\` `\"` 与 C0 控制字符（`\\n` `\\t` 走短形式）",
        build: || s("he said \"hi\"\n\t\\\u{1}"),
    },
    Case {
        id: "empty-object",
        source: "{ }",
        note: "C9 空对象写 `{ }`（不是 `{}`）",
        build: || obj(&[]),
    },
    Case {
        id: "empty-array",
        source: "[ ]",
        note: "C9 空数组写 `[ ]`（不是 `[]`）",
        build: || Value::Array(vec![]),
    },
    Case {
        id: "flat-array",
        source: "[1 2 3]",
        note: "C8 标量元素单行、空格分隔（不用逗号）",
        build: || Value::Array(vec![Value::Int(1), Value::Int(2), Value::Int(3)]),
    },
    Case {
        id: "object-source-order",
        source: "z: 1\na: 2\nm: 3",
        note: "C2 键序 = 源序（**不是**字节序：a 不能在 z 前面）",
        build: || obj(&[("z", Value::Int(1)), ("a", Value::Int(2)), ("m", Value::Int(3))]),
    },
    Case {
        id: "typed-block",
        source: "Rule ssh { action: allow, port: 22 }",
        note: "C10 `__type`/`__name` 固定最前；§19.2 flags 置「含 __type」位",
        build: || {
            obj(&[
                ("action", s("allow")),
                ("port", Value::Int(22)),
                ("__type", s("Rule")),
                ("__name", s("ssh")),
            ])
        },
    },
    Case {
        id: "featags-nested",
        source: "wea !critical { name: wea }",
        note: "C10 `__featags` 提到最前 + C8 含容器则分行",
        build: || {
            obj(&[
                ("name", s("wea")),
                (
                    "__featags",
                    Value::Array(vec![
                        obj(&[("name", s("critical"))]),
                        obj(&[("name", s("obey")), ("args", Value::Array(vec![Value::Int(1), Value::Int(2)]))]),
                    ]),
                ),
            ])
        },
    },
    Case {
        id: "nested-containers",
        source: "config {\n  port: 8080\n  hosts [ a b ]\n}\ntags [x]",
        note: "C8 多行容器：`键: {` 同行，收尾括号回到键的缩进",
        build: || {
            obj(&[
                (
                    "config",
                    obj(&[
                        ("port", Value::Int(8080)),
                        ("hosts", Value::Array(vec![s("a"), s("b")])),
                    ]),
                ),
                ("tags", Value::Array(vec![s("x")])),
            ])
        },
    },
    Case {
        id: "strtab-dedup",
        source: "k: k\nother: k",
        note: "§19.2 STRTAB 全局去重：键与值同串只占一格",
        build: || obj(&[("k", s("k")), ("other", s("k"))]),
    },
];

/// Render the whole vectors file (deterministic: same code ⇒ same bytes).
/// 渲染整份向量文件（确定性：同代码 ⇒ 同字节）。
pub fn render() -> Result<String, CanonicalError> {
    let mut out = String::with_capacity(4096);
    out.push_str("{\n");
    out.push_str(&format!("  \"format\": \"{FORMAT}\",\n"));
    out.push_str(&format!("  \"version\": {VERSION},\n"));
    out.push_str("  \"spec\": \"docs/amberice/spec/format.md §18 (C1-C13) + §19\",\n");
    out.push_str(
        "  \"note\": \"由 `amberice vectors --gen` 生成，勿手改；`--check` 会重新生成并逐行比对。\
任一实现按 source 造出同一个值后，canonical_text 必须逐字符相同、bytes_hex 必须逐字节相同。\",\n",
    );
    out.push_str("  \"cases\": [\n");
    for (i, case) in CASES.iter().enumerate() {
        let v = (case.build)();
        let text = v.canonical_text()?;
        let bytes = v.canonical_bytes()?;
        out.push_str("    {\n");
        out.push_str(&format!("      \"id\": \"{}\",\n", case.id));
        out.push_str("      \"source\": ");
        push_json_str(&mut out, case.source);
        out.push_str(",\n      \"note\": ");
        push_json_str(&mut out, case.note);
        out.push_str(",\n      \"canonical_text\": ");
        push_json_str(&mut out, &text);
        out.push_str(&format!(",\n      \"bytes_len\": {},\n", bytes.len()));
        out.push_str("      \"bytes_hex\": ");
        push_json_str(&mut out, &hex(&bytes));
        out.push_str("\n    }");
        if i + 1 != CASES.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}\n");
    Ok(out)
}

/// Self-check: **touches no disk**, verifies properties of the implementation itself (the
/// other half of the criteria, which the vectors file cannot express).
/// 自检：**不碰磁盘**，只验证实现自身的性质。
///
/// 1. every case `bytes → load → bytes` is byte-identical ｜ 每例往返逐字节相同；
/// 2. every case `text(load(bytes)) == text(orig)` — otherwise signature and bytes disagree
///    ｜ 否则签名与字节会各说各话；
/// 3. ids are unique (cross-end scripts join on them) ｜ id 不重复。
pub fn verify_self() -> Result<(), String> {
    for (i, case) in CASES.iter().enumerate() {
        if CASES[..i].iter().any(|c| c.id == case.id) {
            return Err(format!("用例 id 重复：{}", case.id));
        }
        let v = (case.build)();
        let text = v.canonical_text().map_err(|e| format!("{e}"))?;
        let bytes = v.canonical_bytes().map_err(|e| format!("{e}"))?;
        let loaded =
            Value::from_canonical_bytes(&bytes).map_err(|e| format!("[{}] 解码失败：{e}", case.id))?;
        let again = loaded
            .canonical_bytes()
            .map_err(|e| format!("[{}] 重编码失败：{e}", case.id))?;
        if again != bytes {
            return Err(format!("[{}] `bytes → load → bytes` 不一致", case.id));
        }
        let text2 = loaded
            .canonical_text()
            .map_err(|e| format!("[{}] 重编码文本失败：{e}", case.id))?;
        if text2 != text {
            return Err(format!(
                "[{}] 文本与字节说的不是同一件事：\n  原文本: {text:?}\n  往返后: {text2:?}",
                case.id
            ));
        }
    }
    Ok(())
}

/// Render the **container** vectors file (whole `.ambr` bytes per case).
/// 渲染**容器**向量文件（每例给整份 `.ambr` 字节）。
///
/// Why a separate file rather than more fields in the canonical one: the two layers change for
/// different reasons (node layout vs. header/table/alignment), so a change in one must not
/// rewrite the other's diff. ｜ 分层分文件：节点布局与容器布局各自演进，互不搅 diff。
///
/// The pass criterion for a decoder is **the decoded tree**, expressed as `canonical_text`;
/// write-back is deliberately not required (a read-only port can still be conformant).
/// ｜ 判据是解出的值树（用 `canonical_text` 表达），**不要求**写回。
pub fn render_container() -> Result<String, String> {
    let mut out = String::with_capacity(8192);
    out.push_str("{\n");
    out.push_str(&format!("  \"format\": \"{CONTAINER_FORMAT}\",\n"));
    out.push_str(&format!("  \"version\": {CONTAINER_VERSION},\n"));
    out.push_str("  \"spec\": \"docs/amberice/spec/format.md §19.1 / §19.3 / §19.6 / §19.7 / §19.9\",\n");
    out.push_str(
        "  \"note\": \"由 `amberice vectors --gen` 生成，勿手改；`--check` 重新生成并逐行比对。\
读端拿 ambr_hex 解出值树后，canonical_text 必须逐字符相同；未知段一律跳过（表项自带 len），\
不需要理解 TYPED / DIR 的语义。元数据键（__type / __name / __args / __featags）在**字节侧保持源序**、\
在 canonical_text 里按 C10 **前置**（§19.5 L5）—— 对拍前两边都按 C10 归一化，否则 typed block / featag 会只差键位。\",\n",
    );
    out.push_str("  \"cases\": [\n");
    for (i, case) in CASES.iter().enumerate() {
        let v = (case.build)();
        let cv = CanonicalValue::try_from(&v).map_err(|e| format!("[{}] 值不可有序：{e}", case.id))?;
        let bytes = crate::dump(&cv).map_err(|e| format!("[{}] dump 失败：{e}", case.id))?;
        let text = v.canonical_text().map_err(|e| format!("[{}] {e}", case.id))?;
        out.push_str("    {\n");
        out.push_str(&format!("      \"id\": \"{}\",\n", case.id));
        out.push_str("      \"source\": ");
        push_json_str(&mut out, case.source);
        out.push_str(",\n      \"note\": ");
        push_json_str(&mut out, case.note);
        out.push_str(",\n      \"canonical_text\": ");
        push_json_str(&mut out, &text);
        out.push_str(&format!(",\n      \"ambr_len\": {},\n", bytes.len()));
        out.push_str("      \"ambr_hex\": ");
        push_json_str(&mut out, &hex(&bytes));
        out.push_str("\n    }");
        if i + 1 != CASES.len() {
            out.push(',');
        }
        out.push('\n');
    }
    out.push_str("  ]\n}\n");
    Ok(out)
}

/// Container self-check: determinism + `dump → load → canonical_text` for every case.
/// 容器自检：确定性 + 每例「dump → load → canonical_text」往返。
///
/// Determinism is a hard requirement here: the fixtures are raw bytes, so a timestamp or a
/// hash-map iteration order leaking into `dump` would make `--check` flaky rather than wrong.
/// ｜ 确定性是硬要求：fixture 是原始字节，一旦 `dump` 带进时间戳或哈希序，`--check` 会抖动。
pub fn verify_container() -> Result<(), String> {
    for case in CASES {
        let v = (case.build)();
        let cv = CanonicalValue::try_from(&v).map_err(|e| format!("[{}] 值不可有序：{e}", case.id))?;
        let a = crate::dump(&cv).map_err(|e| format!("[{}] dump 失败：{e}", case.id))?;
        let b = crate::dump(&cv).map_err(|e| format!("[{}] dump 失败：{e}", case.id))?;
        if a != b {
            return Err(format!(
                "[{}] `dump` 两次产出不同字节（{} B vs {} B）—— 容器层混进了非确定性",
                case.id,
                a.len(),
                b.len()
            ));
        }
        let back = crate::load(&a).map_err(|e| format!("[{}] 解不回来：{e}", case.id))?;
        let want = v.canonical_text().map_err(|e| format!("[{}] {e}", case.id))?;
        let got = back.canonical_text().map_err(|e| format!("[{}] {e}", case.id))?;
        if want != got {
            return Err(format!(
                "[{}] 容器往返后值变了：\n  原: {want:?}\n  回: {got:?}",
                case.id
            ));
        }
    }
    Ok(())
}

/// `--gen`: regenerate and write to disk. ｜ 重新生成并写盘。
pub fn generate(path: &Path) -> Result<String, String> {
    verify_self()?;
    let body = render().map_err(|e| format!("{e}"))?;
    write_bytes(path, &body)?;
    Ok(format!(
        "已生成 {} 例 → {}（{} B）",
        CASES.len(),
        path.display(),
        body.len()
    ))
}

/// `--check`: regenerate and compare line by line with the file (no JSON parsing — see module docs).
/// 重新生成并与磁盘逐行比对（不自解析 JSON，见模块头）。
pub fn check(path: &Path) -> Result<String, String> {
    verify_self()?;
    let expected = render().map_err(|e| format!("{e}"))?;
    let actual = std::fs::read_to_string(path)
        .map_err(|e| format!("读 {} 失败：{e}（先跑 `amberice vectors --gen`）", path.display()))?;
    if actual == expected {
        return Ok(format!(
            "{} 例与 {} 逐字节一致（自检亦过）",
            CASES.len(),
            path.display()
        ));
    }
    Err(format!(
        "{} 与当前代码不一致 —— 要么改了 canonical 实现（那就重新 `--gen`），\
         要么手改了文件（请改回）：\n{}",
        path.display(),
        first_line_diff(&expected, &actual)
    ))
}

/// `--gen` for the container vectors. ｜ 生成容器向量。
pub fn generate_container(path: &Path) -> Result<String, String> {
    verify_container()?;
    let body = render_container()?;
    write_bytes(path, &body)?;
    Ok(format!(
        "已生成 {} 例容器向量 → {}（{} B）",
        CASES.len(),
        path.display(),
        body.len()
    ))
}

/// `--check` for the container vectors. ｜ 校验容器向量。
pub fn check_container(path: &Path) -> Result<String, String> {
    verify_container()?;
    let expected = render_container()?;
    let actual = std::fs::read_to_string(path).map_err(|e| {
        format!(
            "读 {} 失败：{e}（先跑 `amberice vectors --gen`）",
            path.display()
        )
    })?;
    if actual == expected {
        return Ok(format!(
            "{} 例容器向量与 {} 逐字节一致（自检亦过）",
            CASES.len(),
            path.display()
        ));
    }
    Err(format!(
        "{} 与当前代码不一致 —— 要么改了容器布局（那就重新 `--gen`），\
         要么手改了文件（请改回）：\n{}",
        path.display(),
        first_line_diff(&expected, &actual)
    ))
}

/// Write bytes **without** newline translation (Windows must not turn `\n` into `\r\n`).
/// 按字节写盘（Windows 上不许把 `\n` 翻成 `\r\n`）。
fn write_bytes(path: &Path, body: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        if !dir.as_os_str().is_empty() {
            std::fs::create_dir_all(dir)
                .map_err(|e| format!("建目录 {} 失败：{e}", dir.display()))?;
        }
    }
    std::fs::write(path, body.as_bytes()).map_err(|e| format!("写 {} 失败：{e}", path.display()))
}

fn first_line_diff(expected: &str, actual: &str) -> String {
    let mut el = expected.lines();
    let mut al = actual.lines();
    let mut n = 0usize;
    loop {
        n += 1;
        match (el.next(), al.next()) {
            (Some(e), Some(a)) if e == a => continue,
            (Some(e), Some(a)) => {
                return format!("  首个差异在第 {n} 行：\n    期望: {e}\n    实得: {a}")
            }
            (Some(e), None) => return format!("  文件提前结束于第 {n} 行，期望: {e}"),
            (None, Some(a)) => return format!("  文件多出第 {n} 行：{a}"),
            (None, None) => return "  （行内容全同，差异在行尾换行符）".to_string(),
        }
    }
}

/// Resolve the default location: cwd first, then two levels up (`amberice` usually runs from
/// `rust/` or `rust/amberice/`). ｜ 默认落点解析：先看 cwd，再往上找两级。
pub fn resolve_default() -> PathBuf {
    resolve_in(DEFAULT_PATH)
}

/// Same resolution for the container vectors. ｜ 容器向量用同一套落点解析。
pub fn resolve_container_default() -> PathBuf {
    resolve_in(CONTAINER_PATH)
}

fn resolve_in(rel: &str) -> PathBuf {
    let up = |n: usize| {
        let mut p = String::new();
        for _ in 0..n {
            p.push_str("../");
        }
        p.push_str(rel);
        p
    };
    for cand in [rel.to_string(), up(1), up(2)] {
        if Path::new(&cand).is_file() {
            return PathBuf::from(cand);
        }
    }
    PathBuf::from(rel)
}

fn hex(bytes: &[u8]) -> String {
    let mut s = String::with_capacity(bytes.len() * 2);
    for b in bytes {
        s.push_str(&format!("{b:02x}"));
    }
    s
}

fn push_json_str(out: &mut String, s: &str) {
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => out.push_str(&format!("\\u{:04x}", c as u32)),
            c => out.push(c),
        }
    }
    out.push('"');
}
