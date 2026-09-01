use crate::core::*;
#[cfg(feature = "sml")]
use crate::core::to_sml;
use crate::value::*;
use std::collections::BTreeMap;
// ---------------------------------------------------------------------------
// C-ABI (cdylib, 供 C / 其它语言调用)
// ---------------------------------------------------------------------------

use std::os::raw::{c_char, c_int};
use std::ptr;

/// 把 `&str` 编码为 NUL 结尾的 C 字符串。
///
/// 若 `s` 含内嵌 NUL（`CString::new` 失败），返回 `None` 而非静默回退为空串——
/// 调用方应据以返回 NULL 并（在可用处）置错误码，避免「拿到空结果却无错误」的
/// 数据完整性陷阱（见 C-ABI 审计报告 #2）。
fn cstr(s: &str) -> Option<*mut c_char> {
    match std::ffi::CString::new(s) {
        Ok(c) => Some(c.into_raw()),
        Err(_) => None,
    }
}

/// [`cstr`] 的便捷封装：失败时返回 `NULL`（与既有 `sml_parse` 等「失败返回 NULL」
/// 的约定一致）。调用处若持有 `err` 参数，应优先自行返回 NULL 并填充错误。
fn cstr_or_null(s: &str) -> *mut c_char {
    cstr(s).unwrap_or(ptr::null_mut())
}

/// sml_parse(text) -> 返回 JSON 字符串 (调用方 sml_free 释放); 失败返回 NULL
///
/// # Safety
/// `text` 必须是合法 NUL 结尾的 C 字符串，或 NULL（NULL 直接返回 NULL）。
/// 传入悬垂 / 非 NUL 结尾的指针是未定义行为。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse(text: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    match parse(&t) {
        Ok(v) => cstr_or_null(&jsonify(&v)),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_dump(json) -> 接受 JSON 字符串, 序列化为 SML; 调用方 sml_free
///
/// # Safety
/// `json` 必须是合法 NUL 结尾的 C 字符串，或 NULL（NULL 直接返回 NULL）。
/// 传入悬垂 / 非 NUL 结尾的指针是未定义行为。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dump(json: *const c_char) -> *mut c_char {
    if json.is_null() {
        return ptr::null_mut();
    }
    let j = unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned();
    #[cfg(feature = "sml")]
    {
        match json_to_value(&j) {
            Some(v) => cstr_or_null(&to_sml(&v)),
            None => ptr::null_mut(),
        }
    }
    #[cfg(not(feature = "sml"))]
    {
        let _ = j;
        ptr::null_mut()
    }
}

/// sml_free_str(p): 释放由 sml_parse / sml_dump / sml_dumps 等返回的字符串。
///
/// 注：早期版本此函数名为 `sml_free`，与 `sml.h`（纯 C 后端）的
/// `sml_free(sml_value*)` 语义冲突。为让两个后端心智模型一致
/// （`sml_free` 释放值树、`sml_free_str` 释放字符串），此处重命名。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_free_str(p: *mut c_char) {
    if !p.is_null() {
        drop(unsafe { std::ffi::CString::from_raw(p) });
    }
}

/// sml_version() -> 版本静态字符串（**无需释放**，与 jansson 的
/// `jansson_version_str()` 语义一致）。
///
/// 返回指向编译期常量的指针，生命周期为 `'static`。
/// 需要可释放的副本请用 [`sml_version_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_version() -> *const c_char {
    concat!("sml ", env!("CARGO_PKG_VERSION"), "\0").as_ptr() as *const c_char
}

// ---------------------------------------------------------------------------
// v3 扩展 ABI (与基础 sml_parse 并存, 不破坏既有符号)
// 暴露 $env 内联 / glob-include / @feature / @contract 等 v3 能力。
// 这些入口独立于 c/ 与 cpp/ 的纯 native 实现, 仅供需要完整 v3 功能的
// 调用方链接本 cdylib 使用。
// ---------------------------------------------------------------------------

