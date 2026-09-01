// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! C-ABI 值树接口测试。
//!
//! 重点守护两件容易出错的事：
//!  1. `sml_feature_name` 的硬编码字面量必须与运行时的 `FEATURES` 表
//!     顺序完全一致（否则 C 侧拿到的特性名是错的）。
//!  2. 加载 / 遍历 / 取值 / 释放的生命周期约定不能破
//!     （借用指针不得被释放、根节点释放后不得再用）。

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_int, c_uint};

// 与 c/sml_rs.h 的 sml_error 布局保持一致。
// 数组字段超过 32 元素时 derive(Default) 不可用，故手动实现。
#[repr(C)]
struct CSmlError {
    code: c_int,
    line: c_int,
    column: c_int,
    position: usize,
    source: [c_char; 128],
    text: [c_char; 256],
}

impl Default for CSmlError {
    fn default() -> Self {
        CSmlError {
            code: 0,
            line: 0,
            column: 0,
            position: 0,
            source: [0; 128],
            text: [0; 256],
        }
    }
}

#[repr(transparent)]
struct CSmlValue(std::mem::MaybeUninit<u8>);

extern "C" {
    fn sml_loads(text: *const c_char, flags: c_uint, err: *mut CSmlError) -> *mut CSmlValue;
    fn sml_load_file(path: *const c_char, flags: c_uint, err: *mut CSmlError) -> *mut CSmlValue;
    fn sml_free(v: *mut CSmlValue);
    fn sml_typeof(v: *const CSmlValue) -> c_int;
    fn sml_get(v: *const CSmlValue, key: *const c_char) -> *const CSmlValue;
    fn sml_get_path(v: *const CSmlValue, path: *const c_char) -> *const CSmlValue;
    fn sml_at(v: *const CSmlValue, idx: usize) -> *const CSmlValue;
    fn sml_size(v: *const CSmlValue) -> usize;
    fn sml_str_copy(v: *const CSmlValue, buf: *mut c_char, buflen: usize) -> usize;
    fn sml_str_dup(v: *const CSmlValue) -> *mut c_char;
    fn sml_int_value(v: *const CSmlValue) -> i64;
    fn sml_real_value(v: *const CSmlValue) -> f64;
    fn sml_bool_value(v: *const CSmlValue) -> c_int;
    fn sml_str_in(v: *const CSmlValue, path: *const c_char) -> *mut c_char;
    fn sml_int_in(v: *const CSmlValue, path: *const c_char, ok: *mut c_int) -> i64;
    fn sml_bool_in(v: *const CSmlValue, path: *const c_char, ok: *mut c_int) -> c_int;
    fn sml_dumps(v: *const CSmlValue, flags: c_uint) -> *mut c_char;
    fn sml_free_str(p: *mut c_char);
    fn sml_version() -> *const c_char;
    fn sml_features_mask() -> c_uint;
    fn sml_feature_name(bit: c_uint) -> *const c_char;
}

// sml_type 枚举值（与 sml_rs.h 对齐）
const SML_TYPE_OBJECT: c_int = 6;
const SML_TYPE_ARRAY: c_int = 5;
const SML_TYPE_STR: c_int = 4;
const SML_TYPE_INT: c_int = 2;
const SML_TYPE_BOOL: c_int = 1;

// sml_errc 错误码（与 sml_rs.h 对齐）
const SML_OK: c_int = 0;
const SML_ERR_SYNTAX: c_int = 1;
const SML_ERR_FEATURE_DISABLED: c_int = 2;
const SML_ERR_VERSION_MISMATCH: c_int = 3;
const SML_ERR_CONTRACT: c_int = 4;
const SML_ERR_INCLUDE_LOOP: c_int = 5;
const SML_ERR_IO: c_int = 6;
const SML_ERR_UTF8: c_int = 7;
const SML_ERR_INTERNAL: c_int = 8;

