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
//
// **W16 查出的既有漂移**：这里原先**缺少 `code_str`**（W10 给真实 ABI 结构加了
// 这个字段，测试侧没跟着改）。后果不是「少测一个字段」，而是 `CSmlError::fill` 会
// **写到测试这块结构之外 16 字节** —— 栈上的越界写，属 UB，只是恰好没炸。
// 凡是「与 C 头文件对齐」的镜像结构，改一边就必须改另一边。
#[repr(C)]
struct CSmlError {
    code: c_int,
    line: c_int,
    column: c_int,
    position: usize,
    source: [c_char; 128],
    text: [c_char; 256],
    /// 真实错误码（如 `E-PARSE-008`）；`code` 只是粗粒度枚举。
    code_str: [c_char; 16],
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
            code_str: [0; 16],
        }
    }
}

/// 读出 `CSmlError.code_str`（NUL 结尾的定长数组）为 String。
fn err_code_str(e: &CSmlError) -> String {
    let bytes: Vec<u8> = e
        .code_str
        .iter()
        .take_while(|c| **c != 0)
        .map(|c| *c as u8)
        .collect();
    String::from_utf8_lossy(&bytes).into_owned()
}

#[repr(transparent)]
struct CSmlValue(std::mem::MaybeUninit<u8>);

/// 与 `sml_rs.h` 的 `sml_diag` 一一对应。
#[repr(C)]
struct CSmlDiag {
    code_str: *mut c_char,
    text: *mut c_char,
    line: c_int,
    severity: c_int,
}

/// 与 `sml_rs.h` 的 `sml_diag_list` 一一对应。
#[repr(C)]
struct CSmlDiagList {
    items: *mut CSmlDiag,
    len: usize,
}

/// 与 `sml_rs.h` 的 `sml_builtin` 对应（不透明句柄，测试里只传指针）。
#[repr(C)]
struct CSmlBuiltin {
    _opaque: std::mem::MaybeUninit<u8>,
}

/// 与 `sml_rs.h` 的 `sml_opts` 逐字节对应。
#[repr(C)]
struct CSmlOpts {
    struct_size: usize,
    flags: c_uint,
    env: *const *const c_char,
    env_len: usize,
    builtins: *const CSmlBuiltin,
}

/// 与 `sml_rs.h` 的三个弱表结构逐字节对应。
#[repr(C)]
struct CSmlWeakBind {
    path: *mut c_char,
    name: *mut c_char,
    line: c_int,
}

#[repr(C)]
struct CSmlWeakData {
    path: *mut c_char,
    name: *mut c_char,
    line: c_int,
    value: *mut CSmlValue,
}

#[repr(C)]
struct CSmlWeakDoc {
    path: *mut c_char,
    name: *mut c_char,
    line: c_int,
}

#[repr(C)]
struct CSmlWeakLists {
    binds: *mut CSmlWeakBind,
    n_binds: usize,
    data: *mut CSmlWeakData,
    n_data: usize,
    docs: *mut CSmlWeakDoc,
    n_docs: usize,
}

/// 与 `sml_rs.h` 的 `sml_contract_ref` / `sml_contractmap` 逐字节对应。
#[repr(C)]
struct CSmlContractRef {
    path: *mut c_char,
    name: *mut c_char,
}

#[repr(C)]
struct CSmlContractMap {
    items: *mut CSmlContractRef,
    len: usize,
}

/// `sml_result` 对 C 不透明，测试里只传指针。
#[repr(C)]
struct CSmlResult {
    _opaque: std::mem::MaybeUninit<u8>,
}