/// 极简解析 opts JSON: 仅识别顶层 object 的
///   "features": [ "glob-include", ... ]
///   "env":      { "KEY": "VAL", ... }
///   "allow":    [ "v1", "v2", "v3" ]
/// 返回 (features: Vec<Feature>, env: Vec<(String,String)>, allow: Vec<Version>)。
/// 任何字段缺失即视为空/不限制; 解析失败返回 Err。
fn parse_opts_json(opts: &str) -> Result<(Vec<Feature>, Vec<(String, String)>, Vec<Version>), String> {
    let mut features: Vec<Feature> = Vec::new();
    let mut env: Vec<(String, String)> = Vec::new();
    let mut allow: Vec<Version> = Vec::new();
    if opts.trim().is_empty() {
        return Ok((features, env, allow));
    }
    // 手工 tokenizer: 仅支持本结构, 不引第三方依赖。
    let b = opts.as_bytes();
    let mut i = 0usize;
    let len = b.len();
    // 跳到首个 {
    while i < len && b[i] != b'{' { i += 1; }
    if i >= len { return Err("opts 不是 JSON object".into()); }
    i += 1; // 越过 {
    loop {
        // 跳空白与逗号
        while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
        if i >= len || b[i] == b'}' { break; }
        // 读 key (双引号字符串)
        if b[i] != b'"' { return Err("opts key 须为字符串".into()); }
        i += 1;
        let ks = i;
        while i < len && b[i] != b'"' { i += 1; }
        let key = std::str::from_utf8(&b[ks..i]).map_err(|_| "opts key 非法 UTF-8".to_string())?.to_string();
        i += 1; // 越过 "
        while i < len && (b[i] == b' ' || b[i] == b':' || b[i] == b'\t') { i += 1; }
        match key.as_str() {
            "features" | "allow" => {
                // 读数组 [ ... ]
                if i >= len || b[i] != b'[' { return Err(format!("opts.{key} 须为数组")); }
                i += 1;
                loop {
                    while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
                    if i < len && b[i] == b']' { i += 1; break; }
                    if i >= len || b[i] != b'"' { return Err(format!("opts.{key} 元素须为字符串")); }
                    i += 1;
                    let vs = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let val = std::str::from_utf8(&b[vs..i]).map_err(|_| "opts 值非法 UTF-8".to_string())?.to_string();
                    i += 1;
                    if key == "features" {
                        features.push(Feature::from_name(&val).ok_or_else(|| format!("未知特性 {val}"))?);
                    } else {
                        allow.push(Version::from_word(&val).ok_or_else(|| format!("未知版本 {val}"))?);
                    }
                }
            }
            "env" => {
                if i >= len || b[i] != b'{' { return Err("opts.env 须为 object".into()); }
                i += 1;
                loop {
                    while i < len && (b[i] == b' ' || b[i] == b'\t' || b[i] == b'\n' || b[i] == b'\r' || b[i] == b',') { i += 1; }
                    if i < len && b[i] == b'}' { i += 1; break; }
                    if i >= len || b[i] != b'"' { return Err("opts.env key 须为字符串".into()); }
                    i += 1;
                    let ks = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let ek = std::str::from_utf8(&b[ks..i]).map_err(|_| "opts.env key 非法".to_string())?.to_string();
                    i += 1;
                    while i < len && (b[i] == b' ' || b[i] == b':' || b[i] == b'\t') { i += 1; }
                    if i >= len || b[i] != b'"' { return Err("opts.env value 须为字符串".into()); }
                    i += 1;
                    let vs = i;
                    while i < len && b[i] != b'"' { i += 1; }
                    let ev = std::str::from_utf8(&b[vs..i]).map_err(|_| "opts.env value 非法".to_string())?.to_string();
                    i += 1;
                    env.push((ek, ev));
                }
            }
            _ => {
                // 跳过未知字段的值 (标量/数组/对象)
                let mut depth = 0i32;
                loop {
                    if i >= len { break; }
                    match b[i] {
                        b'"' => { i += 1; while i < len && b[i] != b'"' { if b[i] == b'\\' { i += 2; } else { i += 1; } } i += 1; }
                        b'{' | b'[' => { depth += 1; i += 1; }
                        b'}' | b']' => { depth -= 1; i += 1; if depth <= 0 { break; } }
                        _ => { i += 1; }
                    }
                }
            }
        }
    }
    Ok((features, env, allow))
}