/// 守护点 1：`sml_feature_name` 的每个位必须等于 `FEATURES` 表同名项。
#[test]
fn feature_name_matches_features_table() {
    let names = sml::feature_names();
    assert!(!names.is_empty(), "FEATURES 表不应为空");

    for (i, expected) in names.iter().enumerate() {
        let bit = i as c_uint;
        let raw = unsafe { sml_feature_name(bit) };
        assert!(!raw.is_null(), "bit {bit} 应返回有效名字");
        let got = unsafe { CStr::from_ptr(raw) }.to_str().unwrap();
        assert_eq!(got, *expected, "bit {bit} 的名字与 FEATURES 表不一致");
        // 必须是 NUL 结尾（C 侧 printf("%s") 依赖这一点）
        assert_eq!(got.len(), expected.len(), "名字不应含多余内容");
    }

    // 越界返回 NULL
    let overflow = unsafe { sml_feature_name(names.len() as c_uint) };
    assert!(overflow.is_null(), "越界位应返回 NULL");
}

/// `sml_features_mask` 应覆盖表中全部特性。
#[test]
fn features_mask_covers_all() {
    let mask = unsafe { sml_features_mask() };
    let n = sml::feature_names().len();
    for i in 0..n {
        assert_ne!(mask & (1u32 << i), 0, "bit {i} 应在掩码中");
    }
}

#[test]
fn version_is_static_and_nonempty() {
    let p = unsafe { sml_version() };
    assert!(!p.is_null());
    let s = unsafe { CStr::from_ptr(p) }.to_str().unwrap();
    assert!(s.starts_with("sml "), "版本串应以 'sml ' 开头: {s}");
    // 静态字符串：连续两次调用应返回同一地址
    let p2 = unsafe { sml_version() };
    assert_eq!(p, p2, "sml_version 应返回静态地址");
}