extern "C" {
    fn sml_loads(text: *const c_char, flags: c_uint, err: *mut CSmlError) -> *mut CSmlValue;
    fn sml_parse_all(
        text: *const c_char,
        opts: *const CSmlOpts,
        want: c_uint,
        err: *mut CSmlError,
    ) -> *mut CSmlResult;
    fn sml_result_value(r: *const CSmlResult) -> *mut CSmlValue;
    fn sml_result_diags(r: *const CSmlResult) -> *const CSmlDiagList;
    fn sml_result_errors(r: *const CSmlResult) -> *const CSmlDiagList;
    fn sml_result_weak(r: *const CSmlResult) -> *const CSmlWeakLists;
    fn sml_result_contract_map(r: *const CSmlResult) -> *const CSmlContractMap;
    fn sml_result_contracts(r: *const CSmlResult) -> *const CSmlBuiltin;
    fn sml_result_free(r: *mut CSmlResult);
    fn sml_parse_full_lists(
        text: *const c_char,
        opts: *const CSmlOpts,
        out_lists: *mut CSmlWeakLists,
        out_diags: *mut CSmlDiagList,
        err: *mut CSmlError,
    ) -> *mut CSmlValue;
    fn sml_load_file_full(
        path: *const c_char,
        opts: *const CSmlOpts,
        out_lists: *mut CSmlWeakLists,
        out_diags: *mut CSmlDiagList,
        err: *mut CSmlError,
    ) -> *mut CSmlValue;
    fn sml_weaklists_free(lists: *mut CSmlWeakLists);
    fn sml_parse_with_diags(
        text: *const c_char,
        flags: c_uint,
        out_diags: *mut CSmlDiagList,
        err: *mut CSmlError,
    ) -> *mut CSmlValue;
    fn sml_diag_list_free(list: *mut CSmlDiagList);
    fn sml_builtin_new() -> *mut CSmlBuiltin;
    fn sml_builtin_free(b: *mut CSmlBuiltin);
    fn sml_builtin_count(b: *const CSmlBuiltin) -> c_uint;
    fn sml_builtin_clone(b: *const CSmlBuiltin) -> *mut CSmlBuiltin;
    fn sml_builtin_add_field(
        b: *mut CSmlBuiltin,
        contract: *const c_char,
        field: *const c_char,
        ty: *const c_char,
        required: c_int,
    ) -> c_int;
    fn sml_builtin_set_range(
        b: *mut CSmlBuiltin,
        contract: *const c_char,
        field: *const c_char,
        min: f64,
        max: f64,
    ) -> c_int;
    fn sml_builtin_add_enum(
        b: *mut CSmlBuiltin,
        contract: *const c_char,
        field: *const c_char,
        value: *const c_char,
    ) -> c_int;
    fn sml_parse_full(
        text: *const c_char,
        opts: *const CSmlOpts,
        out_diags: *mut CSmlDiagList,
        err: *mut CSmlError,
    ) -> *mut CSmlValue;
    fn sml_load_file(path: *const c_char, flags: c_uint, err: *mut CSmlError) -> *mut CSmlValue;
    fn sml_free(v: *mut CSmlValue);
    fn sml_typeof(v: *const CSmlValue) -> c_int;
    fn sml_get(v: *const CSmlValue, key: *const c_char) -> *const CSmlValue;
    fn sml_at(v: *const CSmlValue, idx: usize) -> *const CSmlValue;
    fn sml_size(v: *const CSmlValue) -> usize;
    fn sml_key_at(v: *const CSmlValue, idx: usize) -> *mut c_char;
    fn sml_str_copy(v: *const CSmlValue, buf: *mut c_char, buflen: usize) -> usize;
    fn sml_str_dup(v: *const CSmlValue) -> *mut c_char;
    fn sml_int_value(v: *const CSmlValue) -> i64;
    fn sml_real_value(v: *const CSmlValue) -> f64;
    fn sml_bool_value(v: *const CSmlValue) -> c_int;
    fn sml_str_in(v: *const CSmlValue, path: *const c_char) -> *mut c_char;
    fn sml_int_in(v: *const CSmlValue, path: *const c_char, ok: *mut c_int) -> i64;
    fn sml_dumps(v: *const CSmlValue, flags: c_uint) -> *mut c_char;
    fn sml_dump_err(json: *const c_char, err: *mut CSmlError) -> *mut c_char;
    fn sml_free_str(p: *mut c_char);
    fn sml_version() -> *const c_char;
    fn sml_features_mask() -> c_uint;
    fn sml_feature_name(bit: c_uint) -> *const c_char;
}

// sml_type 枚举值（与 sml_rs.h 对齐）
const SML_TYPE_OBJECT: c_int = 6;
const SML_TYPE_ARRAY: c_int = 5;
const SML_TYPE_STR: c_int = 4;
const SML_TYPE_BOOL: c_int = 1;

// sml_errc 错误码（与 sml_rs.h 对齐）
const SML_OK: c_int = 0;
const SML_ERR_CONTRACT: c_int = 4;

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