/// sml_parse_ex(text, opts_json) -> JSON 字符串 (调用方 sml_free) 或 NULL。
///
/// opts_json 示例:
///   {"features":["glob-include","contract"],"env":{"APP_ENV":"prod"},"allow":["v1","v3"]}
/// - features: 调用方额外启用的特性 (与文档 @feature 取交集)。
/// - env:      环境变量**覆盖表**，供 `$env.X` 内联解析。仅作用于本次调用，
///             不写入进程环境（避免并发调用下 `set_var` 的数据竞争，以及
///             覆盖 `PATH` / `LD_PRELOAD` 等影响进程行为的变量）。
/// - allow:    限定文档声明的版本必须在此范围内; 空数组表示不限制。
/// 失败 (语法/版本/特性越权/文件找不到) 返回 NULL。
///
/// # Safety
/// `text` 必须为合法 NUL 结尾的 C 字符串或 NULL；`opts` 可为 NULL（表示不传选项），
/// 非 NULL 时同样必须是合法 NUL 结尾的 C 字符串。传入悬垂 / 非 NUL 结尾的指针
/// 是未定义行为。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_ex(text: *const c_char, opts: *const c_char) -> *mut c_char {
    if text.is_null() {
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let opts_str = if opts.is_null() {
        String::new()
    } else {
        unsafe { std::ffi::CStr::from_ptr(opts) }.to_string_lossy().into_owned()
    };
    let (feats, env, allow) = match parse_opts_json(&opts_str) {
        Ok(x) => x,
        Err(_) => return ptr::null_mut(),
    };
    // env 仅作为本次解析的覆盖表传入，不触碰进程环境：
    // 线程安全，且不会让文档通过覆盖 PATH / LD_PRELOAD 等变量影响宿主行为。
    let result = (|| {
        // 构造调用方允许特性集: 基础全集 并 上 opts 指定特性。
        let mut allowed = FeatureSet::all();
        for f in &feats {
            allowed = allowed.with(*f);
        }
        let env_map: BTreeMap<String, String> = env.into_iter().collect();
        let val = parse_with_features_env(&t, allowed, env_map).map(|(v, _)| v)?;
        if !allow.is_empty() {
            let declared = strip_version(&t).ok().and_then(|(_, d)| d);
            if let Some(d) = declared {
                if !allow.contains(&d) {
                    return Err(format!("文档声明版本 {} 不在 allow 范围", d.name()));
                }
            }
        }
        Ok(jsonify(&val))
    })();
    match result {
        Ok(s) => cstr_or_null(&s),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_parse_file(path) -> JSON 字符串 (调用方 sml_free) 或 NULL。
/// 桥接内部 parse_file: 自动处理 include / glob / @contract 校验, 带文件上下文。
///
/// # Safety
/// `path` 必须为合法 NUL 结尾的 C 字符串或 NULL（NULL 直接返回 NULL）。
/// 传入悬垂 / 非 NUL 结尾的指针是未定义行为。
///
/// 注：`include` 的路径遍历防护（canonicalize + starts_with）在 Rust 侧生效，
/// 传入任意路径都不会越出主文件所在目录。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_file(path: *const c_char) -> *mut c_char {
    if path.is_null() {
        return ptr::null_mut();
    }
    let p = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();
    match parse_file(&p) {
        Ok(v) => cstr_or_null(&jsonify(&v)),
        Err(_) => ptr::null_mut(),
    }
}

/// sml_features() -> 当前支持的特性名 JSON 数组 (调用方 sml_free)。
/// 例: ["include","env","contract","glob-include", ...]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_features() -> *mut c_char {
    let names: Vec<&str> = FEATURES.iter().map(|(n, _)| *n).collect();
    let body = names
        .iter()
        .map(|n| format!("\"{}\"", n))
        .collect::<Vec<_>>()
        .join(",");
    cstr_or_null(&format!("[{}]", body))
}

// ---------------------------------------------------------------------------
// C-ABI: 值树 (v2 API)
//
// 旧 API (sml_parse / sml_parse_file / sml_parse_ex) 以 JSON 字符串为交换格式，
// 迫使 C 侧再集成一个 JSON 库——这削弱了 SML 作为替代品的动机。
// 这套值树 API 让 C 直接遍历结果、直接读错误，零外部依赖。
//
// 设计参照 jansson (sml_error 详细定位 + flags 位标志) 与 tomlc99 (xxx_in 单行取值)。
// 生命周期约定：
//   * sml_loads / sml_load_file 返回的根指针由调用方 sml_free 释放；
//   * sml_get / sml_get_path / sml_at 返回**借用**指针 (const)，不可释放，
//     随根节点一同失效；
//   * 所有 char* 输出由调用方 sml_free_str 释放。
// ---------------------------------------------------------------------------

