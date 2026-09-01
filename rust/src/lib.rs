// SPDX-License-Identifier: MulanPSL-2.0
//! SML — SNOWARE Markup Language (Rust 实现, crate 名 `sml`)
//!
//! 声明式数据/配置格式, JSON/YAML 的替代品。语法与 Soup 生态的
//! `lib/sml.soup` (Lua) 对齐:
//!
//! ```sml
//! firstName: John
//! age: 27
//! address:
//! {
//!     streetAddress: "21 2nd Street"
//!     state: NY
//! }
//! phoneNumbers: [ { type: home } { type: office } ]
//! @base { region: cn-north-1 }
//! # 片段以「值」的形式引用：region 会展开为 @base 定义的内容
//! region: &base
//! ```
//!
//! 特性:
//! - 引号可选 (裸词即字符串)
//! - 块冒号可省 (`address { }` ≡ `address: { }`)
//! - 数组分隔灵活 (逗号可选)
//! - 片段继承 (`@name { }` 定义 / `&name` 引用)
//! - `include "path"` 引入外部文件（见 [`parse_file`]）
//! - `$env.VAR` 环境变量内联
//! - `#` 行注释
//! - 类型自识别: true/false -> bool, null -> None, 数字 -> i64/f64, 其余 -> String
//!
//! 值模型: `Value` 枚举 (与 JSON 同构, 另加 `__type`/`__name` 裸块元数据)。
//!
//! # 纯解析 vs 文件解析
//!
//! [`parse`] 是**纯函数**（只吃字符串，不做 IO），因此不含 include 处理。
//! 需要 include 时用 [`parse_file`]，它会先展开指令再交给 `parse`。
//! 这样设计保证了 `parse` 的可嵌入性（如 WASM / 沙箱内无文件系统）。
//!
//! # Cargo features
//!
//! - `serde`（默认关闭）：`Value` 实现 `Serialize`/`Deserialize`，可与
//!   serde_json / serde_yaml / toml 等任意 serde 后端互通；同时提供
//!   [`serde::from_str`] / [`serde::from_value`] / [`serde::to_value`] /
//!   [`serde::to_string`] 桥接函数，任何 `#[derive(serde::Deserialize)]`
//!   类型都能像 toml-rs 一样一键从 SML 反序列化（无需 `SmlDeserialize`）。
//! - `derive`（默认开启）：提供 [`SmlSerialize`] / [`SmlDeserialize`]
//!   两个 derive 宏，把自定义结构体/枚举「自然地」序列化为 SML，
//!   无需引入 serde。
//! - `sml`（默认开启）：启用 `to_sml` 序列化器（把 [`Value`] 渲染回 SML 文本）。
//!   纯解析场景可 `default-features = false` 关闭它，获得完全零依赖。
//! - `emit-markdown` / `emit-latex` / `emit-xml` / `emit-svg` / `emit-slint` /
//!   `emit-custom`（默认全部开启）：把 [`Value`] 转译为 Markdown、LaTeX、
//!   XML（含 LVGL UI）、SVG、Slint DSL，或用户自定义生成器。详见 [`emit`]。
//!
//! ```toml
//! sml-rs = { version = "0.2", features = ["serde"] }
//! # 不需要宏时可关闭默认 feature，回到完全零依赖：
//! sml-rs = { version = "0.2", default-features = false }
//! # 只解析 + 转 Markdown：
//! sml-rs = { version = "0.2", default-features = false, features = ["emit-markdown"] }
//! ```
//!
//! ## 多目标转译示例
//!
//! SML 用「原生名即块类型」的声明层（`h1`/`p`/`ul`/`table` 是字段名，
//! 其值是块内容）。解析后交给 [`emit`] 各后端：
//!
//! ```rust,ignore
//! use sml::emit::*;
//! let v = sml::parse("h1 { text: \"Hello\" }\np { text: \"world\" }").unwrap();
//! // Markdown:  # Hello\n\nworld\n
//! let md = to_markdown(&v, &MarkdownOptions::new()).unwrap();
//! // XML:       <h1>Hello</h1><p>world</p>
//! let xml = to_xml(&v, &XmlOptions::new()).unwrap();
//! // LVGL UI:  <screen><label .../>...</screen>
//! let lvgl = to_lvgl(&v, &XmlOptions::new()).unwrap();
//! ```

// ---------------------------------------------------------------------------
// 模块拆分 (见各子模块文件)
// BUG(workaround): some editors strip the BOM; normalize on load. іӏоѵеԛіанхун

// ---------------------------------------------------------------------------

use std::collections::BTreeMap;

mod value;
mod core;
mod c_abi;
/// 解析期条件/重复原语（`@when`，未来 `@for`）。
///
/// 由 cargo feature `when` 门控：不需要这套能力的构建可关掉，省掉相关代码。
/// 注意运行时 `Feature::When` 不受此门控（兼容性是文档属性，须始终可判定），
/// 详见模块文档。
#[cfg(feature = "when")]
mod cond;
#[cfg(feature = "serde")]
mod serde_bridge;
mod derive_macro;
#[cfg(any(
    feature = "emit-markdown",
    feature = "emit-latex",
    feature = "emit-xml",
    feature = "emit-svg",
    feature = "emit-slint",
    feature = "emit-custom"
))]
pub mod emit;

