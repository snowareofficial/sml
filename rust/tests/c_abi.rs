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

/// 借用指针不可释放：反复取值不应崩溃或破坏根节点。
#[test]
fn borrowed_pointers_are_stable() {
    let text = CString::new("a { b { c: deep } }\n").unwrap();
    let mut err = CSmlError::default();
    let root = unsafe { sml_loads(text.as_ptr(), 0, &mut err) };
    assert!(!root.is_null());

    let path = CString::new("a.b.c").unwrap();
    let n1 = unsafe { sml_get_path(root, path.as_ptr()) };
    let n2 = unsafe { sml_get_path(root, path.as_ptr()) };
    assert_eq!(n1, n2, "同一路径应返回同一借用地址");
    assert_eq!(unsafe { sml_int_value(n1) }, unsafe { sml_int_value(n2) });

    unsafe { sml_free(root) };
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