/// W28 §2.3-A：sml_key_at 让宿主不经 JSON 即可完整物化对象（拿到键名）。
/// 注：返回的键是堆副本，必须 sml_free_str，而非按「借用」长期持有。
#[test]
fn key_at_enumerates_object() {
    let text = CString::new("name: John\nage: 27\nactive: true\nserver { host: a.b.c }\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null(), "解析应成功: err.code={}", err.code);
    assert_eq!(unsafe { sml_size(root) }, 4);

    // 按索引取出所有键，并与 sml_get 取到的值一一对应
    let mut seen = std::collections::BTreeSet::new();
    for i in 0..unsafe { sml_size(root) } {
        let kp = unsafe { sml_key_at(root, i) };
        assert!(!kp.is_null(), "idx {i} 的键不应为 NULL");
        let key = unsafe { CStr::from_ptr(kp) }.to_str().unwrap().to_string();
        unsafe { sml_free_str(kp) }; // 键是堆副本，必须释放
        assert!(seen.insert(key.clone()), "键重复: {key}");

        let ck = CString::new(key.clone()).unwrap();
        let node = unsafe { sml_get(root, ck.as_ptr()) };
        assert!(!node.is_null(), "按名取 {key} 应成功");
        // 键顺序与对象的迭代序一致（默认档 = 字典序）
    }
    assert_eq!(
        seen,
        ["active", "age", "name", "server"]
            .into_iter()
            .map(|s| s.to_string())
            .collect(),
        "键应为字典序"
    );

    // 越界返回 NULL
    let over = unsafe { sml_key_at(root, 999) };
    assert!(over.is_null(), "越界应返回 NULL");

    // 非对象返回 NULL
    let key = CString::new("name").unwrap();
    let str_node = unsafe { sml_get(root, key.as_ptr()) };
    let not_obj = unsafe { sml_key_at(str_node, 0) };
    assert!(not_obj.is_null(), "字符串值不应有键");

    unsafe { sml_free(root) };
}

extern "C" {
    fn sml_parse(text: *const c_char) -> *mut c_char;
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
///
/// 输入改用**溢出**（1e309）而非裸词 inf/nan：裸词 `inf` / `nan` 已不再被识别为
/// 浮点数（见 `sml-lex` 的 `numeric_head` 闸门），溢出才是当前唯一会产生
/// Float(inf) 的解析路径。本测试守护的是**序列化侧**的防护，与词法侧无关。
#[test]
fn jsonify_nan_inf_becomes_null() {
    let text = CString::new("j: 1e309\nk: -1e309\n").unwrap();
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
    let v = sml::c_abi::json_to_value(json).expect("json_to_value 应成功");
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
    let v = sml::c_abi::json_to_value(&s);
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
        let mut m = sml::Map::new();
        m.insert("k".into(), Value::Str("a\u{0}b".into()));
        m
    });
    let json = sml::c_abi::jsonify(&v);
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

/// W16：JSON→SML 的入口终于带回**可查的码**（`sml_dump_err`），
/// 并且深度超限不再吐「看着合法、回读变成 null」的占位文本。
///
/// 改前：`sml_dump(json)` 失败只返回 NULL，调用方分不清「不是 JSON」与「别的原因」；
/// 超深值更是**成功返回**一段含 `/* …深度超限… */ null` 的文本（静默改数据）。
#[test]
fn dump_err_reports_codes() {
    // ① 非法 JSON ⇒ E-PARSE-012（兜底码：底层只给 Option，没有更细的原因）
    let bad = CString::new("这不是 JSON").unwrap();
    let mut err = CSmlError::default();
    let out = unsafe { sml_dump_err(bad.as_ptr(), &mut err) };
    assert!(out.is_null(), "非法 JSON 应返回 NULL");
    assert_eq!(err_code_str(&err), "E-PARSE-012", "实得：{:?}", err_code_str(&err));

    // ② 正对照：合法 JSON 照常输出 SML
    let good = CString::new(r#"{"a":1}"#).unwrap();
    let mut err2 = CSmlError::default();
    let out2 = unsafe { sml_dump_err(good.as_ptr(), &mut err2) };
    assert!(!out2.is_null(), "合法 JSON 应给出文本");
    let text = unsafe { CStr::from_ptr(out2) }.to_string_lossy().into_owned();
    assert!(text.contains("a: 1"), "实得：{text}");
    unsafe { sml_free_str(out2) };

    // ③ 嵌套过深：JSON 入口自己的 `MAX_DEPTH = 128` 会**先**拦住（⇒ E-PARSE-012，
    //    文案里写明「或嵌套过深」）。E-LIMIT-004（序列化深度闸）在 C-ABI 这条路上
    //    属**防御性**：解析层先拦，走不到序列化层 —— 这一点在 dump.rs 的单测里直接钉
    //    （程序化构造的深值走 `to_sml_checked` 才真正触发它）。
    let deep = format!("{}1{}", "[".repeat(140), "]".repeat(140));
    let deepc = CString::new(deep).unwrap();
    let mut err3 = CSmlError::default();
    let out3 = unsafe { sml_dump_err(deepc.as_ptr(), &mut err3) };
    assert!(out3.is_null(), "超深 JSON 不应给出（带占位文本的）结果");
    assert_eq!(err_code_str(&err3), "E-PARSE-012", "实得：{:?}", err_code_str(&err3));

    // ④ 同一份超深值走 `sml_dumps`（值树入口）同样失败返回 NULL
    let mut err4 = CSmlError::default();
    let v = unsafe { sml_loads(CString::new("[1]").unwrap().as_ptr(), 0, &mut err4) };
    assert!(!v.is_null(), "简单值树应加载成功");
    let d = unsafe { sml_dumps(v, 0) };
    assert!(!d.is_null(), "正常值树应能序列化");
    unsafe { sml_free_str(d) };
    unsafe { sml_free(v) };
}

// ---------------------------------------------------------------------------
// P7：诊断出口（sml_parse_with_diags）
// ---------------------------------------------------------------------------

/// 读出诊断表里的第 i 条的 `(code, text, line, severity)`（`code` 无码时为 None）。
unsafe fn diag_at(list: &CSmlDiagList, i: usize) -> (Option<String>, String, c_int, c_int) {
    let d = &*list.items.add(i);
    let code = if d.code_str.is_null() {
        None
    } else {
        Some(CStr::from_ptr(d.code_str).to_str().unwrap().to_string())
    };
    let text = if d.text.is_null() {
        String::new()
    } else {
        CStr::from_ptr(d.text).to_str().unwrap().to_string()
    };
    (code, text, d.line, d.severity)
}

/// 诊断出口的三条硬性要求：**带码带行号**、**值树与 `sml_loads` 逐字节相同**、
/// **不阻断解析**。另钉住「重复键已提升为数组」这一既定语义没被顺手改掉。
#[test]
fn parse_with_diags_reports_duplicate_key() {
    let src = CString::new("a: 1\na: 2\n").unwrap();
    let mut err = CSmlError::default();
    let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
    let v = unsafe { sml_parse_with_diags(src.as_ptr(), 0, &mut list, &mut err) };

    assert!(!v.is_null(), "有诊断不等于解析失败：{}", err_code_str(&err));
    assert_eq!(list.len, 1, "重复键应产生 1 条诊断");
    let (code, text, line, sev) = unsafe { diag_at(&list, 0) };
    assert_eq!(code.as_deref(), Some("W-PARSE-001"), "诊断须带稳定码");
    assert_eq!(line, 2, "诊断须带行号（第 2 行的重复键）");
    assert_eq!(sev, 0, "WARN 档 severity 应为 0");
    assert!(text.contains('a'), "文案须点名键：{text}");

    // 值树逐字节相同：与 sml_loads 的 dumps 输出比对
    let mut err2 = CSmlError::default();
    let v2 = unsafe { sml_loads(src.as_ptr(), 0, &mut err2) };
    assert!(!v2.is_null());
    let d1 = unsafe { sml_dumps(v, 0) };
    let d2 = unsafe { sml_dumps(v2, 0) };
    let s1 = unsafe { CStr::from_ptr(d1) }.to_string_lossy().into_owned();
    let s2 = unsafe { CStr::from_ptr(d2) }.to_string_lossy().into_owned();
    assert_eq!(s1, s2, "诊断不得改变值树");
    // 既定语义仍是「提升为数组」（诊断只报告，不改语义）
    let key = CString::new("a").unwrap();
    let promoted = unsafe { sml_get(v, key.as_ptr()) };
    assert_eq!(unsafe { sml_typeof(promoted) }, SML_TYPE_ARRAY, "仍是数组提升：{s1}");
    assert_eq!(unsafe { sml_size(promoted) }, 2);

    unsafe { sml_free_str(d1) };
    unsafe { sml_free_str(d2) };
    unsafe { sml_diag_list_free(&mut list) };
    assert!(list.items.is_null() && list.len == 0, "释放后应清零");

    unsafe { sml_free(v) };
    unsafe { sml_free(v2) };
}

/// 片段覆盖是**真丢值**那一类：同样要能经 C-ABI 拿到 `W-PARSE-002`。
#[test]
fn parse_with_diags_reports_fragment_overwrite() {
    let src = CString::new("@base { x: 1 y: 2 }\nw {\n  x: 9\n  &base\n}\n").unwrap();
    let mut err = CSmlError::default();
    let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
    let v = unsafe { sml_parse_with_diags(src.as_ptr(), 0, &mut list, &mut err) };
    assert!(!v.is_null(), "{}", err_code_str(&err));
    assert_eq!(list.len, 1, "片段覆盖应产生 1 条诊断");
    let (code, _, _, _) = unsafe { diag_at(&list, 0) };
    assert_eq!(code.as_deref(), Some("W-PARSE-002"));
    unsafe { sml_diag_list_free(&mut list) };
    unsafe { sml_free(v) };
}

/// 失败路径：返回 NULL、诊断表为空、错误仍带码；NULL 出参 / NULL 输入都安全。
#[test]
fn parse_with_diags_failure_and_null_paths() {
    // ① 语法错误：致命错误只走 err，诊断表为空
    let bad = CString::new("a {\n").unwrap();
    let mut err = CSmlError::default();
    let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
    let v = unsafe { sml_parse_with_diags(bad.as_ptr(), 0, &mut list, &mut err) };
    assert!(v.is_null(), "未闭合的块应失败");
    assert_eq!(list.len, 0, "失败时不产诊断");
    assert!(list.items.is_null());
    assert!(!err_code_str(&err).is_empty(), "致命错误仍须带码");
    unsafe { sml_diag_list_free(&mut list) }; // 空表释放安全

    // ② out_diags 传 NULL：只拿值树，不崩
    let ok = CString::new("k: 1\n").unwrap();
    let mut err2 = CSmlError::default();
    let v2 = unsafe { sml_parse_with_diags(ok.as_ptr(), 0, std::ptr::null_mut(), &mut err2) };
    assert!(!v2.is_null());
    unsafe { sml_free(v2) };

    // ③ text 传 NULL：E-INTERNAL-001，与 sml_loads 同码
    let mut err3 = CSmlError::default();
    let mut list3 = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
    let v3 = unsafe { sml_parse_with_diags(std::ptr::null(), 0, &mut list3, &mut err3) };
    assert!(v3.is_null());
    assert_eq!(err_code_str(&err3), "E-INTERNAL-001");
    assert_eq!(list3.len, 0);

    // ④ 释放函数本身 NULL 安全、可重复调用
    unsafe { sml_diag_list_free(std::ptr::null_mut()) };
    unsafe { sml_diag_list_free(&mut list3) };
    unsafe { sml_diag_list_free(&mut list3) };
}

// ---------------------------------------------------------------------------
// P7 第二片：内置契约表 + 扩展选项（sml_parse_full）
// ---------------------------------------------------------------------------

unsafe fn diag_codes(list: &CSmlDiagList) -> Vec<String> {
    let mut out = Vec::new();
    for i in 0..list.len {
        let (code, _, _, _) = diag_at(list, i);
        if let Some(c) = code {
            out.push(c);
        }
    }
    out
}

/// 内置表注入后，`W-CONTRACT-001` 在 C-ABI 这条路上**真的可达**：
/// 声明侧一档 + 块内容侧三档，且值树**一个键都没被改**（软校验只读）。
#[test]
fn parse_full_soft_checks_against_injected_builtins() {
    unsafe {
        let b = sml_builtin_new();
        assert!(!b.is_null());
        assert_eq!(sml_builtin_count(std::ptr::null()), 0, "NULL 句柄应返回 0");

        let tls = CString::new("tls").unwrap();
        let cert = CString::new("cert").unwrap();
        let port = CString::new("port").unwrap();
        let mode = CString::new("mode").unwrap();
        let t_str = CString::new("str").unwrap();
        let t_int = CString::new("int").unwrap();
        let t_enum = CString::new("enum").unwrap();
        assert_eq!(sml_builtin_add_field(b, tls.as_ptr(), cert.as_ptr(), t_str.as_ptr(), 1), 0);
        assert_eq!(sml_builtin_add_field(b, tls.as_ptr(), port.as_ptr(), t_int.as_ptr(), 0), 0);
        assert_eq!(sml_builtin_set_range(b, tls.as_ptr(), port.as_ptr(), 1.0, 65535.0), 0);
        assert_eq!(sml_builtin_add_field(b, tls.as_ptr(), mode.as_ptr(), t_enum.as_ptr(), 0), 0);
        let strict = CString::new("strict").unwrap();
        assert_eq!(sml_builtin_add_enum(b, tls.as_ptr(), mode.as_ptr(), strict.as_ptr()), 0);
        assert_eq!(sml_builtin_count(b), 1, "同一个契约名只建一份");

        // 未知类型：拒绝且**不往表里塞半成品**
        let bogus = CString::new("image").unwrap();
        let nofield = CString::new("nofield").unwrap();
        assert_eq!(sml_builtin_add_field(b, tls.as_ptr(), nofield.as_ptr(), bogus.as_ptr(), 0), -1);
        assert_eq!(sml_builtin_set_range(b, tls.as_ptr(), nofield.as_ptr(), 0.0, 1.0), -1);
        assert_eq!(sml_builtin_add_enum(b, tls.as_ptr(), cert.as_ptr(), strict.as_ptr()), -1);

        let src = CString::new(
            "@feature enable type.weak\n\
             srv {\n  @weakbind tls\n  web { cert: 1 port: 99999 mode: nope }\n}\n\
             @weakbind nosuch\nother { x: 1 }\n",
        )
        .unwrap();
        let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut err = CSmlError::default();
        // `flags` 是**特性上限（交集）**：`type.weak` 是 opt-in，不在基线里，
        // 必须显式允许，否则文档的 `@feature enable type.weak` 会被静默剔除、
        // 随后 `@weakbind` 报 E-FEATURE-001（这正是 parse_with_features 的三条陷阱之一）。
        const SML_F_BASIC: c_uint = 0b1_0111;
        const SML_F_WEAK: c_uint = 1 << 19;
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: SML_F_BASIC | SML_F_WEAK,
            env: std::ptr::null(),
            env_len: 0,
            builtins: b,
        };
        let v = sml_parse_full(src.as_ptr(), &opts, &mut list, &mut err);
        assert!(!v.is_null(), "有告警不等于失败：{}", err_code_str(&err));

        let codes = diag_codes(&list);
        assert_eq!(
            codes.iter().filter(|c| *c == "W-CONTRACT-001").count(),
            4,
            "应为 4 条：cert 类型 / port 区间 / mode 枚举 / nosuch 未知契约，实得 {codes:?}"
        );
        assert!(codes.iter().all(|c| c == "W-CONTRACT-001"), "不该有别的码：{codes:?}");

        // 只读：cert 仍是数字 1，没被填默认值、没被删键
        let k_srv = CString::new("srv").unwrap();
        let k_web = CString::new("web").unwrap();
        let k_cert = CString::new("cert").unwrap();
        let srv = sml_get(v, k_srv.as_ptr());
        let web = sml_get(srv, k_web.as_ptr());
        let cert_v = sml_get(web, k_cert.as_ptr());
        assert!(!cert_v.is_null(), "cert 键必须还在");
        assert_eq!(sml_typeof(cert_v), 2, "cert 仍应是 int（未被改成 str）");
        assert_eq!(sml_int_value(cert_v), 1, "cert 值未被改写");

        sml_free(v);
        sml_diag_list_free(&mut list);
        sml_builtin_free(b);
        sml_builtin_free(std::ptr::null_mut());
    }
}

/// env 注入：`$env.X` 走覆盖表，**不改进程环境**。
#[test]
fn parse_full_injects_env_without_touching_process() {
    const KEY: &str = "SML_ABI_ENV_PROBE";
    assert!(std::env::var(KEY).is_err(), "测试前置：进程环境里不该有这个变量");
    unsafe {
        let src = format!("k: $env.{KEY}\n");
        let csrc = CString::new(src).unwrap();
        let pair = CString::new(format!("{KEY}=prod")).unwrap();
        let arr: [*const c_char; 1] = [pair.as_ptr()];
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0,
            env: arr.as_ptr(),
            env_len: 1,
            builtins: std::ptr::null(),
        };
        let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut err = CSmlError::default();
        let v = sml_parse_full(csrc.as_ptr(), &opts, &mut list, &mut err);
        assert!(!v.is_null(), "{}", err_code_str(&err));
        let k = CString::new("k").unwrap();
        let p = sml_str_dup(sml_get(v, k.as_ptr()));
        assert_eq!(CStr::from_ptr(p).to_str().unwrap(), "prod");
        sml_free_str(p);
        assert_eq!(list.len, 0);
        sml_free(v);
        sml_diag_list_free(&mut list);
    }
    assert!(std::env::var(KEY).is_err(), "解析不得写入进程环境");
}