use std::os::raw::{c_uint, c_ulonglong};

/// 与 `sml_rs.h` 的 `sml_errc` 一一对应。
#[repr(C)]
#[derive(Clone, Copy)]
pub enum CSmlErrc {
    Ok = 0,
    Syntax = 1,
    FeatureDisabled = 2,
    VersionMismatch = 3,
    Contract = 4,
    IncludeLoop = 5,
    Io = 6,
    Utf8 = 7,
    Internal = 8,
}

/// 与 `sml_rs.h` 的 `sml_error` 一一对应。
///
/// 字段顺序、类型、数组长度必须与头文件完全一致，否则跨语言内存布局错位。
#[repr(C)]
pub struct CSmlError {
    pub code: c_int,
    pub line: c_int,
    pub column: c_int,
    pub position: usize,
    pub source: [c_char; 128],
    pub text: [c_char; 256],
}

impl CSmlError {
    /// 用错误信息填充一块调用方提供的内存。
    ///
    /// # Safety
    /// `out` 必须可写且按 [`CSmlError`] 布局对齐；为 NULL 时静默跳过。
    unsafe fn fill(out: *mut CSmlError, code: CSmlErrc, msg: &str, source: &str) {
        if out.is_null() {
            return;
        }
        let e = &mut *out;
        e.code = code as c_int;
        e.line = 0;
        e.column = 0;
        e.position = 0;
        e.source = [0; 128];
        e.text = [0; 256];
        copy_cstr(&mut e.source, source);
        copy_cstr(&mut e.text, msg);

        // 从消息里尽量还原行号：形如 "sml: 第 12 行 ..." / "... (line 12)"。
        if let Some(l) = extract_line(msg) {
            e.line = l;
        }
    }
}

/// 把 Rust `&str` 复制进定长 C 字符数组，保证 NUL 结尾且截断安全。
fn copy_cstr(dst: &mut [c_char], s: &str) {
    if dst.is_empty() {
        return;
    }
    let bytes = s.as_bytes();
    let n = bytes.len().min(dst.len() - 1);
    for i in 0..n {
        dst[i] = bytes[i] as c_char;
    }
    dst[n] = 0;
}

/// 从错误信息中抽取行号（尽力而为，抽不到返回 `None`）。
fn extract_line(msg: &str) -> Option<c_int> {
    for pat in ["第 ", "line "] {
        if let Some(idx) = msg.find(pat) {
            let rest = &msg[idx + pat.len()..];
            let digits: String = rest.chars().take_while(|c| c.is_ascii_digit()).collect();
            if let Ok(n) = digits.parse::<i32>() {
                if n > 0 {
                    return Some(n);
                }
            }
        }
    }
    None
}

/// 值树句柄。`repr(transparent)` 使其与内部 [`Value`] 布局一致，
/// 从而可以把子值的 `&Value` 安全地重解释为此类型的借用指针。
#[repr(transparent)]
pub struct CSmlValue(Value);

/// C 侧要释放的错误信息前缀判断：把解析错误归类。
fn classify(err: &str) -> CSmlErrc {
    if err.contains("include") && (err.contains("循环") || err.contains("loop")) {
        CSmlErrc::IncludeLoop
    } else if err.contains("特性") || err.contains("feature") {
        CSmlErrc::FeatureDisabled
    } else if err.contains("版本") || err.contains("version") {
        CSmlErrc::VersionMismatch
    } else if err.contains("契约") || err.contains("contract") {
        CSmlErrc::Contract
    } else if err.contains("读取失败") || err.contains("IO") {
        CSmlErrc::Io
    } else {
        CSmlErrc::Syntax
    }
}

/// `flags` 位 → [`FeatureSet`]。
///
/// `flags == 0` 视为「默认基线」（与 jansson 的 flags=0 语义一致），
/// 非 0 时按位精确构造，调用方可借此收紧允许范围。
fn feature_set_from_flags(flags: c_uint) -> FeatureSet {
    if flags == 0 {
        return FeatureSet::baseline();
    }
    let mut s = FeatureSet::none();
    for (i, (_, f)) in FEATURES.iter().enumerate() {
        if i >= 32 {
            break;
        }
        if flags & (1u32 << i) != 0 {
            s = s.with(*f);
        }
    }
    s
}