#[test]
fn loads_and_traverse() {
    let text = CString::new("name: John\nage: 27\nactive: true\nserver { host: a.b.c }\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null(), "解析应成功: err.code={}", err.code);

    assert_eq!(unsafe { sml_typeof(root) }, SML_TYPE_OBJECT);
    assert_eq!(unsafe { sml_size(root) }, 4);

    // 字符串
    let key = CString::new("name").unwrap();
    let node = unsafe { sml_get(root, key.as_ptr()) };
    assert!(!node.is_null());
    assert_eq!(unsafe { sml_typeof(node) }, SML_TYPE_STR);
    let dup = unsafe { sml_str_dup(node) };
    assert_eq!(
        unsafe { CStr::from_ptr(dup) }.to_str().unwrap(),
        "John"
    );
    unsafe { sml_free_str(dup) };

    // 整数
    let key = CString::new("age").unwrap();
    let node = unsafe { sml_get(root, key.as_ptr()) };
    assert_eq!(unsafe { sml_int_value(node) }, 27);

    // 布尔
    let key = CString::new("active").unwrap();
    let node = unsafe { sml_get(root, key.as_ptr()) };
    assert_eq!(unsafe { sml_typeof(node) }, SML_TYPE_BOOL);
    assert_eq!(unsafe { sml_bool_value(node) }, 1);

    // 点路径
    let path = CString::new("server.host").unwrap();
    let s = unsafe { sml_str_in(root, path.as_ptr()) };
    assert!(!s.is_null());
    assert_eq!(unsafe { CStr::from_ptr(s) }.to_str().unwrap(), "a.b.c");
    unsafe { sml_free_str(s) };

    // 取值失败时 ok=0
    let path = CString::new("server.nope").unwrap();
    let mut ok: c_int = 1;
    let v = unsafe { sml_int_in(root, path.as_ptr(), &mut ok) };
    assert_eq!(v, 0);
    assert_eq!(ok, 0, "取不到时 ok 应为 0");

    unsafe { sml_free(root) };
}

extern "C" {
    fn sml_parse(text: *const c_char) -> *mut c_char;
    fn sml_dump(v: *mut CSmlValue) -> *mut c_char;
}

/// C-ABI 审计 #1：jsonify 必须转义控制字符，否则产出非法 JSON。
#[test]
fn jsonify_escapes_control_chars() {
    // 含换行/制表符的字符串
    let text = CString::new("k: \"a\\nb\\tc\"\n").unwrap();
    let out = unsafe { sml_parse(text.as_ptr()) };
    assert!(!out.is_null(), "解析应成功");
    let s = unsafe { CStr::from_ptr(out) }.to_str().unwrap();
    // 输出必须是合法 JSON：串内不应存在裸换行字节（0x0A / 0x0D）
    assert!(!s.contains('\n'), "json 串内不应含裸换行: {s:?}");
    assert!(!s.contains('\r'), "json 串内不应含裸回车: {s:?}");
    // 应含转义后的 \n / \t
    assert!(s.contains("\\n"), "应有 \\n 转义: {s}");
    assert!(s.contains("\\t"), "应有 \\t 转义: {s}");
    unsafe { sml_free_str(out) };
}

/// C-ABI 审计 #6：NaN / inf 序列化为 JSON `null`（合法字面量），而非 "NaN"/"inf"。
#[test]
fn jsonify_nan_inf_becomes_null() {
    let text = CString::new("j: inf\nk: NaN\n").unwrap();
    let out = unsafe { sml_parse(text.as_ptr()) };
    assert!(!out.is_null());
    let s = unsafe { CStr::from_ptr(out) }.to_str().unwrap();
    // 用 serde_json 校验整个输出可解析（说明不再是非法字面量）
    let parsed: serde_json::Value = serde_json::from_str(s)
        .unwrap_or_else(|e| panic!("jsonify 输出应可解析: {s}\n{e}"));
    assert_eq!(parsed["j"], serde_json::Value::Null);
    assert_eq!(parsed["k"], serde_json::Value::Null);
    unsafe { sml_free_str(out) };
}

/// C-ABI 审计 #3：json_to_value 不得对多字节 UTF-8 二次编码。
#[test]
fn json_to_value_keeps_utf8() {
    // SML 里写中文，再 dump 回 JSON，再解析回来，字节应不变
    let text = CString::new("k: 中文\n").unwrap();
    let out = unsafe { sml_parse(text.as_ptr()) };
    assert!(!out.is_null());
    let json = unsafe { CStr::from_ptr(out) }.to_str().unwrap();
    let v = sml::json_to_value(json).expect("json_to_value 应成功");
    let got = v.get("k").and_then(|x| x.as_str()).unwrap_or_default();
    assert_eq!(got, "中文", "中文不应被二次编码");
    unsafe { sml_free_str(out) };
}

/// C-ABI 审计 #4：过深嵌套 JSON 不得令进程栈溢出（应安全返回 None）。
#[test]
fn json_to_value_depth_limit() {
    // 构造 100000 层嵌套数组
    let mut s = String::new();
    for _ in 0..100_000 {
        s.push('[');
    }
    s.push('1');
    for _ in 0..100_000 {
        s.push(']');
    }
    let v = sml::json_to_value(&s);
    assert!(v.is_none(), "超深嵌套应被拒绝而非栈溢出");
}

/// C-ABI 审计 #2：含内嵌 NUL 的字符串经 jsonify 不得产出含 NUL 的非法 JSON，
/// 也不得因 `CString::new` 失败而静默回退为空串。
#[test]
fn nul_in_string_not_silent_empty() {
    use sml::Value;
    // 直接用含 NUL 字节的字符串值走 jsonify（此前 esc 仅转义 \\ 与 "，
    // 0x00 会原样进入输出，CString::new 失败后回退空串）。
    let v = Value::Object({
        let mut m = std::collections::BTreeMap::new();
        m.insert("k".into(), Value::Str("a\u{0}b".into()));
        m
    });
    let json = sml::jsonify(&v);
    // 输出不得含裸 NUL（应转义为 \u0000）
    assert!(!json.contains('\u{0}'), "jsonify 输出不应含裸 NUL: {json:?}");
    assert!(json.contains("\\u0000"), "NUL 应被转义为 \\u0000: {json}");
    // 整个输出必须是合法 JSON（裸 NUL 会让下游解析器报错）
    let parsed: serde_json::Value = serde_json::from_str(&json)
        .unwrap_or_else(|e| panic!("含 NUL 的字符串仍应输出合法 JSON: {json}\n{e}"));
    let got = parsed["k"].as_str().unwrap();
    assert_eq!(got, "a\u{0}b", "NUL 不应丢失");
}

/// C-ABI 审计 #5：sml_load_file 的 flags 必须生效（禁用 include 时不应展开）。
#[test]
fn load_file_flags_disable_include() {
    use std::io::Write;
    let dir = std::env::temp_dir().join("sml_cabi_flags_test");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    std::fs::write(dir.join("inc.sml"), "secret: leaked\n").unwrap();
    std::fs::write(
        dir.join("main.sml"),
        // 真实 include 指令语法：include "path"（无括号）。相对路径。
        "a: 1\ninclude \"inc.sml\"\n",
    )
    .unwrap();

    let path = CString::new(dir.join("main.sml").to_str().unwrap()).unwrap();

    // flags = 1 => 仅 bit0 (bareword-string)，include 未启用
    let mut err = CSmlError::default();
    let root = unsafe { sml_load_file(path.as_ptr(), 1, &mut err) };
    assert!(root.is_null(), "flags 禁用 include 时解析应失败（flags 生效）");
    assert_ne!(err.code, SML_OK, "应返回非零错误码");

    // flags = 0（全特性）应成功并读到 include 内容，证明能力本身可用
    let mut err2 = CSmlError::default();
    let root2 = unsafe { sml_load_file(path.as_ptr(), 0, &mut err2) };
    assert!(!root2.is_null(), "全特性 flags 下 include 应展开");
    if !root2.is_null() {
        unsafe { sml_free(root2) };
    }
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn str_copy_two_call_pattern() {
    let text = CString::new("name: John\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null());

    let key = CString::new("name").unwrap();
    let node = unsafe { sml_get(root, key.as_ptr()) };

    // 第一次：传 NULL 只取长度
    let need = unsafe { sml_str_copy(node, std::ptr::null_mut(), 0) };
    assert_eq!(need, 4);

    // 第二次：实际拷贝
    let mut buf = vec![0i8; need + 1];
    let wrote = unsafe { sml_str_copy(node, buf.as_mut_ptr(), need + 1) };
    assert_eq!(wrote, 4);
    assert_eq!(
        unsafe { CStr::from_ptr(buf.as_ptr()) }.to_str().unwrap(),
        "John"
    );

    // 缓冲区不足时返回所需长度，且安全截断
    let mut small = vec![0i8; 3];
    let wrote = unsafe { sml_str_copy(node, small.as_mut_ptr(), 3) };
    assert_eq!(wrote, 4, "应返回所需长度而非已写入长度");
    assert_eq!(
        unsafe { CStr::from_ptr(small.as_ptr()) }.to_str().unwrap(),
        "Jo",
        "缓冲区不足应安全截断"
    );

    unsafe { sml_free(root) };
}

#[test]
fn error_reports_undefined_contract() {
    // 注：SML 解析器对未闭合的引号/块/数组较为宽容（按 EOF 收尾），
    // 因此这里用「引用未定义契约」这种确定会失败的输入。
    let text = CString::new("@is NoSuchContract\nx: 1\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(root.is_null(), "契约未定义应返回 NULL");
    assert_eq!(err.code, SML_ERR_CONTRACT, "err.code 应为 SML_ERR_CONTRACT");

    let msg = unsafe { CStr::from_ptr(err.text.as_ptr()) }.to_str().unwrap();
    assert!(msg.contains("契约"), "错误信息应填充: {msg}");

    let src = unsafe { CStr::from_ptr(err.source.as_ptr()) }.to_str().unwrap();
    assert_eq!(src, "<string>", "文本入口的 source 应为 <string>");
}

#[test]
fn error_reports_unknown_feature() {
    let text = CString::new("@feature enable nonexistent-feature\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(root.is_null(), "未知特性应返回 NULL");
    assert_ne!(err.code, 0);

    let msg = unsafe { CStr::from_ptr(err.text.as_ptr()) }.to_str().unwrap();
    assert!(msg.contains("特性"), "错误信息应提示特性: {msg}");
}

/// 文件入口出错时，source 应回填文件名（jansson 的定位体验）。
#[test]
fn error_source_is_file_path() {
    let dir = std::env::temp_dir().join("swsml_cabi_err_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("bad.sml"), "@is NoSuchContract\nx: 1\n").unwrap();

    let path = CString::new(dir.join("bad.sml").to_string_lossy().as_ref()).unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_load_file(path.as_ptr(), 0, &mut err) };
    assert!(root.is_null());

    let src = unsafe { CStr::from_ptr(err.source.as_ptr()) }.to_str().unwrap();
    assert!(src.contains("bad.sml"), "source 应含文件名: {src}");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn load_file_expands_include() {
    let dir = std::env::temp_dir().join("swsml_cabi_include_test");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(dir.join("conf.d")).unwrap();
    std::fs::write(
        dir.join("conf.d").join("extra.sml"),
        "from_name: ops\nmonth_count: 12\n",
    )
    .unwrap();
    std::fs::write(
        dir.join("main.sml"),
        "include \"conf.d/extra.sml\"\nport: 8080\n",
    )
    .unwrap();

    let path = CString::new(dir.join("main.sml").to_string_lossy().as_ref()).unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_load_file(path.as_ptr(), 0, &mut err) };
    assert!(!root.is_null(), "load_file 应成功");

    let p = CString::new("from_name").unwrap();
    let s = unsafe { sml_str_in(root, p.as_ptr()) };
    assert!(!s.is_null(), "include 的字段应展开");
    assert_eq!(unsafe { CStr::from_ptr(s) }.to_str().unwrap(), "ops");
    unsafe { sml_free_str(s) };

    let p = CString::new("port").unwrap();
    let mut ok = 0;
    assert_eq!(unsafe { sml_int_in(root, p.as_ptr(), &mut ok) }, 8080);
    assert_eq!(ok, 1);

    unsafe { sml_free(root) };
    let _ = std::fs::remove_dir_all(&dir);
}

/// C-ABI 审计 #3a：多目标 include 在特性未启用时必须报错，而非静默当作普通行
/// 解析注入垃圾键（此前与 include/glob/regex 行为不一致，是唯一会静默损坏的）。
#[test]
fn multi_include_feature_off_is_error() {
    use std::io::Write;
    let dir = std::env::temp_dir().join("sml_multi_inc_off");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    // flags=3 => bit0(bareword-string)+bit1(include) 启用，bit8(multi-include) 禁用。
    // 注意：include 解析仅在「文件」入口（resolve_includes）生效，故用 sml_load_file 测试。
    std::fs::write(dir.join("main.sml"), "include \"a.sml\", \"b.sml\"\n").unwrap();
    let path = CString::new(dir.join("main.sml").to_str().unwrap()).unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_load_file(path.as_ptr(), 3, &mut err) };
    assert!(root.is_null(), "multi-include 禁用时解析应失败");
    assert_ne!(err.code, SML_OK, "应返回非零错误码");
    let msg = unsafe { CStr::from_ptr(err.text.as_ptr()) }.to_str().unwrap();
    assert!(
        msg.contains("multi-include"),
        "错误应指出缺少 multi-include 特性，实际: {msg:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// 对照：启用 multi-include 后，同样的指令不再报「特性缺失」（而是因文件不存在报错，
/// 证明特性门控本身已生效而非恒报错）。
#[test]
fn multi_include_feature_on_not_feature_error() {
    use std::io::Write;
    let dir = std::env::temp_dir().join("sml_multi_inc_on");
    let _ = std::fs::remove_dir_all(&dir);
    let _ = std::fs::create_dir_all(&dir);
    // 文档内声明启用 multi-include，且调用方 flags=258(bit1 include + bit8 multi-include)
    // 也允许它；effective = feats ∩ allowed 才会包含 multi-include（否则被基线 ∩ 掉）。
    std::fs::write(
        dir.join("main.sml"),
        "@feature enable multi-include\ninclude \"a.sml\", \"b.sml\"\n",
    )
    .unwrap();
    let path = CString::new(dir.join("main.sml").to_str().unwrap()).unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_load_file(path.as_ptr(), 258, &mut err) };
    assert!(root.is_null(), "文件不存在时仍应失败");
    let msg = unsafe { CStr::from_ptr(err.text.as_ptr()) }.to_str().unwrap();
    assert!(
        !msg.contains("multi-include"),
        "启用 multi-include 后不应再报特性缺失，实际: {msg:?}"
    );
    let _ = std::fs::remove_dir_all(&dir);
}

/// C-ABI 审计 #3b：正则 include 的回溯匹配器必须受步数上限约束，防止 ReDoS 挂死解析。
#[test]
fn regex_include_no_redos() {
    use std::time::Instant;
    // 构造灾难性回溯模式：多个 `a*` 叠加后接一个不匹配的 `b`，匹配由 a 组成的长串。
    let pat = "a*a*a*a*a*a*a*a*b";
    let text = "aaaaaaaaaaaaaaaaaaaaaaaa"; // 24 个 a，足以触发指数级回溯
    let re = sml::compile_regex(&format!("re:{pat}"));
    let start = Instant::now();
    let matched = sml::regex_matches(&re, text);
    let elapsed = start.elapsed();
    // 不应匹配（无 b），且必须在合理时间内返回（步数上限兜底）
    assert!(!matched, "无 b 不应匹配");
    assert!(
        elapsed.as_millis() < 2000,
        "正则匹配应在步数上限内返回，实际耗时 {:?}（疑似 ReDoS）",
        elapsed
    );
}

#[test]
fn dumps_roundtrip() {
    let text = CString::new("name: John\nage: 27\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null());

    let out = unsafe { sml_dumps(root, 0) };
    assert!(!out.is_null());
    let dumped = unsafe { CStr::from_ptr(out) }.to_str().unwrap().to_string();
    unsafe { sml_free_str(out) };

    // 序列化结果应能被再次解析
    let reparsed = sml::parse(&dumped);
    assert!(reparsed.is_ok(), "sml_dumps 输出应可再解析: {dumped}");

    unsafe { sml_free(root) };
}

#[test]
fn arrays_are_accessible() {
    let text = CString::new("tags: [ a b c ]\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null());

    let key = CString::new("tags").unwrap();
    let arr = unsafe { sml_get(root, key.as_ptr()) };
    assert_eq!(unsafe { sml_typeof(arr) }, SML_TYPE_ARRAY);
    assert_eq!(unsafe { sml_size(arr) }, 3);

    let e0 = unsafe { sml_at(arr, 0) };
    let s = unsafe { sml_str_dup(e0) };
    assert_eq!(unsafe { CStr::from_ptr(s) }.to_str().unwrap(), "a");
    unsafe { sml_free_str(s) };

    assert!(unsafe { sml_at(arr, 99) }.is_null(), "越界应返回 NULL");

    unsafe { sml_free(root) };
}

#[test]
fn null_input_is_safe() {
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(std::ptr::null(), 0, &mut err) };
    assert!(root.is_null(), "NULL 输入应安全返回 NULL");

    assert_eq!(unsafe { sml_typeof(std::ptr::null()) }, -1);
    assert_eq!(unsafe { sml_size(std::ptr::null()) }, 0);
    assert_eq!(unsafe { sml_int_value(std::ptr::null()) }, 0);
    assert_eq!(unsafe { sml_real_value(std::ptr::null()) }, 0.0);

    // NULL 安全释放
    unsafe { sml_free(std::ptr::null_mut()) };
    unsafe { sml_free_str(std::ptr::null_mut()) };
}