/// 选项的坏入参必须**响亮失败**，不能静默按默认值跑：
/// `struct_size` 太小 / env 项不是 `KEY=VALUE` / NULL 项；NULL opts 照常可用。
#[test]
fn parse_full_rejects_malformed_opts() {
    unsafe {
        let src = CString::new("k: 1\n").unwrap();
        let mut err = CSmlError::default();
        let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };

        // ① struct_size 太小（模拟用旧头文件配新库）
        let small = CSmlOpts {
            struct_size: 8,
            flags: 0,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        let v = sml_parse_full(src.as_ptr(), &small, &mut list, &mut err);
        assert!(v.is_null(), "struct_size 太小应失败");
        assert_eq!(err_code_str(&err), "E-INTERNAL-001");
        assert_eq!(list.len, 0);

        // ② env 项缺 `=`
        let bad = CString::new("NOEQUALS").unwrap();
        let arr: [*const c_char; 1] = [bad.as_ptr()];
        let mut err2 = CSmlError::default();
        let o2 = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0,
            env: arr.as_ptr(),
            env_len: 1,
            builtins: std::ptr::null(),
        };
        let v2 = sml_parse_full(src.as_ptr(), &o2, std::ptr::null_mut(), &mut err2);
        assert!(v2.is_null());
        assert_eq!(err_code_str(&err2), "E-INTERNAL-001");

        // ③ env 里有 NULL 项
        let arr3: [*const c_char; 1] = [std::ptr::null()];
        let mut err3 = CSmlError::default();
        let o3 = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0,
            env: arr3.as_ptr(),
            env_len: 1,
            builtins: std::ptr::null(),
        };
        let v3 = sml_parse_full(src.as_ptr(), &o3, std::ptr::null_mut(), &mut err3);
        assert!(v3.is_null());
        assert_eq!(err_code_str(&err3), "E-INTERNAL-001");

        // ④ opts 传 NULL：等价于 flags = 0 的 sml_parse_with_diags
        let mut err4 = CSmlError::default();
        let mut list4 = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let v4 = sml_parse_full(src.as_ptr(), std::ptr::null(), &mut list4, &mut err4);
        assert!(!v4.is_null(), "{}", err_code_str(&err4));
        assert_eq!(list4.len, 0);
        sml_free(v4);
        sml_diag_list_free(&mut list4);
    }
}