/// 解析 SML 文本为值树。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_loads(
    text: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if text.is_null() {
        CSmlError::fill(err, CSmlErrc::Internal, "sml_loads: text is NULL", "<string>");
        return ptr::null_mut();
    }
    let t = std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_with_features(&t, allowed) {
        Ok((v, _)) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(&e), &e, "<string>");
            ptr::null_mut()
        }
    }
}

/// 解析 SML 文件为值树（展开 `include`，相对路径以文件所在目录为基准）。
///
/// # Safety
/// `path` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_load_file(
    path: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if path.is_null() {
        CSmlError::fill(err, CSmlErrc::Internal, "sml_load_file: path is NULL", "<file>");
        return ptr::null_mut();
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_file_features(&p, allowed) {
        Ok(v) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(&e), &e, &p);
            ptr::null_mut()
        }
    }
}

/// 释放 [`sml_loads`] / [`sml_load_file`] 返回的根节点（NULL 安全）。
///
/// 与 `sml.h`（纯 C 后端）的 `sml_free` 语义一致：都是释放值树。
/// 释放字符串请用 [`sml_free_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_free(v: *mut CSmlValue) {
    if !v.is_null() {
        drop(Box::from_raw(v));
    }
}

/// 值类型判别，返回 `sml_type` 枚举值；NULL 或非预期返回 -1。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_typeof(v: *const CSmlValue) -> c_int {
    if v.is_null() {
        return -1;
    }
    let inner = &(*(v as *const Value));
    match inner {
        Value::Null => 0,
        Value::Bool(_) => 1,
        Value::Int(_) => 2,
        Value::Float(_) => 3,
        Value::Str(_) => 4,
        Value::Array(_) => 5,
        Value::Object(_) => 6,
    }
}

/// 取对象字段（**借用**，不可释放）；键不存在或类型不符返回 NULL。
///
/// # 安全契约（调用方必须遵守）
/// 返回的指针**借用**自 `v` 指向的值，**没有独立所有权**：
/// - 不得传给 [`sml_free`]（会 double free）；
/// - 不得在 `v` 被 `sml_free` 之后继续使用（use-after-free）；
/// - 需要延长生命周期时，必须先 `sml_clone` 取得独立副本。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_get(
    v: *const CSmlValue,
    key: *const c_char,
) -> *const CSmlValue {
    if v.is_null() || key.is_null() {
        return ptr::null();
    }
    let inner = &(*(v as *const Value));
    let k = std::ffi::CStr::from_ptr(key).to_string_lossy();
    match inner {
        Value::Object(m) => m
            .get(k.as_ref())
            .map(|x| x as *const Value as *const CSmlValue)
            .unwrap_or(ptr::null()),
        _ => ptr::null(),
    }
}

/// 按 `.` 分隔路径逐层取值（**借用**，不可释放）。
///
/// 生命周期约束同 [`sml_get`]：返回值借用自 `v`，不可释放、不可跨越 `v` 的
/// 释放点使用。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_get_path(
    v: *const CSmlValue,
    path: *const c_char,
) -> *const CSmlValue {
    if v.is_null() || path.is_null() {
        return ptr::null();
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy();
    let mut cur: *const CSmlValue = v;
    for seg in p.split('.') {
        if seg.is_empty() {
            continue;
        }
        let c_seg = match std::ffi::CString::new(seg) {
            Ok(c) => c,
            Err(_) => return ptr::null(),
        };
        let next = sml_get(cur, c_seg.as_ptr());
        if next.is_null() {
            return ptr::null();
        }
        cur = next;
    }
    cur
}

/// 取数组第 `idx` 个元素（**借用**，不可释放）。
///
/// 生命周期约束同 [`sml_get`]：返回值借用自 `v`，不可释放、不可跨越 `v` 的
/// 释放点使用。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_at(v: *const CSmlValue, idx: usize) -> *const CSmlValue {
    if v.is_null() {
        return ptr::null();
    }
    let inner = &(*(v as *const Value));
    match inner {
        Value::Array(a) => a
            .get(idx)
            .map(|x| x as *const Value as *const CSmlValue)
            .unwrap_or(ptr::null()),
        _ => ptr::null(),
    }
}