// re-export 公共 API
pub use crate::value::*;
pub use crate::core::*;
pub use crate::c_abi::*;

// derive trait + 宏 (两个不同命名空间：手写 trait + swsml_derive 提供的 derive 宏)
#[cfg(feature = "derive")]
pub use crate::derive_macro::{SmlSerialize, SmlDeserialize, __private, from_str};
#[cfg(all(feature = "derive", feature = "sml"))]
pub use crate::derive_macro::to_string;
#[cfg(feature = "derive")]
pub use swsml_derive::{SmlDeserialize, SmlSerialize};

// serde 桥接 (可选 feature) —— 桥接函数放在 `sml::serde::*` 命名空间，
// 与 derive 体系的 `sml::to_string` / `sml::from_str`（基于 SmlSerialize trait）区分。
#[cfg(feature = "serde")]
pub mod serde {
    pub use crate::serde_bridge::*;
}


mod tests {
    use super::*;

    // ---------------- version ----------------

    #[test]
    fn version_defaults_to_v1_when_absent() {
        // 既有文档没有版本声明，必须仍能解析且默认为 V1（裸词即字符串，向后兼容）
        let (v, ver) = parse_versioned("a: 1\n").unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declared_as_v1() {
        let (v, ver) = parse_versioned("@version v1\na: 1\n").unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
    }

    #[test]
    fn version_declaration_is_stripped_not_parsed_as_content() {
        // 若未剥离，`@version v1` 会被当成片段定义而解析异常
        let v = parse("@version v1\na: 1\n").unwrap();
        assert_eq!(v.get("a"), Some(&Value::Int(1)));
        assert!(v.get("version").is_none(), "@version 不应进入数据");
    }

    #[test]
    fn unsupported_version_is_rejected() {
        let err = parse_versioned("@version v99\na: 1\n").unwrap_err();
        assert!(err.contains("不支持"), "应拒绝不支持的版本，got: {err}");
        assert!(err.contains("v99"), "错误应含版本号，got: {err}");
    }

    #[test]
    fn conflicting_version_is_rejected() {
        let err = parse_versioned("@version v1\n@version v2\n").unwrap_err();
        // v2 尚未定义，优先报「不支持」
        assert!(!err.is_empty());
        // 两个都支持但不一致时的路径：v1 与 v1 不冲突
        let (_, ver) = parse_versioned("@version v1\n@version v1\n").unwrap();
        assert_eq!(ver, Version::V1, "重复但一致的声明应被接受");
    }

    #[test]
    fn version_is_reserved_as_fragment_name() {
        let err = parse("@version { x: 1 }\n").unwrap_err();
        assert!(err.contains("保留") || err.contains("版本声明"), "got: {err}");
    }

    #[test]
    fn version_works_with_include() {
        let d = tmpdir("version");
        std::fs::write(d.join("p.sml"), "@version v1\nb: 2\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"p.sml\"\n").unwrap();
        let (v, ver) = parse_file_versioned(d.join("main.sml")).unwrap();
        assert_eq!(ver, Version::V1);
        assert_eq!(v.get("b"), Some(&Value::Int(2)), "版本与 include 应协同");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn version_display_matches_name() {
        assert_eq!(Version::V1.name(), "v1");
        assert_eq!(format!("{}", Version::V1), "v1");
    }

    // ---------------- include ----------------

    /// 在临时目录下建文件，返回目录句柄（drop 时自动清理）
    fn tmpdir(tag: &str) -> std::path::PathBuf {
        let mut d = std::env::temp_dir();
        d.push(format!("sml_test_{tag}_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(&d).expect("create tmpdir");
        d
    }

    #[test]
    fn include_inlines_external_file() {
        let d = tmpdir("inline");
        std::fs::write(d.join("part.sml"), "port: 8080\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\nhost: local\ninclude \"part.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("host").unwrap().as_str(), Some("local"));
        assert_eq!(v.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_at_prefix_is_equivalent() {
        let d = tmpdir("at");
        std::fs::write(d.join("p.sml"), "b: 2\n").unwrap();
        std::fs::write(d.join("m.sml"), "@include \"p.sml\"\n").unwrap();
        let v = parse_file(d.join("m.sml")).unwrap();
        assert_eq!(v.get("b"), Some(&Value::Int(2)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_resolves_relative_to_including_file() {
        // 关键：相对路径按「被包含文件自身目录」解析，而非进程工作目录
        let d = tmpdir("nested");
        std::fs::create_dir_all(d.join("sub")).unwrap();
        std::fs::write(d.join("sub/leaf.sml"), "@version v1\nleaf: yes\n").unwrap();
        // mid 在根，include sub/mid2；mid2 在 sub 内，include leaf.sml（相对 sub）
        std::fs::write(d.join("sub/mid2.sml"), "@version v1\ninclude \"leaf.sml\"\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"sub/mid2.sml\"\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(
            v.get("leaf").unwrap().as_str(),
            Some("yes"),
            "嵌套 include 的路径应相对各自所在目录解析"
        );
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_inside_block_injects_fields() {
        // 文本内联语义：可在块内注入一组字段
        let d = tmpdir("block");
        std::fs::write(d.join("fields.sml"), "@version v1\nregion: cn-north-1\nzone: a\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\nserver web {\ninclude \"fields.sml\"\nport: 8080\n}\n").unwrap();

        let v = parse_file(d.join("main.sml")).unwrap();
        let server = v.get("server").expect("应有 server 块");
        assert_eq!(server.get("region").unwrap().as_str(), Some("cn-north-1"));
        assert_eq!(server.get("zone").unwrap().as_str(), Some("a"));
        assert_eq!(server.get("port"), Some(&Value::Int(8080)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_detects_cycles() {
        let d = tmpdir("cycle");
        std::fs::write(d.join("a.sml"), "include \"b.sml\"\n").unwrap();
        std::fs::write(d.join("b.sml"), "include \"a.sml\"\n").unwrap();
        let err = parse_file(d.join("a.sml")).unwrap_err();
        assert!(err.contains("循环引用"), "应报循环引用，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_missing_file_is_error() {
        let d = tmpdir("missing");
        std::fs::write(d.join("m.sml"), "include \"nope.sml\"\n").unwrap();
        let err = parse_file(d.join("m.sml")).unwrap_err();
        assert!(err.contains("nope.sml"), "错误应含缺失文件名，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn hash_in_quoted_string_is_not_a_comment() {
        // 引号内的 # 不应被当成注释，否则 `include "a#b.sml"` 会被截断
        assert_eq!(strip_line_comment("k: \"a#b\""), "k: \"a#b\"");
        assert_eq!(strip_line_comment("k: v # comment"), "k: v ");
    }

    #[test]
    fn glob_include_requires_feature() {
        // 未开启 glob-include 时，`*` 模式应报错
        let d = tmpdir("globoff");
        std::fs::write(d.join("a.sml"), "x: 1\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"*.sml\"\n").unwrap();
        let err = parse_file(d.join("main.sml")).unwrap_err();
        assert!(err.contains("glob-include"), "应要求 glob-include，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn glob_include_expands_multiple_files() {
        // 开启 glob-include 后，`lib/*.sml` 展开为子目录下所有 .sml（main.sml 不在该目录，避免自包含）
        let d = tmpdir("glob");
        std::fs::create_dir_all(d.join("lib")).unwrap();
        std::fs::write(d.join("lib/a.sml"), "@version v1\nx: 1\n").unwrap();
        std::fs::write(d.join("lib/b.sml"), "@version v1\ny: 2\n").unwrap();
        std::fs::write(d.join("note.txt"), "ignored\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\n@feature enable glob-include\ninclude \"lib/*.sml\"\n").unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(1)));
        assert_eq!(v.get("y"), Some(&Value::Int(2)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn regex_include_requires_feature() {
        let d = tmpdir("regexoff");
        std::fs::write(d.join("a.sml"), "x: 1\n").unwrap();
        std::fs::write(d.join("main.sml"), "@version v1\ninclude \"re:.*\\.sml\"\n").unwrap();
        let err = parse_file(d.join("main.sml")).unwrap_err();
        assert!(err.contains("regex-include"), "应要求 regex-include，got: {err}");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn regex_include_matches_files() {
        let d = tmpdir("regex");
        std::fs::write(d.join("widget_a.sml"), "@version v1\nx: 1\n").unwrap();
        std::fs::write(d.join("widget_b.sml"), "@version v1\ny: 2\n").unwrap();
        std::fs::write(d.join("other.sml"), "@version v1\nz: 3\n").unwrap();
        std::fs::write(
            d.join("main.sml"),
            "@version v1\n@feature enable regex-include\ninclude \"re:widget_.*\\.sml\"\n",
        )
        .unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(1)));
        assert_eq!(v.get("y"), Some(&Value::Int(2)));
        assert_eq!(v.get("z"), None, "other.sml 不应被正则匹配");
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn ext_rewrite_allows_non_sml() {
        // ext-rewrite 开启时，include 非 .sml 文件按 sml 解析
        let d = tmpdir("exrew");
        std::fs::write(d.join("conf.smlc"), "@version v1\nx: 9\n").unwrap();
        std::fs::write(
            d.join("main.sml"),
            "@version v1\n@feature enable ext-rewrite\ninclude \"conf.smlc\"\n",
        )
        .unwrap();
        let v = parse_file(d.join("main.sml")).unwrap();
        assert_eq!(v.get("x"), Some(&Value::Int(9)));
        let _ = std::fs::remove_dir_all(&d);
    }

    #[test]
    fn include_line_is_not_confused_with_key_named_include() {
        let f = FeatureSet::baseline();
        // `key: include` 不是指令——前面有 key 与冒号
        assert_eq!(parse_include_line("key: include", f), Ok(None));
        // 带扩展名无 as ⇒ 普通内联（namespace = None）
        assert_eq!(
            parse_include_line("include \"a.sml\"", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None }]))
        );
        // @include 等价
        assert_eq!(
            parse_include_line("@include \"a.sml\"", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None }]))
        );
        // 显式 as ns
        assert_eq!(
            parse_include_line("include \"a.sml\" as ui.form", f),
            Ok(Some(vec![IncludeTarget { raw: "a.sml".into(), namespace: Some("ui.form".into()), via_import: false, keys: None }]))
        );
        // 无扩展名 ⇒ implicit-ns 默认 as 文件名
        assert_eq!(
            parse_include_line("include \"widgets\"", f),
            Ok(Some(vec![IncludeTarget { raw: "widgets".into(), namespace: Some("widgets".into()), via_import: false, keys: None }]))
        );
        // import 别名
        assert_eq!(
            parse_include_line("import ui.buttons", f),
            Ok(Some(vec![IncludeTarget { raw: "ui.buttons".into(), namespace: Some("ui.buttons".into()), via_import: true, keys: None }]))
        );
        // 多目标（需 multi-include）
        let fm = FeatureSet::all();
        assert_eq!(
            parse_include_line("include \"a.sml\", \"b\" as y", fm),
            Ok(Some(vec![
                IncludeTarget { raw: "a.sml".into(), namespace: None, via_import: false, keys: None },
                IncludeTarget { raw: "b".into(), namespace: Some("y".into()), via_import: false, keys: None },
            ]))
        );
        // 注释行不生效
        assert_eq!(parse_include_line("# include \"a.sml\"", f), Ok(None));
    }

    #[test]
    fn import_partial_keys_both_syntaxes() {
        let f = FeatureSet::all();
        // 语法①：import "x.sml" as w { a, b }
        assert_eq!(
            parse_include_line("import \"m.sml\" as w { a, b }", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: Some("w".into()),
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法①无 as：平铺挑键（namespace 为 None，不触发 implicit-ns）
        assert_eq!(
            parse_include_line("import \"m.sml\" { a, b }", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: None,
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法②：import { a, b } as w in "m.sml"
        assert_eq!(
            parse_include_line("import { a, b } as w in \"m.sml\"", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: Some("w".into()),
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 语法②无 as：平铺挑键
        assert_eq!(
            parse_include_line("import { a, b } in \"m.sml\"", f),
            Ok(Some(vec![IncludeTarget {
                raw: "m.sml".into(),
                namespace: None,
                via_import: true,
                keys: Some(vec!["a".into(), "b".into()]),
            }]))
        );
        // 空键列表报错
        assert!(parse_include_line("import \"m.sml\" { }", f).is_err());
        // 语法②缺少 in "file" 报错
        assert!(parse_include_line("import { a, b } as w", f).is_err());
        // 部分引用不能配 glob 通配
        assert!(parse_include_line("import \"*.sml\" { a }", f).is_err());
    }

    // ---------------- 邮箱 / 裸词中的 @ ----------------

    #[test]
    fn email_in_bare_word_survives() {
        // 回归：裸词中的 `@` 曾被切成 At token，导致邮箱被截断为 `a`
        let v = parse("to: a@b.c\nfrom: \"SML Team <dev@mail.swebase.cn>\"\n").unwrap();
        assert_eq!(v.get("to").unwrap().as_str(), Some("a@b.c"), "got: {v:?}");
        assert_eq!(
            v.get("from").unwrap().as_str(),
            Some("SML Team <dev@mail.swebase.cn>"),
            "got: {v:?}"
        );
    }

    #[test]
    fn email_roundtrips_through_to_sml() {
        let v = Value::Object(BTreeMap::from([(
            "to".to_string(),
            Value::Str("dev@mail.swebase.cn".into()),
        )]));
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "邮箱必须能往返，got:\n{}", to_sml(&v));
    }

    #[test]
    fn fragment_definition_still_works() {
        // 词首的 `@` 仍是片段定义标记，不能被上面的修改破坏。
        // 注：SML 的片段继承用法是「定义后作为值引用」（`k: &base`）；
        // 块内裸写 `&base` 会被当作键，不属于本用例覆盖范围。
        let v = parse("@base { region: cn }\nregion: &base\n").unwrap();
        assert_eq!(
            v.get("region").unwrap().get("region").unwrap().as_str(),
            Some("cn"),
            "片段引用应展开为定义的内容，got: {v:?}"
        );
    }

    // ---------------- 顶层数组 / 对象（与 to_sml 对称）----------------

    #[test]
    fn toplevel_array_roundtrips() {
        // 回归：to_sml 能输出顶层数组，但 parse 曾只认键值块，
        // 导致「能写不能读」（"期望键, 得 LBrack"）。
        let v = Value::Array(vec![
            Value::Object(BTreeMap::from([
                ("ts".to_string(), Value::Str("2026-01-01".into())),
                ("to".to_string(), Value::Str("a@b.c".into())),
            ])),
            Value::Object(BTreeMap::from([
                ("ts".to_string(), Value::Str("2026-01-02".into())),
                ("to".to_string(), Value::Str("x@y.z".into())),
            ])),
        ]);
        let text = to_sml(&v);
        let back = parse(&text).unwrap();
        assert_eq!(back, v, "顶层对象数组必须能往返，got text:\n{text}");
    }

    #[test]
    fn toplevel_array_of_scalars_roundtrips() {
        let v = Value::Array(vec![
            Value::Int(1),
            Value::Str("two".into()),
            Value::Bool(true),
        ]);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "顶层标量数组必须能往返");
    }

    #[test]
    fn toplevel_object_block_roundtrips() {
        let mut m = BTreeMap::new();
        m.insert("k".to_string(), Value::Int(1));
        let v = Value::Object(m);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "顶层对象块必须能往返");
    }

    #[test]
    fn toplevel_empty_array_roundtrips() {
        let v = Value::Array(vec![]);
        let back = parse(&to_sml(&v)).unwrap();
        assert_eq!(back, v, "空数组必须能往返");
    }

    // ---------------- serde ----------------

    #[cfg(feature = "serde")]
    #[test]
    fn serde_roundtrip_preserves_shape() {
        let v = parse("name: John\nage: 27\ntags: [a b]\nnested { k: v }\n").unwrap();
        let json = serde_json::to_string(&v).unwrap();
        // 自然形状：字符串就是字符串，数字就是数字，而非 {"Int":27}
        assert!(json.contains("\"name\":\"John\""), "got: {json}");
        assert!(json.contains("\"age\":27"), "got: {json}");
        assert!(json.contains("\"tags\":[\"a\",\"b\"]"), "got: {json}");
        assert!(json.contains("\"nested\":{\"k\":\"v\"}"), "got: {json}");

        let back: Value = serde_json::from_str(&json).unwrap();
        assert_eq!(back, v, "serde 往返应还原原值");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn serde_deserializes_json_into_value() {
        let v: Value = serde_json::from_str(r#"{"s":"x","i":5,"f":1.5,"b":true,"n":null,"a":[1,2]}"#).unwrap();
        assert_eq!(v.get("s").unwrap().as_str(), Some("x"));
        assert_eq!(v.get("i"), Some(&Value::Int(5)));
        assert_eq!(v.get("f"), Some(&Value::Float(1.5)));
        assert_eq!(v.get("b"), Some(&Value::Bool(true)));
        assert_eq!(v.get("n"), Some(&Value::Null));
        assert!(matches!(v.get("a"), Some(Value::Array(a)) if a.len() == 2));
    }

    #[test]
    fn nested_array_inside_object_inside_array_survives_roundtrip() {
        // 回归测试：数组元素是对象、对象里又有数组（如配置的条目列表）。
        // dump_inline 曾把嵌套数组缩略成 [..]，导致 chunks 丢成 [".."]。
        let mut item = BTreeMap::new();
        item.insert("path".to_string(), Value::Str("a.txt".into()));
        item.insert(
            "chunks".to_string(),
            Value::Array(vec![
                Value::Str("c1".into()),
                Value::Str("c2".into()),
            ]),
        );
        let mut root = BTreeMap::new();
        root.insert(
            "entries".to_string(),
            Value::Array(vec![Value::Object(item)]),
        );
        let text = to_sml(&Value::Object(root));
        assert!(!text.contains("[..]"), "嵌套数组不得被缩略: {text}");

        let back = parse(&text).unwrap();
        let chunks = back.get("entries").and_then(|e| match e {
            Value::Array(a) => a.first(),
            _ => None,
        });
        let chunks = match chunks {
            Some(Value::Object(m)) => m.get("chunks"),
            _ => None,
        };
        match chunks {
            Some(Value::Array(a)) => {
                assert_eq!(a.len(), 2, "两个块都应保留: {text}");
                assert_eq!(
                    a.iter().filter_map(|c| c.as_str()).collect::<Vec<_>>(),
                    vec!["c1", "c2"]
                );
            }
            other => panic!("chunks 应解析为数组，实际 {other:?}"),
        }
    }

    #[test]
    fn utf8_in_quoted_string_survives_roundtrip() {
        // 回归测试：tokenizer 曾按字节 `as char` 逐个处理，
        // 把 UTF-8 多字节字符拆成 Latin-1 字符，导致
        // `"修复若干问题"` 解析后变成双编码乱码。
        let v = parse(r#"note: "修复若干问题""#).unwrap();
        assert_eq!(
            v.get("note").and_then(|x| x.as_str()),
            Some("修复若干问题"),
            "引号串中的中文不应被破坏"
        );
        // 裸词中文同样不能破坏
        let v2 = parse("region: 华北").unwrap();
        assert_eq!(v2.get("region").and_then(|x| x.as_str()), Some("华北"));
        // 转义 \u 序列
        let v3 = parse(r#"k: "\u{4fee}\u{590d}""#).unwrap();
        assert_eq!(v3.get("k").and_then(|x| x.as_str()), Some("修复"));
    }

    #[test]
    fn nested_depth_within_limit_succeeds() {
        // 在 MAX_VALUE_DEPTH(128) 内应正常解析：每层 `a: {`
        let open = "a: {".repeat(120);
        let close = "}".repeat(120);
        let text = format!("{open} leaf: 1{close}");
        let v = parse(&text);
        assert!(v.is_ok(), "120 层嵌套应解析成功，实际 {v:?}");
    }

    #[test]
    fn deep_nesting_exceeding_limit_is_rejected() {
        // `a: { a: { ... }}` 超过 128 层必须返回 Err，绝不能栈溢出(abort/segfault)
        let open = "a: {".repeat(1000);
        let close = "}".repeat(1000);
        let text = format!("{open} leaf: 1{close}");
        let v = parse(&text);
        assert!(
            v.is_err(),
            "超深嵌套必须被拒绝（返回 Err），而不是栈溢出崩溃；实际 {v:?}"
        );
        assert!(
            v.unwrap_err().contains("嵌套过深"),
            "错误信息应提示嵌套过深"
        );
    }

    #[test]
    fn serialize_string_literalization_is_prevented() {
        // B1: "true"/"null"/"8080"/"1.5" 序列化后必须仍是字符串，不能变 Bool/Null/Int/Float
        for (raw, expect) in [
            ("true", "\"true\""),
            ("false", "\"false\""),
            ("null", "\"null\""),
            ("8080", "\"8080\""),
            ("1.5", "\"1.5\""),
            ("inf", "\"inf\""),
        ] {
            let v = Value::Object({
                let mut m = BTreeMap::new();
                m.insert("k".into(), Value::Str(raw.into()));
                m
            });
            let out = to_sml(&v);
            assert!(
                out.contains(expect),
                "字符串 {:?} 序列化应加引号得到 {}，实际: {:?}",
                raw,
                expect,
                out
            );
            // round-trip: 再解析仍是 Str
            let back = parse(&out).unwrap();
            assert_eq!(
                back.get("k").and_then(|x| x.as_str()),
                Some(raw),
                "round-trip 后 {:?} 应仍是字符串",
                raw
            );
        }
    }

    #[test]
    fn b8_unicode_escape_rejects_short_and_invalid() {
        // B8: \uXXXX 必须定长读 4 个十六进制数字；不足或非法必须报错，
        // 绝不能静默丢弃并已吃掉闭合引号导致后续内容被吞并。
        // 不足 4 位（这里只有 3 位 D8 0）应为 Err。
        let short = r#"k: "\uD80""#;
        assert!(
            parse(short).is_err(),
            "B8: \\u 后不足 4 位十六进制必须报错，实际解析为 {:?}",
            parse(short)
        );
        // 非法码点（代理区 / 非 hex / 空 hex）必须报错。
        for bad in [r#"k: "\uD800""#, r#"k: "\uZZZZ""#, r#"k: "\u""#] {
            assert!(
                parse(bad).is_err(),
                "B8: 非法 \\u 转义 {:?} 必须报错",
                bad
            );
        }
        // 合法 4 位十六进制必须成功，且码点正确。
        let good = r#"k: "\u4e2d""#; // 中
        let v = parse(good).expect("B8: 合法 \\u4e2d 应成功");
        assert_eq!(v.get("k").and_then(|x| x.as_str()), Some("中"));
    }

    #[test]
    fn b9_unterminated_string_errors() {
        // B9: 未闭合的字符串必须报错，绝不能静默吞并后续所有行。
        let unclosed = "k: \"hello";
        assert!(
            parse(unclosed).is_err(),
            "B9: 未闭合字符串必须报错，实际 {:?}",
            parse(unclosed)
        );
        // 转义符后遇 EOF 也必须报错。
        let dangling_escape = "k: \"abc\\";
        assert!(
            parse(dangling_escape).is_err(),
            "B9: 转义符后 EOF 必须报错，实际 {:?}",
            parse(dangling_escape)
        );
        // 关键：报错后不应把后续内容误吞为同一字符串（验证独立解析正常）。
        let after = "k: \"hi\"\nother: 1";
        let v = parse(after).expect("B9: 正常多行应成功");
        assert_eq!(
            match v.get("other") {
                Some(Value::Int(i)) => Some(*i),
                _ => None,
            },
            Some(1)
        );
    }

    #[test]
    fn b10_overflow_int_preserved_as_string() {
        // B10: 超出 i64 范围的整数不能静默降级为 Float（丢精度）。
        // 合法 uint64 上界应保留为字符串，round-trip 零损。
        let big = "k: 9223372036854775808"; // i64::MAX + 1
        let v = parse(big).expect("B10: 大整数应可解析");
        match v.get("k") {
            Some(Value::Str(s)) => {
                assert_eq!(s, "9223372036854775808", "B10: 超 i64 整数应保留为字符串");
                // round-trip: 序列化后仍是裸字符串，解析回来一致
                let out = to_sml(&v);
                let back = parse(&out).expect("B10: round-trip 应成功");
                assert_eq!(
                    back.get("k").and_then(|x| x.as_str()),
                    Some("9223372036854775808")
                );
            }
            other => panic!("B10: 超 i64 整数应保留为 Str，实际 {:?}", other),
        }
        // 普通 i64 仍解析为 Int
        let normal = "k: 12345";
        let v = parse(normal).unwrap();
        assert_eq!(
            match v.get("k") {
                Some(Value::Int(i)) => Some(*i),
                _ => None,
            },
            Some(12345)
        );
    }

    #[test]
    fn b11_fragment_reference_is_value_not_merge() {
        // B11: 文档示例 `server web { &base port: 8080 }` 无法解析（裸词当键）。
        // 正确语义：&name 是值引用，写作 `key: &name`。
        // 错误写法必须报错：
        assert!(
            parse("@base { region: cn }\nserver web { &base port: 8080 }").is_err(),
            "B11: 块内裸写 &base 必须报错（不是合并语义）"
        );
        // 正确写法解析成功，&base 作为值引用附着在显式键上：
        let ok = "@base { region: cn-north-1 }\nregion: &base";
        let v = parse(ok).expect("B11: region: &base 应成功");
        let region = v.get("region").expect("B11: 应有 region 键");
        assert!(
            matches!(region, Value::Object(_)),
            "B11: &base 作为值应展开为对象，实际 {:?}",
            region
        );
        assert_eq!(
            region.get("region").and_then(|x| x.as_str()),
            Some("cn-north-1")
        );
    }

    #[test]
    fn serialize_comment_prefix_is_quoted() {
        // B2: 以注释符开头的字符串必须引号，否则再解析被吞成 Null
        for raw in ["--flag", "//path", "/*x*/", "_*x*_"] {
            let v = Value::Object({
                let mut m = BTreeMap::new();
                m.insert("k".into(), Value::Str(raw.into()));
                m
            });
            let out = to_sml(&v);
            assert!(out.contains(&format!("\"{}\"", raw)), "{} 应被引号, 得 {:?}", raw, out);
            let back = parse(&out).unwrap();
            assert_eq!(back.get("k").and_then(|x| x.as_str()), Some(raw));
        }
    }

    #[test]
    fn serialize_comma_and_brackets_are_quoted() {
        // B3/B4: 含逗号/方括号的字符串必须引号，否则裂键或截断
        for raw in ["a,b", "a[b", "a]b", "[", "]"] {
            let v = Value::Object({
                let mut m = BTreeMap::new();
                m.insert("k".into(), Value::Str(raw.into()));
                m
            });
            let out = to_sml(&v);
            assert!(out.contains(&format!("\"{}\"", raw)), "{} 应被引号, 得 {:?}", raw, out);
            let back = parse(&out).unwrap();
            assert_eq!(back.get("k").and_then(|x| x.as_str()), Some(raw));
        }
    }

    #[test]
    fn serialize_keys_are_quoted_when_needed() {
        // B5: 含特殊字符的键必须引号，否则文档结构损坏
        for key in ["a b", "a#b", "a,b", "http://x"] {
            let mut m = BTreeMap::new();
            m.insert(key.to_string(), Value::Int(1));
            let out = to_sml(&Value::Object(m));
            let back = parse(&out).unwrap();
            let got = back.get(key).and_then(|x| match x {
                Value::Int(n) => Some(*n),
                _ => None,
            });
            assert_eq!(got, Some(1), "键 {:?} 应可 round-trip, 得 {:?}", key, out);
        }
    }

    #[test]
    fn serialize_float_keeps_decimal_point() {
        // B6: Float(1.0) 必须序列化为 "1.0"，round-trip 回来仍是 Float
        let v = Value::Object({
            let mut m = BTreeMap::new();
            m.insert("f".into(), Value::Float(1.0));
            m
        });
        let out = to_sml(&v);
        let back = parse(&out).unwrap();
        assert_eq!(back.get("f"), Some(&Value::Float(1.0)), "Float(1.0) 不能变成 Int, 得 {:?}", out);
    }

    #[test]
    fn parse_nested_array_roundtrip() {
        // B7: 嵌套数组应能被解析
        let v = parse("a: [[1 2] [3 4]]").unwrap();
        let a = match v.get("a") {
            Some(Value::Array(a)) => a,
            _ => panic!("a 应为数组"),
        };
        assert_eq!(a.len(), 2);
        assert_eq!(
            match &a[0] {
                Value::Array(x) => x.len(),
                _ => 0,
            },
            2
        );
    }

    #[test]
    fn parse_basic() {
        let text = "firstName: John\nage: 27\nisAlive: true\nspouse: null\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("firstName"), Some(&Value::Str("John".into())));
        assert_eq!(v.get("age"), Some(&Value::Int(27)));
        assert_eq!(v.get("isAlive"), Some(&Value::Bool(true)));
        assert_eq!(v.get("spouse"), Some(&Value::Null));
    }

    #[test]
    fn parse_nested() {
        let text = "address:\n{\n    streetAddress: \"21 2nd Street\"\n    state: NY\n}\n";
        let v = parse(text).unwrap();
        assert_eq!(
            v.get("address.streetAddress"),
            Some(&Value::Str("21 2nd Street".into()))
        );
        assert_eq!(v.get("address.state"), Some(&Value::Str("NY".into())));
    }

    #[test]
    fn parse_array() {
        let text = "phoneNumbers:\n[\n    { type: home }\n    { type: office }\n]\n";
        let v = parse(text).unwrap();
        if let Some(Value::Array(a)) = v.get("phoneNumbers") {
            assert_eq!(a.len(), 2);
            assert_eq!(a[0].get("type"), Some(&Value::Str("home".into())));
        } else {
            panic!("not array");
        }
    }

    #[test]
    fn parse_fragment() {
        let text = "@base { region: cn-north-1 }\nserver web { &base }\n";
        let v = parse(text).unwrap();
        // &base 展开为字段 (键名 "&base", 值=片段对象), 与 Lua 实现一致
        assert_eq!(
            v.get("server.&base.region"),
            Some(&Value::Str("cn-north-1".into()))
        );
        assert_eq!(v.get("server.__type"), Some(&Value::Str("server".into())));
        assert_eq!(v.get("server.__name"), Some(&Value::Str("web".into())));
    }

    #[test]
    fn roundtrip() {
        let text = "name: myapp\nport: 8080\nflags: [ a b c ]\n";
        let v = parse(text).unwrap();
        let out = to_sml(&v);
        let v2 = parse(&out).unwrap();
        assert_eq!(v, v2);
    }

    #[test]
    fn env_inline() {
        // Rust 1.85+ 起 set_var 为 unsafe（与 edition 无关，2021/2024 均需）
        unsafe { std::env::set_var("SML_TEST_VAR", "hello") };
        let text = "greeting: $env.SML_TEST_VAR\n";
        let v = parse(text).unwrap();
        assert_eq!(v.get("greeting"), Some(&Value::Str("hello".into())));
    }

    #[test]
    fn c_abi_json_bridge() {
        let text = "name: John\nage: 27\n";
        let v = parse(text).unwrap();
        let j = jsonify(&v);
        assert!(j.contains("\"name\":\"John\""));
        let back = json_to_value(&j).unwrap();
        assert_eq!(back, v);
    }
}

// ===========================================================================
// @feature 特性裁剪 + 调用方限制 测试
// ===========================================================================

#[cfg(test)]
mod feature {
    use super::*;

    #[test]
    fn feature_unknown_name_errors() {
        let r = parse("@feature enable nope\nx: 1\n");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("未知特性"));
    }

    #[test]
    fn feature_whitelist_narrows() {
        // 仅保留 bareword 与 include，其它（env/fragment/contract...）关闭
        let parsed = parse("@feature whitelist bareword-string,include\nx: John\n").unwrap();
        let v = match &parsed {
            Value::Object(m) => m.clone(),
            _ => panic!("应为对象"),
        };
        assert_eq!(v.get("x"), Some(&Value::Str("John".into())));
    }

    #[test]
    fn feature_blacklist_removes() {
        // 关掉 bareword-string：v1 文档里裸词字符串也应被拒
        let r = parse("@feature blacklist bareword-string\nx: John\n");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("字符串必须加引号"));
    }

    #[test]
    fn feature_mode_whitelist_enable() {
        // mode whitelist 后基集清空，仅 enable 的生效；
        // fragment 特性开启但名字未定义时，必须报错（不再静默降级为字符串）。
        let r = parse("@feature mode whitelist\n@feature enable fragment\nx: &frag\n");
        assert!(r.is_err());
        assert!(r.unwrap_err().contains("未定义的片段引用"));
    }

    #[test]
    fn caller_allowed_intersection_empty_errors() {
        // 调用方只接受 env；文档用白名单模式只开 contract —— 与调用方无交集则报错
        let allowed = FeatureSet::none().with(Feature::Env);
        let r = parse_with_features(
            "@feature mode whitelist\n@feature enable contract\nx: 1\n",
            allowed,
        );
        assert!(r.is_err());
    }

    #[test]
    fn caller_allowed_subset_ok() {
        // 调用方允许全部，文档收窄到 bareword+include，应成功
        let allowed = FeatureSet::all();
        let (v, eff) = parse_with_features(
            "@feature whitelist bareword-string,include\nx: John\n",
            allowed,
        )
        .unwrap();
        assert!(eff.has(Feature::BarewordStr));
        assert!(eff.has(Feature::Include));
        assert!(!eff.has(Feature::Env));
        assert_eq!(v.get("x"), Some(&Value::Str("John".into())));
    }

    #[test]
    fn feature_namespace_include() {
        // 用临时文件验证 include "x.sml" as ns 把键挂到 ns 下。
        // 用相对路径 + 正斜杠，避开 Windows 反斜杠在字符串转义中的处理。
        // 注意：include 展开只在 parse_file 进行，故这里把主文档也落盘。
        let dir = std::env::temp_dir().join("sml_feat_ns_test");
        let _ = std::fs::create_dir_all(&dir);
        let sub = dir.join("sub.sml");
        let main = dir.join("main.sml");
        std::fs::write(&sub, "a: 1\nb: 2\n").unwrap();
        // 用正斜杠书写相对路径，避免反斜杠被字符串转义吃掉
        let rel = format!("include \"sub.sml\" as pkg\n");
        std::fs::write(&main, &rel).unwrap();
        let v = match parse_file(&main) {
            Ok(v) => v,
            Err(e) => {
                let _ = std::fs::remove_dir_all(&dir);
                panic!("parse_file 失败: {e}");
            }
        };
        let _ = std::fs::remove_dir_all(&dir);
        let pkg = match v.get("pkg") {
            Some(Value::Object(m)) => m.clone(),
            _ => panic!("pkg 应为对象"),
        };
        assert_eq!(pkg.get("a"), Some(&Value::Int(1)));
        assert_eq!(pkg.get("b"), Some(&Value::Int(2)));
    }

    #[test]
    fn version_v3_disables_bareword() {
        // v3 默认关闭 bareword-string；裸词应被拒
        let r = parse("@version v3\nname: John\n");
        assert!(r.is_err());
        // 但引号字符串可用
        let v = parse("@version v3\nname: \"John\"\nage: 27\n").unwrap();
        assert_eq!(v.get("name"), Some(&Value::Str("John".into())));
        assert_eq!(v.get("age"), Some(&Value::Int(27)));
    }

    #[test]
    fn feature_base_derives_strict() {
        // @feature base v3 等价于 v3 严格
        let r = parse("@feature base v3\nname: John\n");
        assert!(r.is_err());
    }
}