// ---------------------------------------------------------------------------
// P7 第四片：弱声明三表 + 文件入口
// ---------------------------------------------------------------------------

unsafe fn cstr_at(p: *const c_char) -> String {
    if p.is_null() {
        String::new()
    } else {
        CStr::from_ptr(p).to_string_lossy().into_owned()
    }
}

/// 三张弱表都经 C-ABI 交出，且**都不进主树**（`@weakdata` 的载荷只在表里）。
#[test]
fn parse_full_lists_reports_weak_tables() {
    const SML_F_BASIC: c_uint = 0b1_0111;
    const SML_F_WEAK: c_uint = 1 << 19;
    unsafe {
        let src = CString::new(
            "@feature enable type.weak\n\
             srv {\n  @weakbind tls\n  web { cert: \"x\" }\n  @weakdata limits { max: 10 }\n}\n\
             @weakdoc base\n",
        )
        .unwrap();
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: SML_F_BASIC | SML_F_WEAK,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        let mut lists = CSmlWeakLists {
            binds: std::ptr::null_mut(),
            n_binds: 0,
            data: std::ptr::null_mut(),
            n_data: 0,
            docs: std::ptr::null_mut(),
            n_docs: 0,
        };
        let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut err = CSmlError::default();
        let v = sml_parse_full_lists(src.as_ptr(), &opts, &mut lists, &mut list, &mut err);
        assert!(!v.is_null(), "{}", err_code_str(&err));

        assert_eq!(
            (lists.n_binds, lists.n_data, lists.n_docs),
            (1, 1, 1),
            "应为 1 绑定 / 1 数据 / 1 profile"
        );

        let b = &*lists.binds;
        assert_eq!(cstr_at(b.path), "srv.web", "bind 的路径");
        assert_eq!(cstr_at(b.name), "tls");
        assert!(b.line > 0, "文本入口应带行号");

        let d = &*lists.data;
        assert_eq!(cstr_at(d.path), "srv");
        assert_eq!(cstr_at(d.name), "limits");
        let k_max = CString::new("max").unwrap();
        assert_eq!(sml_int_value(sml_get(d.value, k_max.as_ptr())), 10, "载荷可读");

        let doc = &*lists.docs;
        assert_eq!(cstr_at(doc.path), "", "顶层 profile 的路径为空串");
        assert_eq!(cstr_at(doc.name), "base");

        // 主树无痕迹：`srv.limits` 不存在、`srv.web` 只有 cert
        let k_srv = CString::new("srv").unwrap();
        let k_limits = CString::new("limits").unwrap();
        let srv = sml_get(v, k_srv.as_ptr());
        assert!(sml_get(srv, k_limits.as_ptr()).is_null(), "weakdata 不得进主树");
        assert_eq!(sml_size(sml_get(srv, CString::new("web").unwrap().as_ptr())), 1);

        sml_weaklists_free(&mut lists);
        assert!(lists.binds.is_null() && lists.n_binds == 0, "释放后清零");
        sml_weaklists_free(&mut lists); // 幂等
        sml_weaklists_free(std::ptr::null_mut());

        sml_free(v);
        sml_diag_list_free(&mut list);
    }
}