/// 元素个数（数组长度 / 对象字段数）；其它类型返回 0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_size(v: *const CSmlValue) -> usize {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Array(a) => a.len(),
        Value::Object(m) => m.len(),
        _ => 0,
    }
}

/// 把字符串值拷进调用方缓冲区，返回不含 NUL 的长度；缓冲区不足时返回所需长度。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_copy(
    v: *const CSmlValue,
    buf: *mut c_char,
    buflen: usize,
) -> usize {
    if v.is_null() {
        return 0;
    }
    let s = match &(*(v as *const Value)) {
        Value::Str(s) => s.as_str(),
        _ => return 0,
    };
    let need = s.len();
    if buf.is_null() || buflen == 0 {
        return need;
    }
    let n = need.min(buflen - 1);
    let src = s.as_bytes();
    for i in 0..n {
        *buf.add(i) = src[i] as c_char;
    }
    *buf.add(n) = 0;
    need
}

/// 字符串值的新分配副本（调用方 `sml_free_str` 释放）；非字符串返回 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_dup(v: *const CSmlValue) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    match &(*(v as *const Value)) {
        Value::Str(s) => match cstr(s) {
            Some(p) => p,
            None => ptr::null_mut(),
        },
        _ => ptr::null_mut(),
    }
}

/// 整数取值；非整数返回 0（用 [`sml_typeof`] 先判别类型）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_int_value(v: *const CSmlValue) -> i64 {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Int(i) => *i,
        Value::Float(f) => *f as i64,
        _ => 0,
    }
}

/// 浮点取值；非数值返回 0.0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_real_value(v: *const CSmlValue) -> f64 {
    if v.is_null() {
        return 0.0;
    }
    match &(*(v as *const Value)) {
        Value::Float(f) => *f,
        Value::Int(i) => *i as f64,
        _ => 0.0,
    }
}

/// 布尔取值；非布尔返回 0。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_bool_value(v: *const CSmlValue) -> c_int {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Bool(b) => {
            if *b {
                1
            } else {
                0
            }
        }
        _ => 0,
    }
}

// —— tomlc99 风格的单行便利取值 ——

/// `sml_get_path` + [`sml_str_dup`] 的合体（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_str_in(
    v: *const CSmlValue,
    path: *const c_char,
) -> *mut c_char {
    let node = sml_get_path(v, path);
    if node.is_null() {
        return ptr::null_mut();
    }
    sml_str_dup(node)
}

/// `sml_get_path` + [`sml_int_value`]，经 `ok` 回传是否取到（可为 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_int_in(
    v: *const CSmlValue,
    path: *const c_char,
    ok: *mut c_int,
) -> i64 {
    let node = sml_get_path(v, path);
    if node.is_null() {
        if !ok.is_null() {
            *ok = 0;
        }
        return 0;
    }
    let is_int = sml_typeof(node) == 2;
    if !ok.is_null() {
        *ok = if is_int { 1 } else { 0 };
    }
    sml_int_value(node)
}

/// `sml_get_path` + [`sml_bool_value`]，经 `ok` 回传是否取到（可为 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_bool_in(
    v: *const CSmlValue,
    path: *const c_char,
    ok: *mut c_int,
) -> c_int {
    let node = sml_get_path(v, path);
    if node.is_null() {
        if !ok.is_null() {
            *ok = 0;
        }
        return 0;
    }
    let is_bool = sml_typeof(node) == 1;
    if !ok.is_null() {
        *ok = if is_bool { 1 } else { 0 };
    }
    sml_bool_value(node)
}

/// 把值树序列化为 SML 文本（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dumps(v: *const CSmlValue, _flags: c_uint) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    #[cfg(feature = "sml")]
    {
        cstr_or_null(&to_sml(&(*(v as *const Value))))
    }
    #[cfg(not(feature = "sml"))]
    {
        ptr::null_mut()
    }
}