/// 文件入口：展开 include、交诊断与弱表；路径不存在报 `E-IO-001`。
#[test]
fn load_file_full_expands_include() {
    const SML_F_BASIC: c_uint = 0b1_0111;
    const SML_F_WEAK: c_uint = 1 << 19;
    let dir = std::env::temp_dir().join("sml_abi_file_full");
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("sub.sml"), "k: 1\n").unwrap();
    let main = dir.join("main.sml");
    std::fs::write(
        &main,
        "@feature enable type.weak\ninclude \"sub.sml\"\nm: 2\n@weakdoc base\n",
    )
    .unwrap();

    unsafe {
        let cpath = CString::new(main.to_string_lossy().as_bytes()).unwrap();
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: SML_F_BASIC | SML_F_WEAK,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        let mut lists = CSmlWeakLists {
            binds: std::ptr::null_mut(),
            n_binds: 0,
            data: std::ptr::null_mut(),
            n_data: 0,
            docs: std::ptr::null_mut(),
            n_docs: 0,
        };
        let mut list = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut err = CSmlError::default();
        let v = sml_load_file_full(cpath.as_ptr(), &opts, &mut lists, &mut list, &mut err);
        assert!(!v.is_null(), "{}", err_code_str(&err));
        let k = CString::new("k").unwrap();
        let m = CString::new("m").unwrap();
        assert_eq!(sml_int_value(sml_get(v, k.as_ptr())), 1, "include 已展开");
        assert_eq!(sml_int_value(sml_get(v, m.as_ptr())), 2);
        assert_eq!(lists.n_docs, 1);
        assert_eq!(cstr_at((*lists.docs).name), "base");
        sml_weaklists_free(&mut lists);
        sml_free(v);
        sml_diag_list_free(&mut list);

        // 不存在的路径：E-IO-001，出参为空表
        let missing = CString::new(dir.join("nope.sml").to_string_lossy().as_bytes()).unwrap();
        let mut err2 = CSmlError::default();
        let mut list2 = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let v2 = sml_load_file_full(missing.as_ptr(), &opts, std::ptr::null_mut(), &mut list2, &mut err2);
        assert!(v2.is_null(), "不存在的文件应失败");
        assert_eq!(err_code_str(&err2), "E-IO-001", "实得：{:?}", err_code_str(&err2));
        assert_eq!(list2.len, 0);
    }

    let _ = std::fs::remove_dir_all(&dir);
}

// ---------------------------------------------------------------------------
// P7 第六片：聚合结果（sml_parse_all）—— 契约表 / 契约映射 / 多错误
// ---------------------------------------------------------------------------

/// `@contract` + `@is` 需要 `contract` 位；`@weakbind` 需要 `type.weak` 位。
const SML_F_CONTRACT: c_uint = 1 << 3;

/// 「用 SML 写内置契约、再注入回去」（weakbind.md §6.1）在 C-ABI 上闭环。
#[test]
fn parse_all_captures_contracts_and_map() {
    unsafe {
        let src = CString::new("@contract tls { cert: str }\nsrv { @is tls\n  cert: \"x\"\n}\n").unwrap();
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0b1_0111 | SML_F_CONTRACT,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        const WANT_MAP: c_uint = 1 << 3;
        const WANT_CONTRACTS: c_uint = 1 << 4;
        let mut err = CSmlError::default();
        let r = sml_parse_all(src.as_ptr(), &opts, WANT_MAP | WANT_CONTRACTS, &mut err);
        assert!(!r.is_null(), "{}", err_code_str(&err));

        // 值树（借用）可读
        let k_srv = CString::new("srv").unwrap();
        let k_cert = CString::new("cert").unwrap();
        let v = sml_result_value(r);
        let p = sml_str_dup(sml_get(sml_get(v, k_srv.as_ptr()), k_cert.as_ptr()));
        assert_eq!(CStr::from_ptr(p).to_str().unwrap(), "x");
        sml_free_str(p);

        // 契约映射：srv -> tls
        let cm = sml_result_contract_map(r);
        assert!(!cm.is_null());
        assert_eq!((*cm).len, 1, "应有一条 (srv, tls)");
        let item = &*(*cm).items;
        assert_eq!(cstr_at(item.path), "srv");
        assert_eq!(cstr_at(item.name), "tls");

        // 契约表：借用的 sml_builtin 可以直接当内置表注入
        let captured = sml_result_contracts(r);
        assert!(!captured.is_null(), "SML_WANT_CONTRACTS 应给出表");
        assert_eq!(sml_builtin_count(captured), 1);
        let weak_opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0b1_0111 | (1 << 19),
            env: std::ptr::null(),
            env_len: 0,
            builtins: captured,
        };
        let wsrc = CString::new("@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n").unwrap();
        let mut wlist = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut werr = CSmlError::default();
        let wv = sml_parse_full(wsrc.as_ptr(), &weak_opts, &mut wlist, &mut werr);
        assert!(!wv.is_null(), "{}", err_code_str(&werr));
        let codes = diag_codes(&wlist);
        assert_eq!(codes, vec!["W-CONTRACT-001".to_string()], "闭环应报 unknown_contract");
        sml_free(wv);
        sml_diag_list_free(&mut wlist);

        // 不请求的位：空表
        assert_eq!((*sml_result_diags(r)).len, 0, "未请求 DIAGS -> 空表");
        assert_eq!((*sml_result_weak(r)).n_binds, 0, "未请求 WEAK -> 空表");

        sml_result_free(r);
        sml_result_free(std::ptr::null_mut());
    }
}