/// 返回该特性位对应的名字（静态字符串，无需释放）；越界返回 NULL。
///
/// 这里刻意用 `match` 返回带 `\0` 的字面量：直接取 [`FEATURES`] 里的
/// `&str` 无法保证 NUL 结尾，交给 C 会被 `printf("%s")` 越界读取。
/// 顺序与 [`FEATURES`] 表严格对应，由 `tests/version.rs` 中的用例守护。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_feature_name(bit: c_uint) -> *const c_char {
    let s: &'static str = match bit {
        0 => "bareword-string\0",
        1 => "include\0",
        2 => "env\0",
        3 => "contract\0",
        4 => "fragment\0",
        5 => "top-level-array\0",
        6 => "namespace\0",
        7 => "implicit-ns\0",
        8 => "multi-include\0",
        9 => "glob-include\0",
        10 => "regex-include\0",
        11 => "ext-rewrite\0",
        12 => "when\0",
        13 => "for\0",
        _ => return ptr::null(),
    };
    s.as_ptr() as *const c_char
}

/// 返回受支持特性的位掩码（可直接与 `SML_F_*` 按位与）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_features_mask() -> c_uint {
    let mut m = 0u32;
    for (i, _) in FEATURES.iter().enumerate() {
        if i >= 32 {
            break;
        }
        m |= 1u32 << i;
    }
    m
}

/// 库版本字符串（调用方 `sml_free_str` 释放）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_version_str() -> *mut c_char {
    cstr_or_null(env!("CARGO_PKG_VERSION"))
}

// 供 `#[no_mangle]` 之外的内部代码引用，避免 `c_ulonglong` 触发未使用警告。
#[allow(dead_code)]
type _CUnsignedLongLong = c_ulonglong;

// ---------------------------------------------------------------------------
// 内部: JSON <-> Value (供 C-ABI 便捷桥)
// ---------------------------------------------------------------------------

pub fn jsonify(v: &Value) -> String {
    fn esc(s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 2);
        for c in s.chars() {
            match c {
                '\\' => out.push_str("\\\\"),
                '"' => out.push_str("\\\""),
                '\n' => out.push_str("\\n"),
                '\r' => out.push_str("\\r"),
                '\t' => out.push_str("\\t"),
                // 控制字符 (0x00-0x1F) 必须转义为 \u00XX，否则产出非法 JSON
                // （下游 JSON 解析器遇裸换行/NUL 必报错）。此前仅转义 \\ 与 "，
                // 导致含换行的字符串被吐出成非法 JSON（C-ABI 审计 #1）。
                c if (c as u32) < 0x20 => {
                    out.push_str(&format!("\\u{:04x}", c as u32));
                }
                c => out.push(c),
            }
        }
        out
    }
    match v {
        Value::Null => "null".into(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        // NaN/inf 不是合法 JSON 字面量（`f.to_string()` 产出 "NaN"/"inf"，
        // 下游解析器必拒，且不可回读为 SML）。退化为 JSON `null`，既不破坏
        // 输出合法性，也明确标记「该值非有限数」（C-ABI 审计 #6）。
        Value::Float(f) => {
            if f.is_finite() {
                f.to_string()
            } else {
                "null".into()
            }
        }
        Value::Str(s) => format!("\"{}\"", esc(s)),
        Value::Array(a) => {
            let parts: Vec<String> = a.iter().map(jsonify).collect();
            format!("[{}]", parts.join(","))
        }
        Value::Object(m) => {
            let parts: Vec<String> = m
                .iter()
                .map(|(k, val)| format!("\"{}\":{}", esc(k), jsonify(val)))
                .collect();
            format!("{{{}}}", parts.join(","))
        }
    }
}