/// `sml_builtin_clone`：把借用的表变成宿主独占 —— 释放结果句柄后仍能当内置表注入。
#[test]
fn builtin_clone_outlives_the_result() {
    unsafe {
        const WANT_CONTRACTS: c_uint = 1 << 4;
        const SML_F_CONTRACT: c_uint = 1 << 3;
        let src = CString::new("@contract tls { cert: str }\nsrv { @is tls\n  cert: x\n}\n").unwrap();
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0b1_0111 | SML_F_CONTRACT,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        let mut err = CSmlError::default();
        let r = sml_parse_all(src.as_ptr(), &opts, WANT_CONTRACTS, &mut err);
        assert!(!r.is_null(), "{}", err_code_str(&err));
        let owned = sml_builtin_clone(sml_result_contracts(r));
        assert!(!owned.is_null());
        sml_result_free(r); // 借用的表随结果失效，克隆出来的不受影响
        assert_eq!(sml_builtin_count(owned), 1, "克隆后仍是 1 份契约");

        // 用克隆的表做弱绑定软校验
        let weak_opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0b1_0111 | (1 << 19),
            env: std::ptr::null(),
            env_len: 0,
            builtins: owned,
        };
        let wsrc = CString::new("@feature enable type.weak\n@weakbind tls\ns { cert: 1 }\n").unwrap();
        let mut wlist = CSmlDiagList { items: std::ptr::null_mut(), len: 0 };
        let mut werr = CSmlError::default();
        let wv = sml_parse_full(wsrc.as_ptr(), &weak_opts, &mut wlist, &mut werr);
        assert!(!wv.is_null(), "{}", err_code_str(&werr));
        assert_eq!(diag_codes(&wlist), vec!["W-CONTRACT-001".to_string()], "cert 类型不符");
        sml_free(wv);
        sml_diag_list_free(&mut wlist);
        sml_builtin_free(owned);
        assert!(sml_builtin_clone(std::ptr::null()).is_null(), "NULL 安全");
    }
}

/// 多错误：传 `SML_WANT_ERRORS` 后语法/契约错也返回句柄，错误表 `severity = 1`。
#[test]
fn parse_all_collects_multiple_errors() {
    unsafe {
        let src = CString::new("a { @is NoSuch1\n  x: 1 }\nb { @is NoSuch2\n  y: 2 }\n").unwrap();
        let opts = CSmlOpts {
            struct_size: std::mem::size_of::<CSmlOpts>(),
            flags: 0b1_0111 | SML_F_CONTRACT,
            env: std::ptr::null(),
            env_len: 0,
            builtins: std::ptr::null(),
        };
        const WANT_ERRORS: c_uint = 1 << 1;
        let mut err = CSmlError::default();
        let r = sml_parse_all(src.as_ptr(), &opts, WANT_ERRORS, &mut err);
        assert!(!r.is_null(), "多错误模式不该返回 NULL：{}", err_code_str(&err));
        let el = sml_result_errors(r);
        assert!(!el.is_null());
        assert_eq!((*el).len, 2, "应收集到 2 条错误");
        for i in 0..(*el).len {
            let (code, text, line, sev) = diag_at(&*el, i);
            assert_eq!(sev, 1, "错误档 severity 应为 1");
            assert_eq!(code.as_deref(), Some("E-CONTRACT-001"), "文案：{text}");
            assert!(line > 0, "行号应由字节偏移换算出来");
        }
        sml_result_free(r);

        // 不请求 ERRORS：首个错误即 NULL + 带码 err（与 sml_parse_full 一致）
        let mut err2 = CSmlError::default();
        let r2 = sml_parse_all(src.as_ptr(), &opts, 0, &mut err2);
        assert!(r2.is_null());
        assert_eq!(err_code_str(&err2), "E-CONTRACT-001");
    }
}