pub fn json_to_value(s: &str) -> Option<Value> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let _n = bytes.len();
    let mut skip_ws = |b: &[u8], i: &mut usize| {
        while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
            *i += 1;
        }
    };
    // 字符串解析：把字节累积进 `buf`，解析结束后整体按 UTF-8 解码。
    // 此前逐字节 `out.push(c as char)` 会把每个 UTF-8 字节当成一个码点，
    // 导致中文等多字节字符被二次编码成乱码（C-ABI 审计 #3）。
    let mut parse_str = |b: &[u8], i: &mut usize| -> Option<String> {
        skip_ws(b, i);
        if *i >= b.len() || b[*i] != b'"' {
            return None;
        }
        *i += 1;
        let mut buf: Vec<u8> = Vec::new();
        while *i < b.len() {
            let c = b[*i];
            if c == b'"' {
                *i += 1;
                return String::from_utf8(buf).ok();
            }
            if c == b'\\' && *i + 1 < b.len() {
                *i += 1;
                let e = b[*i];
                match e {
                    b'n' => buf.push(b'\n'),
                    b't' => buf.push(b'\t'),
                    b'r' => buf.push(b'\r'),
                    b'"' => buf.push(b'"'),
                    b'\\' => buf.push(b'\\'),
                    b'/' => buf.push(b'/'),
                    b'u' => {
                        // \uXXXX：读取 4 位十六进制，解码为码点后按 UTF-8 写入。
                        if *i + 4 < b.len() {
                            let hex = std::str::from_utf8(&b[*i + 1..*i + 5]).ok()?;
                            if let Ok(cp) = u32::from_str_radix(hex, 16) {
                                if let Some(ch) = std::char::from_u32(cp) {
                                    let mut tmp = [0u8; 4];
                                    buf.extend_from_slice(ch.encode_utf8(&mut tmp).as_bytes());
                                } else {
                                    buf.extend_from_slice('\u{FFFD}'.encode_utf8(&mut [0u8; 4]).as_bytes());
                                }
                                *i += 5;
                                continue;
                            }
                        }
                        buf.extend_from_slice('\u{FFFD}'.encode_utf8(&mut [0u8; 4]).as_bytes());
                    }
                    _ => buf.push(e),
                }
            } else {
                buf.push(c);
            }
            *i += 1;
        }
        None
    };
    // 递归深度上限：不可信 JSON 可能构造极深嵌套直接令宿主进程栈溢出
    // （abort，不可捕获），故需限制（C-ABI 审计 #4）。
    const MAX_DEPTH: usize = 128;
    fn parse_val_impl(
        b: &[u8],
        i: &mut usize,
        s: &str,
        parse_str: &dyn Fn(&[u8], &mut usize) -> Option<String>,
        depth: usize,
    ) -> Option<Value> {
        if depth > MAX_DEPTH {
            return None;
        }
        let mut skip_ws = |b: &[u8], i: &mut usize| {
            while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
                *i += 1;
            }
        };
        skip_ws(b, i);
        if *i >= b.len() {
            return None;
        }
        match b[*i] {
            b'{' => {
                *i += 1;
                let mut m = BTreeMap::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b'}' {
                    *i += 1;
                    return Some(Value::Object(m));
                }
                loop {
                    skip_ws(b, i);
                    let k = parse_str(b, i)?;
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b':' {
                        *i += 1;
                    }
                    let v = parse_val_impl(b, i, s, parse_str, depth + 1)?;
                    m.insert(k, v);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b'}' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Object(m))
            }
            b'[' => {
                *i += 1;
                let mut a = Vec::new();
                skip_ws(b, i);
                if *i < b.len() && b[*i] == b']' {
                    *i += 1;
                    return Some(Value::Array(a));
                }
                loop {
                    a.push(parse_val_impl(b, i, s, parse_str, depth + 1)?);
                    skip_ws(b, i);
                    if *i < b.len() && b[*i] == b',' {
                        *i += 1;
                    } else if *i < b.len() && b[*i] == b']' {
                        *i += 1;
                        break;
                    }
                }
                Some(Value::Array(a))
            }
            b'"' => parse_str(b, i).map(Value::Str),
            b't' => {
                if s[*i..].starts_with("true") {
                    *i += 4;
                    Some(Value::Bool(true))
                } else {
                    None
                }
            }
            b'f' => {
                if s[*i..].starts_with("false") {
                    *i += 5;
                    Some(Value::Bool(false))
                } else {
                    None
                }
            }
            b'n' => {
                if s[*i..].starts_with("null") {
                    *i += 4;
                    Some(Value::Null)
                } else {
                    None
                }
            }
            _ => {
                let start = *i;
                while *i < b.len()
                    && (b[*i].is_ascii_digit()
                        || matches!(b[*i], b'-' | b'+' | b'.' | b'e' | b'E'))
                {
                    *i += 1;
                }
                let tok = s[start..*i].to_string();
                if let Ok(iv) = tok.parse::<i64>() {
                    Some(Value::Int(iv))
                } else if let Ok(fv) = tok.parse::<f64>() {
                    Some(Value::Float(fv))
                } else {
                    None
                }
            }
        }
    }
    parse_val_impl(bytes, &mut i, s, &parse_str, 0)
}

