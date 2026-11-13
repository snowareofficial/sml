use sml_parse::*;
use sml_parse::strip_version;
use sml_contract::{Contract, FieldSpec, Strictness, TypeSpec};
use sml_feature::FEATURES;
use std::collections::BTreeMap;
use sml_value::Map;
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

/// `sml_parse(text)` -> **JSON** string (free with `sml_free_str`); NULL on failure. ｜ 返回 **JSON** 字符串（用 `sml_free_str` 释放）；失败返回 NULL。
///
/// # 语义：名字不描述数据方向
///
/// 它做的是 **SML → JSON**（解析后经内部 `jsonify` 转成 JSON 文本）。反方向的
/// **JSON → SML** 是 `sml_dump`。两个名字都不说方向，**不要按名字猜参数含义**。
///
/// # 陷阱
///
/// **你以为失败时能问出原因 —— 其实本函数一律只给 NULL**：没有码、没有位置、
/// 也不区分「语法错」与「特性被禁」。要错误语义请用 `sml_dump_err` 或值树 API。
///
/// **你以为按旧文案调 `sml_free(p)` 释放就行 —— 字符串要用 `sml_free_str`**。
/// `sml_free` 释放的是**值树**（与 `sml.h` 纯 C 后端一致），两者语义不同，
/// 改名原因见 `sml_free_str` 的说明。本函数此前的「调用方 sml_free 释放」是过期文案。
///
/// **你以为它能读带 `include` 的配置 —— 它内部走的是 `parse`，不展开 `include`**
/// （文本入口不做 IO）。要模块化请用 `sml_parse_file`。
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

/// `sml_dump(json)` -> SML text (free with `sml_free_str`). ｜ 接受 JSON 字符串，序列化为 SML；用 `sml_free_str` 释放。
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
            // 深度超限（`E-LIMIT-004`）时也返回 NULL：宁可不给结果，
            // 也不吐一份「看着合法、回读变成 null」的占位文本（W16）。
            Some(v) => match crate::to_sml_checked(&v) {
                Ok(s) => cstr_or_null(&s),
                Err(_) => ptr::null_mut(),
            },
            None => ptr::null_mut(),
        }
    }
    #[cfg(not(feature = "sml"))]
    {
        let _ = j;
        ptr::null_mut()
    }
}

/// sml_dump_err(json, err) —— 与 [`sml_dump`] 相同，但**失败时写 `err`**（W16）。
///
/// 为什么另开一个符号而不改 `sml_dump`：`sml_dump(json) -> *mut c_char` 是已发布的
/// C-ABI 契约，加参数会破坏既有调用方。本文件的既定做法就是「新符号并存」
/// （见 v3 扩展的 `sml_parse_ex`）。
///
/// 改前的痛点：`sml_dump` 遇到非法 JSON 只返回 NULL，调用方**无从区分**
/// 「输入不是 JSON」与「别的原因」，也没有码可查。
///
/// # Safety
/// `json` 须为合法 NUL 结尾的 C 字符串或 NULL；`err` 可为 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dump_err(
    json: *const c_char,
    err: *mut CSmlError,
) -> *mut c_char {
    if json.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_dump_err: json is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let j = unsafe { std::ffi::CStr::from_ptr(json) }.to_string_lossy().into_owned();
    #[cfg(feature = "sml")]
    {
        match json_to_value(&j) {
            Some(v) => match crate::to_sml_checked(&v) {
                Ok(s) => cstr_or_null(&s),
                Err(e) => {
                    CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
                    ptr::null_mut()
                }
            },
            None => {
                // JSON 解析失败：底层 `json_to_value` 只给 Option（没有更细的原因），
                // 故按约定用兜底码 `E-PARSE-012`（"未能给出更具体的原因"），
                // 粗粒度枚举沿用 `Syntax`（与 sml_rs.h 的 `sml_errc` 一一对应）。
                CSmlError::fill(
                    err,
                    CSmlErrc::Syntax,
                    sml_codes::E_PARSE_012,
                    "输入不是合法 JSON（或嵌套过深）",
                    "<string>",
                );
                ptr::null_mut()
            }
        }
    }
    #[cfg(not(feature = "sml"))]
    {
        let _ = j;
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_dump_err: 未启用 `sml` feature（无序列化器）",
            "<string>",
        );
        ptr::null_mut()
    }
}

/// sml_free_str(p): 释放由 sml_parse / sml_dump / sml_dumps 等返回的字符串。
///
/// 注：早期版本此函数名为 `sml_free`，与 `sml.h`（纯 C 后端）的
/// `sml_free(sml_value*)` 语义冲突。为让两个后端心智模型一致
/// （`sml_free` 释放值树、`sml_free_str` 释放字符串），此处重命名。
///
/// # Safety
/// `p` 必须是本库字符串入口（`sml_parse` / `sml_dump` / `sml_parse_ex` / `sml_dumps`）
/// 的返回值，或 NULL；且只能释放一次。
/// ｜ `p` must come from this library's string returns and be freed exactly once.
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
///   "allow":    [ "10", "20", "v3" ]
/// 返回 (features: Vec<Feature>, env: Vec<(String,String)>, allow: Vec<Edition>)。
/// 任何字段缺失即视为空/不限制; 解析失败返回 Err。
/// 解析 opts JSON 的三元组；抽成别名压住 `clippy::type_complexity`。
type ParsedOpts = (Vec<Feature>, Vec<(String, String)>, Vec<Edition>);

fn parse_opts_json(opts: &str) -> Result<ParsedOpts, String> {
    let mut features: Vec<Feature> = Vec::new();
    let mut env: Vec<(String, String)> = Vec::new();
    let mut allow: Vec<Edition> = Vec::new();
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
                        // 与文档里的 `@feature enable` 同一套判据（含 `<域>.<名>` 形态规则）
                        features.push(
                            Feature::parse_name(&val)
                                .map_err(|e| format!("特性名 `{val}` 无效：{e}"))?,
                        );
                    } else {
                        // `Edition::parse` 与文档里的 `@version` 同一套字面量（`10`/`1.0`/`v1`）
                        allow.push(Edition::parse(&val).ok_or_else(|| format!("未知版本 {val}"))?);
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

/// `sml_parse_ex(text, opts_json)` -> JSON string (free with `sml_free_str`) or NULL. ｜ 返回 JSON 字符串（用 `sml_free_str` 释放）或 NULL。
///
/// opts_json 示例:
///   {"features":["glob-include","contract"],"env":{"APP_ENV":"prod"},"allow":["v1","v3"]}
/// - features: 调用方额外启用的特性 (与文档 @feature 取交集)。
/// - env:      环境变量**覆盖表**，供 `$env.X` 内联解析。仅作用于本次调用，
///   不写入进程环境（避免并发调用下 `set_var` 的数据竞争，以及
///   覆盖 `PATH` / `LD_PRELOAD` 等影响进程行为的变量）。
/// - allow:    限定文档声明的版本必须在此范围内; 空数组表示不限制。
///
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
        // C ABI 只能传文本，此处显式取文案（不带码），避免隐式丢码。
        // ｜ The C ABI carries text only; take the message explicitly instead of a silent `?` conversion.
        let val = parse_with_features_env(&t, allowed, env_map)
            .map(|(v, _)| v)
            .map_err(|e| e.message().to_string())?;
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

/// Mirrors `sml_errc` in `sml_rs.h` one-to-one ｜ 与 `sml_rs.h` 的 `sml_errc` 一一对应。
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

/// Mirrors `sml_error` in `sml_rs.h` one-to-one ｜ 与 `sml_rs.h` 的 `sml_error` 一一对应。
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
    /// **真实错误码**（如 `E-PARSE-008`），来自 `errors/codes.sml`。
    ///
    /// 与 `code` 的分工：`code` 是粗粒度枚举（领域大类的历史 ABI，9 个取值），
    /// `code_str` 才是**稳定契约**——同一触发条件五端同码，且码不会随文案改写而变。
    /// 只容纳 15 字节，编译器生成的码一律 `E-<领域>-<三位序号>`，装得下。
    pub code_str: [c_char; 16],
}

impl CSmlError {
    /// 用错误信息填充一块调用方提供的内存。
    ///
    /// # Safety
    /// `out` 必须可写且按 [`CSmlError`] 布局对齐；为 NULL 时静默跳过。
    unsafe fn fill(
        out: *mut CSmlError,
        code: CSmlErrc,
        code_str: &str,
        msg: &str,
        source: &str,
    ) {
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
        e.code_str = [0; 16];
        copy_cstr(&mut e.source, source);
        copy_cstr(&mut e.text, msg);
        copy_cstr(&mut e.code_str, code_str);

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

/// Handle to a value tree ｜ 值树句柄。
///
/// `repr(transparent)` 使其与内部 [`Value`] 布局一致，从而可以把子值的 `&Value`
/// 安全地重解释为此类型的借用指针。
#[repr(transparent)]
pub struct CSmlValue(Value);

/// 把**错误码**归类为 C 侧的粗粒度枚举 `sml_errc`。
///
/// 这里原本是按文案里的中文关键词猜（`contains("特性")` …）—— 文案一改就归类错。
/// 现在改成读码本身：码是稳定契约（见 `errors/codes.sml`），归类不会漂移。
/// `sml_errc` 保持粗粒度是**有意的**：它是给 C 调用方做 `switch` 用的历史 ABI，
/// 精确的码请读 [`CSmlError::code_str`]。
fn classify(code: &str) -> CSmlErrc {
    match code {
        // 版本声明非法：历史行为归到「版本不匹配」这一档
        "E-FEATURE-004" => CSmlErrc::VersionMismatch,
        // 循环引用：历史行为单独一档
        "E-INCLUDE-002" => CSmlErrc::IncludeLoop,
        _ => match code.split('-').nth(1).unwrap_or("") {
            "FEATURE" => CSmlErrc::FeatureDisabled,
            "CONTRACT" => CSmlErrc::Contract,
            "IO" => CSmlErrc::Io,
            "INTERNAL" => CSmlErrc::Internal,
            _ => CSmlErrc::Syntax,
        },
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
/// 返回**值树**（根节点，用 [`sml_free`] 释放），不是 JSON / SML 文本 —— 要文本请走
/// `sml_parse` / [`sml_dumps`]（前者只写代码字体：它与 crate 名 `sml-parse` 同名，
/// 挂链接会触发 rustdoc 的歧义警告）。`flags` 是**允许的特性位掩码**：`0` 表示
/// [`FeatureSet::baseline`] 那八项核心能力，不是「全开」也不是「全关」；
/// 要放开全部传 `sml_features_mask()`。第 i 位对应哪个特性可用 [`sml_feature_name`] 反查。
///
/// **不展开 `include`**：本函数只解析传入的这一段文本；需要按文件展开 include 请用
/// [`sml_load_file`]。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
///
/// # 陷阱
///
/// **`flags = 0` 不等于「不限制」**：它是 baseline（八项核心），
/// 于是文档里 `@feature enable when` 这类 opt-in 能力会被**静默削掉**（见
/// `parse_with_features` 的说明：越权不是报错，而是取交集）。
///
/// **位序是 `FEATURES` 数组的下标**，且**只认前 32 个**（`i >= 32` 直接丢弃）。
/// C 侧别硬编码数字：用 `SML_F_*` 宏，或用 [`sml_feature_name`] / [`sml_features_mask`]
/// 现查，否则 Rust 侧调一次表顺序，C 侧就全错位了。
///
/// **非 UTF-8 输入被静默替换**：`text` 走 `to_string_lossy()`，
/// 非法字节变成 U+FFFD 且**不报错**。对字节流不放心的调用方请先自行校验编码。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_loads(
    text: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if text.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_loads: text is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let t = std::ffi::CStr::from_ptr(text).to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_with_features(&t, allowed) {
        Ok((v, _)) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
            ptr::null_mut()
        }
    }
}

/// 解析 SML 文件为值树（展开 `include`，相对路径以文件所在目录为基准）。
///
/// 与 [`sml_loads`] 的唯一差别是**会展开 `include`**（`flags` 语义、返回值所有权、
/// `err` 的用法全部相同）。include 的解析基准是**文件所在目录**；`path` 没有父目录时
/// 退回 `.`，也就是**进程当前工作目录** —— 同一份文档换个 cwd 跑，include 可能就找不到了。
///
/// 返回**值树**，用 [`sml_free`] 释放。
///
/// # Safety
/// `path` 必须是合法 NUL 结尾字符串或 NULL；`err` 可为 NULL。
///
/// # 陷阱
///
/// **`flags = 0` 是 baseline 八项，不是「不限制」**（同 [`sml_loads`]）：
/// 文档里的 `@when` / `@for` / `typed-block` 等 opt-in 语法会直接报 **`E-FEATURE-001`**
/// （不是静默忽略），要全放开传 `sml_features_mask()`。
///
/// **只要真展开了 `include`，错误就没有行列**：跨文件没有单一原文可映射，
/// 位置信息不再生成 —— 同一份内容「手拼成单文件」时能报「第 N 行第 M 列」，
/// 「文件 + include」时只有码和文案。
///
/// **本入口一条告警都拿不到**：它返回 `Value`，键冲突（`W-PARSE-001`）与片段覆盖
/// （`W-PARSE-002`）这两类诊断在这里被**丢弃**。文本入口的诊断出口是
/// [`sml_parse_with_diags`]；文件 + 诊断仍需回 Rust 侧用 `parse_file_with_warnings`
///（C-ABI 的文件 + 诊断入口尚未落，见 `docs/swsml/plan/crossend-sync.md` P7）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_load_file(
    path: *const c_char,
    flags: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if path.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_load_file: path is NULL",
            "<file>",
        );
        return ptr::null_mut();
    }
    let p = std::ffi::CStr::from_ptr(path).to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_file_features(&p, allowed) {
        Ok(v) => Box::into_raw(Box::new(CSmlValue(v))),
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), &p);
            ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// C-ABI: 非致命诊断（P7）
//
// 为什么单独一个入口而不改 `sml_loads`：诊断是**附加信息**，把它塞进旧签名会
// 逼所有既有调用方改代码（`sml_dump_err` 先例：并存新符号，不动旧签名）。
// 值树与 `sml_loads` 逐字节相同 —— 诊断只说明「数据被改写 / 覆盖 / 声明对不上」。
// ---------------------------------------------------------------------------

/// 非致命诊断的严重度 ｜ Severity of a non-fatal diagnostic.
///
/// 目前产出的一律是 `Warning`（键冲突 / 片段覆盖 / 弱声明不匹配）；
/// `Error` 档预留给后续的「多错误收集」出口（`ParseOutput::errors`）。
#[repr(C)]
#[derive(Clone, Copy)]
pub enum CSmlSeverity {
    Warning = 0,
    Error = 1,
}

/// Mirrors `sml_diag` in `sml_rs.h` one-to-one ｜ 与 `sml_rs.h` 的 `sml_diag` 一一对应。
///
/// 字段顺序 / 类型必须与头文件完全一致，否则跨语言内存布局错位。
/// 两个字符串指针由本结构**独占**：随 [`sml_diag_list_free`] 一并释放，不可单独 free。
#[repr(C)]
pub struct CSmlDiag {
    /// 稳定码（如 `W-PARSE-001`）。**NULL 表示该档暂无码**（`Deprecated` / 契约 `warn` 放行）。
    pub code_str: *mut c_char,
    /// 人读文案，**不含码**（与 `sml_error.text` 同一口径）。
    pub text: *mut c_char,
    /// 1 起行号；0 = 未知（文件入口跨 include 时没有可映射的单一原文）。
    pub line: c_int,
    /// [`CSmlSeverity`] 的取值。
    pub severity: c_int,
}

/// Mirrors `sml_diag_list` in `sml_rs.h` one-to-one ｜ 与 `sml_rs.h` 的 `sml_diag_list` 一一对应。
///
/// 空表时 `items` 为 NULL、`len` 为 0 —— 「没有诊断」与「没有拿到诊断」在
/// C 侧看起来一样，故 `len` **总是**被写；要区分请在调用前把 `len` 置哨兵值。
#[repr(C)]
pub struct CSmlDiagList {
    pub items: *mut CSmlDiag,
    pub len: usize,
}

/// 把 Rust 诊断列表搬进 C 结构（字符串逐个转成独立的 C 串）。
fn diag_list_into_c(diags: Vec<Diagnostic>) -> CSmlDiagList {
    if diags.is_empty() {
        return CSmlDiagList { items: ptr::null_mut(), len: 0 };
    }
    let mut items: Vec<CSmlDiag> = Vec::with_capacity(diags.len());
    for d in &diags {
        items.push(CSmlDiag {
            code_str: d.code().and_then(cstr).unwrap_or(ptr::null_mut()),
            text: cstr(&d.message).unwrap_or(ptr::null_mut()),
            line: d.line.map(|l| l as c_int).unwrap_or(0),
            severity: CSmlSeverity::Warning as c_int,
        });
    }
    let len = items.len();
    let boxed = items.into_boxed_slice();
    CSmlDiagList { items: Box::into_raw(boxed) as *mut CSmlDiag, len }
}

/// 解析文本为值树，并交出**非致命诊断** ｜ Parse text into a value tree and hand out the non-fatal diagnostics.
///
/// 与 [`sml_loads`] 的差别**只有一个出参**：`flags` 语义、值树所有权、`err` 用法
/// 完全相同，`value` 与 `sml_loads` 在其成功时逐字节相同。
///
/// 交出的是 [`Diagnostic`] 的 C 形状：稳定码（`W-PARSE-001` / `W-PARSE-002` /
/// `W-CONTRACT-001` 等）+ 文案 + 行号 + 严重度。**失败时不产诊断**——
/// 致命错误仍只走 `err`（`sml_error` 带 `code_str` 与行列）。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾的 C 字符串或 NULL；`out_diags` / `err` 均可为 NULL。
/// 非 NULL 的 `out_diags` 必须指向可写的 [`CSmlDiagList`]；其内容随后由
/// [`sml_diag_list_free`] 释放（可重复调用，NULL 安全）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_with_diags(
    text: *const c_char,
    flags: c_uint,
    out_diags: *mut CSmlDiagList,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    // 先把出参归零：失败路径不必再写一遍，调用方也不会读到上次的陈旧值。
    if !out_diags.is_null() {
        unsafe { *out_diags = CSmlDiagList { items: ptr::null_mut(), len: 0 } };
    }
    if text.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_parse_with_diags: text is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let allowed = feature_set_from_flags(flags);
    match parse_with_features_warnings(&t, allowed) {
        Ok((v, diags)) => {
            if !out_diags.is_null() {
                unsafe { *out_diags = diag_list_into_c(diags) };
            }
            Box::into_raw(Box::new(CSmlValue(v)))
        }
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
            ptr::null_mut()
        }
    }
}

/// 释放 [`sml_parse_with_diags`] 交出的诊断表（NULL 安全，可重复调用）。
///
/// **不释放值树**（那是 [`sml_free`] 的事），也不影响 `err` 的内容。
///
/// # Safety
/// `list` 必须是 [`sml_parse_with_diags`] 写过的那份结构（或 NULL），且只能释放一次
/// —— 释放后再读 `items` / `len` 是未定义行为（本函数会把两者清零，便于观察）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_diag_list_free(list: *mut CSmlDiagList) {
    if list.is_null() {
        return;
    }
    let l = unsafe { &mut *list };
    if !l.items.is_null() {
        let items = unsafe { Vec::from_raw_parts(l.items, l.len, l.len) };
        for d in &items {
            if !d.code_str.is_null() {
                unsafe { drop(std::ffi::CString::from_raw(d.code_str)) };
            }
            if !d.text.is_null() {
                unsafe { drop(std::ffi::CString::from_raw(d.text)) };
            }
        }
        drop(items);
    }
    l.items = ptr::null_mut();
    l.len = 0;
}

// ---------------------------------------------------------------------------
// C-ABI: 内置契约表 + 扩展选项（P7 第二片）
//
// 为什么要有这一段：`@weakbind` 的软校验要比对**宿主内置**的那份契约表，
// 而表在 Rust 侧是 `BTreeMap<String, Contract>`（`Contract` 含不可序列化的
// 校验器）⇒ 不能从 JSON 传。C 原生端先定了「不透明句柄 + 加法式构造」的形状
// （`sml_builtin_new` / `add_field` / `set_range` / `add_enum`），这里照搬，
// 于是切后端不用改应用代码。
// ---------------------------------------------------------------------------

/// 内置契约表句柄（`c/sml_rs.h` 的 `sml_builtin`）｜ Opaque handle for the builtin contract table.
///
/// 空表 ⇒ 弱绑定软校验**整段跳过**（否则一落地，所有弱声明都会报 `decl.unknown_contract`，
/// 见 `docs/swsml/spec/weakbind.md` §6.1）。表只读：注入后解析器不会改它。
pub struct CSmlBuiltin {
    map: BTreeMap<String, Contract>,
}

/// 把 C 侧的类型名翻成 [`TypeSpec`]；`"enum"` 先给空列表，由 `add_enum` 续填。
fn builtin_typespec(name: &str) -> Option<TypeSpec> {
    Some(match name {
        "any" => TypeSpec::Any,
        "str" => TypeSpec::Str,
        "int" => TypeSpec::Int,
        "num" => TypeSpec::Num,
        "bool" => TypeSpec::Bool,
        "enum" => TypeSpec::Enum(Vec::new()),
        _ => return None,
    })
}

/// 新建一个空的内置契约表（失败返回 NULL）｜ Allocate an empty builtin table.
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_builtin_new() -> *mut CSmlBuiltin {
    Box::into_raw(Box::new(CSmlBuiltin { map: BTreeMap::new() }))
}

/// 释放内置契约表（NULL 安全）。注入用的表在 `sml_parse_full` 返回后即可释放。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_free(b: *mut CSmlBuiltin) {
    if b.is_null() {
        return;
    }
    unsafe { drop(Box::from_raw(b)) };
}

/// 复制一份内置契约表（NULL 返回 NULL）｜ Clone a builtin table.
///
/// 用途：把 [`sml_result_contracts`] 交出的**借用**表变成宿主**独占**的句柄 ——
/// 于是「读完结果 → 释放结果 → 继续拿这张表当内置表注入」这条链不用吊着结果句柄的命。
/// 克隆出来的表要用 [`sml_builtin_free`] 释放（它归调用方所有）。
///
/// # Safety
/// `b` 必须是本库返回的、尚未释放的 `sml_builtin`（或 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_clone(b: *const CSmlBuiltin) -> *mut CSmlBuiltin {
    if b.is_null() {
        return ptr::null_mut();
    }
    Box::into_raw(Box::new(CSmlBuiltin { map: unsafe { (*b).map.clone() } }))
}

/// 表里有几份契约（NULL 返回 0）。给宿主做「表是不是真的建起来了」的自检。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_count(b: *const CSmlBuiltin) -> c_uint {
    if b.is_null() {
        return 0;
    }
    unsafe { (*b).map.len() as c_uint }
}

/// 声明一个字段：契约不存在时自动建，同名字段**后写覆盖**。
///
/// `ty` 取 `"any"` / `"str"` / `"int"` / `"num"` / `"bool"` / `"enum"`（枚举值另用
/// [`sml_builtin_add_enum`] 续填）；`required` 非 0 表示必填。
/// 返回 0 = 成功；非 0 = 参数非法（NULL / 未知类型），此时表**不动**。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_add_field(
    b: *mut CSmlBuiltin,
    contract: *const c_char,
    field: *const c_char,
    ty: *const c_char,
    required: c_int,
) -> c_int {
    if b.is_null() || contract.is_null() || field.is_null() || ty.is_null() {
        return -1;
    }
    let (Ok(cname), Ok(fname), Ok(tyname)) = (
        unsafe { std::ffi::CStr::from_ptr(contract) }.to_str(),
        unsafe { std::ffi::CStr::from_ptr(field) }.to_str(),
        unsafe { std::ffi::CStr::from_ptr(ty) }.to_str(),
    ) else {
        return -1;
    };
    let Some(spec_ty) = builtin_typespec(tyname) else {
        return -1;
    };
    let table = unsafe { &mut (*b).map };
    let c = table.entry(cname.to_string()).or_insert_with(|| Contract {
        name: cname.to_string(),
        fields: BTreeMap::new(),
        strictness: Strictness::Strict,
        requires: Vec::new(),
        allow_any: false,
        strict_paths: false,
        featags: Vec::new(),
    });
    c.fields.insert(
        fname.to_string(),
        FieldSpec {
            ty: spec_ty,
            required: required != 0,
            default: None,
            min: None,
            max: None,
            ext: None,
            ext_data: BTreeMap::new(),
            mods: Vec::new(),
            from_featag: false,
        },
    );
    0
}

/// 给已有字段设数值区间（含端点）。返回 0 = 成功；非 0 = 契约或字段不存在 / 参数 NULL。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_set_range(
    b: *mut CSmlBuiltin,
    contract: *const c_char,
    field: *const c_char,
    min: f64,
    max: f64,
) -> c_int {
    if b.is_null() || contract.is_null() || field.is_null() {
        return -1;
    }
    let (Ok(cname), Ok(fname)) = (
        unsafe { std::ffi::CStr::from_ptr(contract) }.to_str(),
        unsafe { std::ffi::CStr::from_ptr(field) }.to_str(),
    ) else {
        return -1;
    };
    let table = unsafe { &mut (*b).map };
    match table.get_mut(cname).and_then(|c| c.fields.get_mut(fname)) {
        Some(f) => {
            f.min = Some(min);
            f.max = Some(max);
            0
        }
        None => -1,
    }
}

/// 给 `enum` 字段追加一个合法取值。返回 0 = 成功；非 0 = 契约/字段不存在或字段不是 `enum`。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_builtin_add_enum(
    b: *mut CSmlBuiltin,
    contract: *const c_char,
    field: *const c_char,
    value: *const c_char,
) -> c_int {
    if b.is_null() || contract.is_null() || field.is_null() || value.is_null() {
        return -1;
    }
    let (Ok(cname), Ok(fname), Ok(val)) = (
        unsafe { std::ffi::CStr::from_ptr(contract) }.to_str(),
        unsafe { std::ffi::CStr::from_ptr(field) }.to_str(),
        unsafe { std::ffi::CStr::from_ptr(value) }.to_str(),
    ) else {
        return -1;
    };
    let table = unsafe { &mut (*b).map };
    match table.get_mut(cname).and_then(|c| c.fields.get_mut(fname)) {
        Some(FieldSpec { ty: TypeSpec::Enum(vals), .. }) => {
            vals.push(val.to_string());
            0
        }
        _ => -1,
    }
}

/// 扩展选项（`c/sml_rs.h` 的 `sml_opts`）｜ Extended options for [`sml_parse_full`].
///
/// 字段顺序与头文件**逐字节一致**；`struct_size` 必须填 `sizeof(sml_opts)`，
/// 将来追加字段时靠它判断调用方传的结构有多长（旧调用方不会被读到越界内存）。
#[repr(C)]
pub struct CSmlOpts {
    /// 本结构的字节数（`sizeof(sml_opts)`）；小于已知大小时报 `E-INTERNAL-001`。
    pub struct_size: usize,
    /// 特性上限位（`SML_F_*`）；0 = 基线。与 `sml_parse_with_diags` 的 `flags` 同义。
    pub flags: c_uint,
    /// `"KEY=VALUE"` 数组，供 `$env.X` 内联；可为 NULL。
    pub env: *const *const c_char,
    /// `env` 的元素个数。
    pub env_len: usize,
    /// 内置契约表句柄；NULL = 不做弱绑定软校验。
    pub builtins: *const CSmlBuiltin,
}

/// 把 `KEY=VALUE` 数组翻成 env 覆盖表；格式不对返回 `None`（已填 `err`）。
///
/// 安全前提：`arr` 要么为空，要么指向 `len` 个合法 NUL 结尾的 C 字符串。
unsafe fn env_from_c(
    arr: *const *const c_char,
    len: usize,
    err: *mut CSmlError,
) -> Option<BTreeMap<String, String>> {
    let mut map = BTreeMap::new();
    if arr.is_null() || len == 0 {
        return Some(map);
    }
    for i in 0..len {
        let p = unsafe { *arr.add(i) };
        if p.is_null() {
            CSmlError::fill(
                err,
                CSmlErrc::Internal,
                sml_codes::E_INTERNAL_001,
                "sml_parse_full: env 数组里有 NULL 项",
                "<string>",
            );
            return None;
        }
        let s = unsafe { std::ffi::CStr::from_ptr(p) }.to_string_lossy().into_owned();
        match s.split_once('=') {
            Some((k, v)) if !k.is_empty() => {
                map.insert(k.to_string(), v.to_string());
            }
            _ => {
                CSmlError::fill(
                    err,
                    CSmlErrc::Internal,
                    sml_codes::E_INTERNAL_001,
                    "sml_parse_full: env 项须为 KEY=VALUE",
                    "<string>",
                );
                return None;
            }
        }
    }
    Some(map)
}

/// 一条弱绑定（`sml_rs.h` 的 `sml_weakbind`）｜ One weak binding, C shape.
///
/// 与纯 C 端的 `sml_weakbind` **同形**（path / name），多一个 `line`。
/// 字段体的类型信息不导出：那是声明内部的字段规格，宿主按 `name` 查自己的契约表即可。
#[repr(C)]
pub struct CSmlWeakBind {
    /// 目标块路径（`"srv.web"` / `"list[0]"`）；顶层块为空串。
    pub path: *mut c_char,
    /// 声明的契约名（书写原样，库不做 resolve）。
    pub name: *mut c_char,
    /// 1 起行号；0 = 未知（文件入口跨 include 时没有可映射的单一原文）。
    pub line: c_int,
}

/// 一条弱数据（`@weakdata` 的载荷）｜ One `@weakdata` payload.
///
/// `value` **由表拥有**：不要单独 `sml_free`，随 [`sml_weaklists_free`] 一起释放。
#[repr(C)]
pub struct CSmlWeakData {
    pub path: *mut c_char,
    pub name: *mut c_char,
    pub line: c_int,
    /// 载荷值树（独立所有权，但**随表**释放）。
    pub value: *mut CSmlValue,
}

/// 一条弱 profile 引用（`@weakdoc`）｜ One `@weakdoc` reference (name only).
#[repr(C)]
pub struct CSmlWeakDoc {
    pub path: *mut c_char,
    pub name: *mut c_char,
    pub line: c_int,
}

/// 弱声明三表（`sml_rs.h` 的 `sml_weaklists`）｜ The three weak side tables.
///
/// 全部**不进主树**：`value` 里查不到它们的任何痕迹（`docs/swsml/spec/weakbind.md` §5）。
/// 空表时对应指针为 NULL、计数为 0。
#[repr(C)]
pub struct CSmlWeakLists {
    pub binds: *mut CSmlWeakBind,
    pub n_binds: usize,
    pub data: *mut CSmlWeakData,
    pub n_data: usize,
    pub docs: *mut CSmlWeakDoc,
    pub n_docs: usize,
}

/// 空表常量（每次写回参前用它归零）。
const EMPTY_WEAK_LISTS: CSmlWeakLists = CSmlWeakLists {
    binds: ptr::null_mut(),
    n_binds: 0,
    data: ptr::null_mut(),
    n_data: 0,
    docs: ptr::null_mut(),
    n_docs: 0,
};

/// 路径拼成 C 串：键名用 `.` 连接，数组下标写成 `[n]`（与 C 原生端同口径）。
fn path_to_c(segs: &[PathSeg]) -> *mut c_char {
    let mut s = String::new();
    for seg in segs {
        match seg {
            PathSeg::Key(k) => {
                if !s.is_empty() {
                    s.push('.');
                }
                s.push_str(k);
            }
            PathSeg::Index(i) => {
                s.push('[');
                s.push_str(&i.to_string());
                s.push(']');
            }
        }
    }
    cstr_or_null(&s)
}

fn line_or_zero(line: Option<usize>) -> c_int {
    line.map(|l| l as c_int).unwrap_or(0)
}

/// 把 [`ParseOutput`] 的三张弱表搬进 C 结构（数组按 `len == cap` 装箱，便于整块回收）。
fn weak_lists_into_c(out: &ParseOutput) -> CSmlWeakLists {
    let binds: Vec<CSmlWeakBind> = out
        .weak_binds
        .iter()
        .map(|w| CSmlWeakBind {
            path: path_to_c(&w.path),
            name: cstr_or_null(&w.name),
            line: line_or_zero(w.line),
        })
        .collect();
    let data: Vec<CSmlWeakData> = out
        .weak_data
        .iter()
        .map(|w| CSmlWeakData {
            path: path_to_c(&w.path),
            name: cstr_or_null(&w.name),
            line: line_or_zero(w.line),
            value: Box::into_raw(Box::new(CSmlValue(w.value.clone()))),
        })
        .collect();
    let docs: Vec<CSmlWeakDoc> = out
        .weak_docs
        .iter()
        .map(|w| CSmlWeakDoc {
            path: path_to_c(&w.path),
            name: cstr_or_null(&w.name),
            line: line_or_zero(w.line),
        })
        .collect();

    fn boxed<T>(v: Vec<T>) -> *mut T {
        if v.is_empty() {
            ptr::null_mut()
        } else {
            Box::into_raw(v.into_boxed_slice()) as *mut T
        }
    }
    let (n_binds, n_data, n_docs) = (binds.len(), data.len(), docs.len());
    CSmlWeakLists {
        binds: boxed(binds),
        n_binds,
        data: boxed(data),
        n_data,
        docs: boxed(docs),
        n_docs,
    }
}

/// 释放 [`sml_parse_full_lists`] / [`sml_load_file_full`] 交出的三张弱表（NULL 安全）。
///
/// **不释放值树**（那是 [`sml_free`] 的事）；`sml_weakdata.value` 由本函数一并释放，
/// 调用方不得单独 `sml_free` 它。释放后所有指针被清零，可重复调用。
///
/// # Safety
/// `lists` 必须是被上述两个入口写过的那份结构（或 NULL），且只能释放一次。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_weaklists_free(lists: *mut CSmlWeakLists) {
    if lists.is_null() {
        return;
    }
    unsafe fn free_c(p: *mut c_char) {
        if !p.is_null() {
            drop(std::ffi::CString::from_raw(p));
        }
    }
    let l = unsafe { &mut *lists };
    if !l.binds.is_null() {
        let v = unsafe { Vec::from_raw_parts(l.binds, l.n_binds, l.n_binds) };
        for d in &v {
            unsafe { free_c(d.path) };
            unsafe { free_c(d.name) };
        }
        drop(v);
    }
    if !l.data.is_null() {
        let v = unsafe { Vec::from_raw_parts(l.data, l.n_data, l.n_data) };
        for d in &v {
            unsafe { free_c(d.path) };
            unsafe { free_c(d.name) };
            if !d.value.is_null() {
                unsafe { drop(Box::from_raw(d.value)) };
            }
        }
        drop(v);
    }
    if !l.docs.is_null() {
        let v = unsafe { Vec::from_raw_parts(l.docs, l.n_docs, l.n_docs) };
        for d in &v {
            unsafe { free_c(d.path) };
            unsafe { free_c(d.name) };
        }
        drop(v);
    }
    unsafe { *lists = EMPTY_WEAK_LISTS };
}

/// `opts_from_c` 的三个产物：特性上限 / env 覆盖表 / 内置契约表。
type COptsParts = (FeatureSet, BTreeMap<String, String>, BTreeMap<String, Contract>);

/// 把 C 侧 `sml_opts` 翻成 Rust 的三个入参；失败返回 `None`（已填 `err`）。
unsafe fn opts_from_c(opts: *const CSmlOpts, err: *mut CSmlError) -> Option<COptsParts> {
    let mut flags: c_uint = 0;
    let mut env = BTreeMap::new();
    let mut builtins = BTreeMap::new();
    if !opts.is_null() {
        let o = unsafe { &*opts };
        if o.struct_size < std::mem::size_of::<CSmlOpts>() {
            CSmlError::fill(
                err,
                CSmlErrc::Internal,
                sml_codes::E_INTERNAL_001,
                "opts.struct_size 小于 sizeof(sml_opts)",
                "<string>",
            );
            return None;
        }
        flags = o.flags;
        env = unsafe { env_from_c(o.env, o.env_len, err) }?;
        if !o.builtins.is_null() {
            // 克隆：表归调用方所有，解析期间不借用它的生命周期。
            builtins = unsafe { (*o.builtins).map.clone() };
        }
    }
    Some((feature_set_from_flags(flags), env, builtins))
}

/// 解析文本，带完整选项 + 非致命诊断 + **弱声明三表** ｜
/// Parse text with the full options, the diagnostics **and** the three weak side tables.
///
/// [`sml_parse_full`] 等价于本函数把 `out_lists` 传 NULL。值树与
/// `sml_loads(text, flags)` 逐字节相同：选项与弱表都只报告，不改语义。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾的 C 字符串或 NULL；`opts` / `out_lists` / `out_diags` / `err`
/// 均可为 NULL。`out_lists` 非 NULL 时其内容由 [`sml_weaklists_free`] 释放。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_full_lists(
    text: *const c_char,
    opts: *const CSmlOpts,
    out_lists: *mut CSmlWeakLists,
    out_diags: *mut CSmlDiagList,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if !out_diags.is_null() {
        unsafe { *out_diags = CSmlDiagList { items: ptr::null_mut(), len: 0 } };
    }
    if !out_lists.is_null() {
        unsafe { *out_lists = EMPTY_WEAK_LISTS };
    }
    if text.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_parse_full_lists: text is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let Some((allowed, env, builtins)) = (unsafe { opts_from_c(opts, err) }) else {
        return ptr::null_mut();
    };
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    let options = ParseOptions::new().builtin_contracts(builtins);
    match parse_full(&t, allowed, env, options) {
        Ok(out) => {
            // 先搬弱表（借用 `out`），再搬诊断（移动 `out.diagnostics`），最后交出值树。
            if !out_lists.is_null() {
                unsafe { *out_lists = weak_lists_into_c(&out) };
            }
            if !out_diags.is_null() {
                unsafe { *out_diags = diag_list_into_c(out.diagnostics) };
            }
            Box::into_raw(Box::new(CSmlValue(out.value)))
        }
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
            ptr::null_mut()
        }
    }
}

/// 解析文本，带**完整选项**（特性上限 / env 注入 / 内置契约表）与非致命诊断 ｜
/// Parse with the full option set and hand out the non-fatal diagnostics.
///
/// 返回的值树与 `sml_loads(text, flags)` 逐字节相同（选项只影响 `$env` 取值、
/// 特性上限与**诊断内容**）；`@weakbind` 与内置表对不上时产出 `W-CONTRACT-001`。
/// 要同时拿弱声明三表请用 [`sml_parse_full_lists`]。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾的 C 字符串或 NULL；`opts` / `out_diags` / `err` 均可为 NULL
/// （`opts` 为 NULL 时等价于 [`sml_parse_with_diags`] 的 `flags = 0`）。
/// `opts->env` 指向的字符串与 `opts->builtins` 必须在调用期间有效。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_full(
    text: *const c_char,
    opts: *const CSmlOpts,
    out_diags: *mut CSmlDiagList,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    unsafe { sml_parse_full_lists(text, opts, ptr::null_mut(), out_diags, err) }
}

/// 解析**文件**（展开 include），带完整选项 / 非致命诊断 / 弱声明三表 ｜
/// Parse a file (expanding includes) with full options, diagnostics and weak tables.
///
/// 与 [`sml_load_file`] 的关系同 [`sml_parse_full_lists`] 之于 [`sml_loads`]：
/// 值树一致，只是多交出诊断与弱表。**行号**在跨 include 时可能为 0
/// （token 跨文件合并，没有可映射的单一原文 —— 不凭空造行号）。
///
/// # Safety
/// `path` 必须是合法 NUL 结尾的 C 字符串或 NULL；其余出参与 [`sml_parse_full_lists`] 相同。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_load_file_full(
    path: *const c_char,
    opts: *const CSmlOpts,
    out_lists: *mut CSmlWeakLists,
    out_diags: *mut CSmlDiagList,
    err: *mut CSmlError,
) -> *mut CSmlValue {
    if !out_diags.is_null() {
        unsafe { *out_diags = CSmlDiagList { items: ptr::null_mut(), len: 0 } };
    }
    if !out_lists.is_null() {
        unsafe { *out_lists = EMPTY_WEAK_LISTS };
    }
    if path.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_load_file_full: path is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let Some((allowed, env, builtins)) = (unsafe { opts_from_c(opts, err) }) else {
        return ptr::null_mut();
    };
    let p = unsafe { std::ffi::CStr::from_ptr(path) }.to_string_lossy().into_owned();
    let options = ParseOptions::new().builtin_contracts(builtins);
    match parse_file_full(&p, allowed, env, options) {
        Ok(out) => {
            // 先搬弱表（借用 `out`），再搬诊断（移动 `out.diagnostics`），最后交出值树。
            if !out_lists.is_null() {
                unsafe { *out_lists = weak_lists_into_c(&out) };
            }
            if !out_diags.is_null() {
                unsafe { *out_diags = diag_list_into_c(out.diagnostics) };
            }
            Box::into_raw(Box::new(CSmlValue(out.value)))
        }
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
            ptr::null_mut()
        }
    }
}

// ---------------------------------------------------------------------------
// C-ABI: 聚合结果 sml_result（P7 第六片）
//
// 为什么不再往 `sml_parse_full_lists` 加出参：每加一样能力就要改一次签名，五门跟着
// 重写一遍。这里改成「一次解析 → 一个结果句柄 → 用 getter 按需取」，新增输出只加
// 一个 `SML_WANT_*` 位与一个 getter，不动既有符号。
// ｜ One parse -> one result handle -> getters, so new outputs never reshape signatures.
// ---------------------------------------------------------------------------

/// 要哪些输出（位掩码）。不传的位对应 getter 返回空表。
pub const SML_WANT_DIAGS: c_uint = 1 << 0;
/// 多错误收集（等价 `ParseOptions::collect_errors`）：语句中**不会**因首个错误返回 NULL。
pub const SML_WANT_ERRORS: c_uint = 1 << 1;
/// 弱声明三表（`@weakbind` / `@weakdata` / `@weakdoc`）。
pub const SML_WANT_WEAK: c_uint = 1 << 2;
/// 契约映射（哪个实体应用了哪些契约）。
pub const SML_WANT_CONTRACT_MAP: c_uint = 1 << 3;
/// 本次解析收集到的契约表（可再喂给 [`sml_parse_full`] 当内置表，见 weakbind.md §6.1）。
pub const SML_WANT_CONTRACTS: c_uint = 1 << 4;

/// 一条「某实体应用了某个契约」（`sml_contractmap` 的元素）。
#[repr(C)]
pub struct CSmlContractRef {
    /// 实体路径（`"srv.web"` / `"list[0]"`）；顶层实体为空串。
    pub path: *mut c_char,
    /// 契约名（已 resolve，与 `@contract` 定义的键一致）。
    pub name: *mut c_char,
}

/// 契约映射的扁平表示（`sml_contractmap`）：每个 (实体, 契约) 一行，按应用顺序。
#[repr(C)]
pub struct CSmlContractMap {
    pub items: *mut CSmlContractRef,
    pub len: usize,
}

/// 一次解析的全部产物（`sml_result`，对 C 不透明）｜ Every output of one parse.
///
/// 值树与各表都由本句柄拥有：取出的是**借用**指针，随 [`sml_result_free`] 一并失效。
/// 因此**不要**对 `sml_result_value` 的结果调 `sml_free`。
pub struct CSmlResult {
    value: CSmlValue,
    diags: CSmlDiagList,
    errors: CSmlDiagList,
    weak: CSmlWeakLists,
    cmap: CSmlContractMap,
    contracts: Option<Box<CSmlBuiltin>>,
}

fn contract_map_into_c(map: &ContractMap) -> CSmlContractMap {
    let mut items: Vec<CSmlContractRef> = Vec::new();
    for e in map.entries() {
        // 每个 (实体, 契约) 一行：路径串各自独立，回收时逐条释放。
        for name in &e.names {
            items.push(CSmlContractRef {
                path: path_to_c(&e.path),
                name: cstr_or_null(name),
            });
        }
    }
    if items.is_empty() {
        return CSmlContractMap { items: ptr::null_mut(), len: 0 };
    }
    let len = items.len();
    CSmlContractMap { items: Box::into_raw(items.into_boxed_slice()) as *mut CSmlContractRef, len }
}

/// 多错误列表搬进 C 结构：`severity = 1`（error），行号由字节偏移换算。
/// ｜ Collected errors in C shape; line is derived from the byte offset.
fn errors_into_c(errors: &[sml_codes::SmlError], text: &str) -> CSmlDiagList {
    if errors.is_empty() {
        return CSmlDiagList { items: ptr::null_mut(), len: 0 };
    }
    let bytes = text.as_bytes();
    let mut items: Vec<CSmlDiag> = Vec::with_capacity(errors.len());
    for e in errors {
        // 只数换行字节：不切片，省掉 UTF-8 边界检查。
        let line = match e.pos() {
            Some(p) => bytes[..p.min(bytes.len())]
                .iter()
                .filter(|b| **b == b'\n')
                .count() as c_int
                + 1,
            None => 0,
        };
        items.push(CSmlDiag {
            code_str: cstr_or_null(e.code()),
            text: cstr_or_null(e.message()),
            line,
            severity: CSmlSeverity::Error as c_int,
        });
    }
    let len = items.len();
    CSmlDiagList { items: Box::into_raw(items.into_boxed_slice()) as *mut CSmlDiag, len }
}

/// 解析文本并交出一个聚合结果句柄 ｜ Parse text into one aggregated result handle.
///
/// `want` 是 `SML_WANT_*` 的位掩码；不传的位对应 getter 返回空表（`len == 0`）。
/// 传了 `SML_WANT_ERRORS` 时，**语法错也返回句柄**（值树是尽力而为的部分结果，
/// 全部错误在 `sml_result_errors` 里）；不传时行为与 [`sml_parse_full`] 一致 ——
/// 首个错误即返回 NULL 并填 `err`。
///
/// # Safety
/// `text` 必须是合法 NUL 结尾的 C 字符串或 NULL；`opts` / `err` 均可为 NULL。
/// 返回的句柄必须用 [`sml_result_free`] 释放一次。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_parse_all(
    text: *const c_char,
    opts: *const CSmlOpts,
    want: c_uint,
    err: *mut CSmlError,
) -> *mut CSmlResult {
    if text.is_null() {
        CSmlError::fill(
            err,
            CSmlErrc::Internal,
            sml_codes::E_INTERNAL_001,
            "sml_parse_all: text is NULL",
            "<string>",
        );
        return ptr::null_mut();
    }
    let Some((allowed, env, builtins)) = (unsafe { opts_from_c(opts, err) }) else {
        return ptr::null_mut();
    };
    let options = ParseOptions::new()
        .builtin_contracts(builtins)
        .collect_errors(want & SML_WANT_ERRORS != 0)
        .trace_contracts(want & SML_WANT_CONTRACT_MAP != 0)
        .capture_contracts(want & SML_WANT_CONTRACTS != 0);
    let t = unsafe { std::ffi::CStr::from_ptr(text) }.to_string_lossy().into_owned();
    match parse_full(&t, allowed, env, options) {
        Ok(out) => {
            // 先取**借用**型输出（弱表 / 契约映射 / 错误表 / 契约表），最后再搬走
            // diags 与 value —— 顺序反了就会踩「借用已部分移动的值」。
            let weak = if want & SML_WANT_WEAK != 0 {
                weak_lists_into_c(&out)
            } else {
                EMPTY_WEAK_LISTS
            };
            let cmap = match out.contract_map.as_ref() {
                Some(m) if want & SML_WANT_CONTRACT_MAP != 0 => contract_map_into_c(m),
                _ => CSmlContractMap { items: ptr::null_mut(), len: 0 },
            };
            let errors = if want & SML_WANT_ERRORS != 0 {
                errors_into_c(&out.errors, &t)
            } else {
                CSmlDiagList { items: ptr::null_mut(), len: 0 }
            };
            let contracts = match out.contracts {
                Some(m) if want & SML_WANT_CONTRACTS != 0 => Some(Box::new(CSmlBuiltin { map: m })),
                _ => None,
            };
            let diags = if want & SML_WANT_DIAGS != 0 {
                diag_list_into_c(out.diagnostics)
            } else {
                CSmlDiagList { items: ptr::null_mut(), len: 0 }
            };
            Box::into_raw(Box::new(CSmlResult {
                value: CSmlValue(out.value),
                diags,
                errors,
                weak,
                cmap,
                contracts,
            }))
        }
        Err(e) => {
            CSmlError::fill(err, classify(e.code()), e.code(), e.message(), "<string>");
            ptr::null_mut()
        }
    }
}

/// 结果里的值树（**借用**，随 [`sml_result_free`] 失效；不要 `sml_free` 它）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_value(r: *const CSmlResult) -> *mut CSmlValue {
    if r.is_null() {
        return ptr::null_mut();
    }
    unsafe { &(*r).value as *const CSmlValue as *mut CSmlValue }
}

/// 非致命诊断表（借用；未请求时 `len == 0`）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_diags(r: *const CSmlResult) -> *const CSmlDiagList {
    if r.is_null() {
        return ptr::null();
    }
    unsafe { &(*r).diags }
}

/// 解析期收集的多条错误（借用；仅 `SML_WANT_ERRORS` 时非空，`severity = 1`）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_errors(r: *const CSmlResult) -> *const CSmlDiagList {
    if r.is_null() {
        return ptr::null();
    }
    unsafe { &(*r).errors }
}

/// 弱声明三表（借用；仅 `SML_WANT_WEAK` 时非空）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_weak(r: *const CSmlResult) -> *const CSmlWeakLists {
    if r.is_null() {
        return ptr::null();
    }
    unsafe { &(*r).weak }
}

/// 契约映射（借用；仅 `SML_WANT_CONTRACT_MAP` 时非空）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_contract_map(r: *const CSmlResult) -> *const CSmlContractMap {
    if r.is_null() {
        return ptr::null();
    }
    unsafe { &(*r).cmap }
}

/// 本次解析收集到的契约表（**借用**的 `sml_builtin` 句柄；仅 `SML_WANT_CONTRACTS` 时非 NULL）。
///
/// 它的典型用途：读一份「内置契约用 SML 写」的文档，拿这里的表再传给
/// [`sml_parse_full`] 的 `opts.builtins`（`docs/swsml/spec/weakbind.md` §6.1）。
/// 不要 `sml_builtin_free` 它 —— 它随结果句柄释放。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_contracts(r: *const CSmlResult) -> *const CSmlBuiltin {
    if r.is_null() {
        return ptr::null();
    }
    match unsafe { &(*r).contracts } {
        Some(b) => &**b as *const CSmlBuiltin,
        None => ptr::null(),
    }
}

/// 释放聚合结果（NULL 安全）：值树、各表、字符串、契约表一并回收。
///
/// # Safety
/// `r` 必须是 [`sml_parse_all`] 返回的句柄（或 NULL），且只能释放一次。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_result_free(r: *mut CSmlResult) {
    if r.is_null() {
        return;
    }
    unsafe {
        let mut boxed = Box::from_raw(r);
        sml_diag_list_free(&mut boxed.diags);
        sml_diag_list_free(&mut boxed.errors);
        sml_weaklists_free(&mut boxed.weak);
        if !boxed.cmap.items.is_null() {
            let v = Vec::from_raw_parts(boxed.cmap.items, boxed.cmap.len, boxed.cmap.len);
            for item in &v {
                if !item.path.is_null() {
                    drop(std::ffi::CString::from_raw(item.path));
                }
                if !item.name.is_null() {
                    drop(std::ffi::CString::from_raw(item.name));
                }
            }
            drop(v);
        }
        drop(boxed);
    }
}

/// 释放 [`sml_loads`] / [`sml_load_file`] 返回的根节点（NULL 安全）。
///
/// 与 `sml.h`（纯 C 后端）的 `sml_free` 语义一致：都是释放值树。
/// 释放字符串请用 [`sml_free_str`]。
///
/// # Safety
/// `v` 必须是 [`sml_loads`] / [`sml_load_file`] 返回的根节点（或 NULL），且只能释放
/// 一次；释放后再使用该句柄（含其子节点）是未定义行为。
/// ｜ Free the root once; any use of the handle or its children afterwards is UB.
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_free(v: *mut CSmlValue) {
    if !v.is_null() {
        drop(Box::from_raw(v));
    }
}

/// 值类型判别，返回 `sml_type` 枚举值；NULL 或非预期返回 -1。
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄（或 NULL）；释放后再用是未定义行为。
/// ｜ `v` must be a live handle from this library, or NULL.
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
        Value::Float(_, _) => 3,
        Value::Str(_) => 4,
        Value::Array(_) => 5,
        Value::Object(_) => 6,
    }
}

/// 取对象字段（**借用**，不可释放）；键不存在或类型不符返回 NULL。
///
/// # Safety
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
///
/// 语义：从 `v` 出发按段调 [`sml_get`]，任一段取不到就返回 NULL。**空段被跳过**
/// （`a..b` / `.a` / `a.` 均等价于 `a.b`）；`path` 为空串时零段生效、原样返回 `v`。
///
/// # 例子（C 侧）
///
/// ```c
/// const CSmlValue *host = sml_get_path(root, "db.host");
/// if (!host) { /* 路径不存在，或中途不是对象 */ }
/// ```
///
/// # 陷阱
///
/// **只能穿对象，不能穿数组**：`items.0.name` 这类写法**永远失败** ——
/// 路径语法里没有下标，数组元素只能先取到数组、再用 [`sml_at`] 逐层往下。
///
/// **键名自带 `.` 的字段取不到**：分段后每段都当作**完整键名**去匹配，
/// 所以 `k: "a.b"` 这种字段路径表达不出来，只能用 [`sml_get`] 显式取。
///
/// **NULL 不说原因**：「路径不存在」「中途不是对象」返回的同一个 NULL，
/// 与「取到的值本身是空」也无法区分。要定位是哪一段断的，就自己分段调 [`sml_get`]。
///
/// # Safety
/// 返回的指针**借用**自 `v`，规则同 [`sml_get`]：不可 `sml_free`，不可在 `v`
/// 释放之后使用；`path` 必须是 NUL 结尾的 C 字符串。
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
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄；返回指针借用自 `v`，不可 `sml_free`。
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
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄（或 NULL）。
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

/// 对象第 `idx` 个键（NUL 结尾的**堆副本**，调用方 `sml_free_str` 释放）；非对象 / 越界返回 NULL。
///
/// 注：内部键为 Rust `String`（非 NUL 结尾），无法安全「借用」为 C 串，
/// 故返回堆副本——语义同 [`sml_str_dup`]。调用方读后务必 `sml_free_str`，
/// 切勿当借用指针跨 `sml_free(v)` 长期使用。
///
/// 顺序与 [`sml_get`] 一致：对象的迭代序 —— `Map::Unordered` 态是键的字典序
/// （`BTreeMap`），`Map::Ordered` 态是**源序**（见 §8：迭代序是行为契约，
/// C 侧不得假设按键排序）。
///
/// # Safety
/// `v` 必须是有效的对象句柄；返回的 `*mut c_char` 是**新分配**，必须 [`sml_free_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_key_at(v: *const CSmlValue, idx: usize) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    let inner = &(*(v as *const Value));
    match inner {
        Value::Object(m) => m
            .keys()
            .nth(idx)
            .and_then(cstr)
            .unwrap_or(ptr::null_mut()),
        _ => ptr::null_mut(),
    }
}

/// 把字符串值拷进调用方缓冲区，返回不含 NUL 的长度；缓冲区不足时返回所需长度。
///
/// 两趟用法：`buf` 为 NULL（或 `buflen == 0`）时**只回报所需字节数、一个字节都不写**；
/// `buf` 非空时最多写 `buflen - 1` 字节再补 NUL，而返回值**始终是完整所需长度** ——
/// 所以 `ret >= buflen` 就是「被截断了，请按 `ret` 重新分配」。
///
/// # 陷阱
///
/// **长度是字节，不是字符**：截断按字节切，`buflen - 1` 正好落在某个 UTF-8 多字节字符
/// 中间时，缓冲区里留下的是**非法 UTF-8 半字符**，且不报任何错。中文/emoji 字段尤其容易中招。
///
/// **`0` 有三种含义**：空串、非字符串、`v` 为 NULL —— 返回值无法区分。
/// 要区分就先 [`sml_typeof`]（`4` = Str），再用 [`sml_str_dup`] 或本函数两趟取。
///
/// # Safety
/// `v` 必须是有效句柄；`buf` 必须可写 `buflen` 字节，或为 NULL（此时只回报长度）。
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
    std::ptr::copy_nonoverlapping(src.as_ptr(), buf as *mut u8, n);
    *buf.add(n) = 0;
    need
}

/// 字符串值的新分配副本（调用方 `sml_free_str` 释放）；非字符串返回 NULL。
///
/// # Safety
/// `v` 必须是有效句柄；返回的字符串是**新分配**，必须 [`sml_free_str`]。
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
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄（或 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_int_value(v: *const CSmlValue) -> i64 {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Int(i) => *i,
        Value::Float(f, _) => *f as i64,
        _ => 0,
    }
}

/// 浮点取值；非数值返回 0.0。
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄（或 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_real_value(v: *const CSmlValue) -> f64 {
    if v.is_null() {
        return 0.0;
    }
    match &(*(v as *const Value)) {
        Value::Float(f, _) => *f,
        Value::Int(i) => *i as f64,
        _ => 0.0,
    }
}

/// 布尔取值；非布尔返回 0。
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄（或 NULL）。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_bool_value(v: *const CSmlValue) -> c_int {
    if v.is_null() {
        return 0;
    }
    match &(*(v as *const Value)) {
        Value::Bool(b)
            if *b => {
                1
            }
        _ => 0,
    }
}

// —— tomlc99 风格的单行便利取值 ——

/// `sml_get_path` + [`sml_str_dup`] 的合体（调用方 `sml_free_str` 释放）。
///
/// # Safety
/// `v` 必须是有效句柄；`path` 必须是 NUL 结尾的 C 字符串；返回的字符串是**新分配**。
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
///
/// `ok` 写 `1` 的条件是**节点类型是 `Int`**（不是「值是数字」）；取不到、类型不对、`v` 为 NULL
/// 一律写 `0`。
///
/// # 陷阱
///
/// **必须看 `ok`，不能看返回值**：`ok = 0` 时返回值**仍可能非 0** ——
/// 字段是 `Float` 时 [`sml_int_value`] 会把它**截断**成 `i64` 返回（`1.9` ⇒ `1`），
/// 而 `ok` 同时是 `0`。所以「返回 1」既不等于「成功」，也不等于「原值就是 1」。
///
/// **`ok` 传 NULL 就没救**：失败与「值就是 0」返回的是同一个 `0`。
///
/// # Safety
/// `v` 必须是有效句柄；`path` 必须是 NUL 结尾的 C 字符串；`ok` 可写 4 字节或为 NULL。
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
///
/// 与 [`sml_int_in`] 同形：`ok` 写 `1` 的条件是节点类型**确实是 `Bool`**。
///
/// # 陷阱
///
/// **同样是「看 `ok`」而不是「看返回 0/1」**：`false` 与非布尔字段都返回 `0`，
/// 只有 `ok` 能区分「读到了 false」和「压根没读到」。
///
/// # Safety
/// `v` 必须是有效句柄；`path` 必须是 NUL 结尾的 C 字符串；`ok` 可写 4 字节或为 NULL。
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
///
/// 与 Rust 侧 `to_sml` 同源，但走的是 `crate::to_sml_checked` 那条**会失败**的路
/// （该入口被 `sml` feature 门控，故这里只写代码字体不挂链接）：
/// 深度超限返回 NULL，而不是吐 `/* …深度超限… */ null` 占位文本（那样回读会变成空值）。
///
/// # 陷阱
///
/// **`_flags` 目前被完全忽略**（形参名带下划线不是笔误）—— 传任何值都不改变输出。
/// 别以为它能选「canonical / 保序」：想按键字节序请用 Rust 的 `to_sml_canonical`。
///
/// **NULL 有两种来源**：`v` 为 NULL、或超过 [`crate::MAX_VALUE_DEPTH`]（128 层，根算第 0 层）。
/// 本函数没有 `err` 出参，**区分不了**，也不告诉你差多少层。
///
/// # Safety
/// `v` 必须是本库返回、尚未释放的句柄；返回的字符串是**新分配**，必须 [`sml_free_str`]。
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dumps(v: *const CSmlValue, _flags: c_uint) -> *mut c_char {
    if v.is_null() {
        return ptr::null_mut();
    }
    #[cfg(feature = "sml")]
    {
        // W16：深度超限返回 NULL（不吐占位文本）。与 `sml_dumps` 的既有契约
        // 「失败返回 NULL」一致 —— 只是以前那种「失败」根本不会发生。
        match crate::to_sml_checked(&(*(v as *const Value))) {
            Ok(s) => cstr_or_null(&s),
            Err(_) => ptr::null_mut(),
        }
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
/// 顺序与 [`FEATURES`] 表严格对应，由 `tests/c_abi.rs` 的
/// `feature_name_matches_features_table` 用例守护。
///
/// **新增特性时必须同步这张表**：`sml-feature` 的 `FEATURES` 增长后，
/// 若这里漏补，C 调用方会对新特性位拿到 NULL（既有的失败即由此而来：
/// `typed-block` 入表后本函数未同步，bit 14 返回 NULL）。
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
        14 => "typed-block\0",
        15 => "case-insensitive\0",
        16 => "block-name\0",
        17 => "type.featag\0",
        18 => "lex.ident\0",
        19 => "type.weak\0",
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

/// Hand-rolled JSON writer behind the C-ABI JSON bridge (`sml_dump` & friends); never fails ｜ C-ABI JSON 桥（`sml_dump` 一族）背后的手写 JSON 写出器；不会失败。
///
/// 改这个函数前先读三条已踩过的坑（都是审计里红过的）：
///
/// - 控制字符（`0x00`–`0x1F`）必须转义成 `\u00XX`。早先只转义 `\` 与 `"`，
///   含换行的字符串会被吐成**非法 JSON**，下游解析器必拒（C-ABI 审计 #1）。
/// - `NaN` / `±inf` 不是合法 JSON 字面量，退化成 `null`（连 `Value::Float` 的
///   raw 一起丢掉），既不破坏输出合法性，也明确标记「该值非有限数」（审计 #6）。
/// - 键序沿用 `Value::Object` 的载体：默认 `BTreeMap` 已排序，开 `preserve_order`
///   则是源序 —— 所以本函数的输出**不是**规范化的跨档稳定文本。
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
        Value::Float(f, _) => {
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

/// Parse JSON text back into a [`Value`]; `None` on any error, with **no diagnostics** ｜ 把 JSON 文本解析回 [`Value`]；任何错误都只给 `None`，**没有诊断信息**。
///
/// 与 [`jsonify`] 是一对（`sml_dump` 收、`sml_dump_err` 等回）。四条实现约束：
///
/// - 只要 **UTF-8**：字符串先按字节收进 `buf`、收完整体解码，非法 UTF-8 直接 `None`
///   —— 早先逐字节 `as char` 会把多字节字符二次编码成乱码（审计 #3）。
/// - `\uXXXX` 解不出来（越界 / 非十六进制 / 非码点）**不报错**，写 `U+FFFD` 占位。
/// - 数字先试 `i64`、失败再试 `f64` ⇒ 超出 `i64` 的整数会被 `f64` 接住、**精度不保证**。
/// - 递归深度上限 128：不可信 JSON 能构造极深嵌套把宿主栈打爆（abort，不可 catch），
///   超限返回 `None`（审计 #4）。
///
/// 解析完**不检查尾随内容**：`{"a":1} 垃圾` 仍返回 `Ok`-等价的 `Some`。
/// 当作严格校验用会漏，需要严格性请走 `parse_*` 那一套（带码的 `SmlError`）。
pub fn json_to_value(s: &str) -> Option<Value> {
    let bytes = s.as_bytes();
    let mut i = 0;
    let _n = bytes.len();
    let skip_ws = |b: &[u8], i: &mut usize| {
        while *i < b.len() && matches!(b[*i], b' ' | b'\t' | b'\n' | b'\r') {
            *i += 1;
        }
    };
    // 字符串解析：把字节累积进 `buf`，解析结束后整体按 UTF-8 解码。
    // 此前逐字节 `out.push(c as char)` 会把每个 UTF-8 字节当成一个码点，
    // 导致中文等多字节字符被二次编码成乱码（C-ABI 审计 #3）。
    let parse_str = |b: &[u8], i: &mut usize| -> Option<String> {
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
        let skip_ws = |b: &[u8], i: &mut usize| {
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
                // 用 `Map`（= `Value::Object` 的载体）：`Map::Ordered` 态下这里建出来的对象
                // 保的是**文本里的出现序**，正是 C-ABI 要承诺的迭代序（§8）。
                let mut m = Map::new();
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
                    Some(Value::float(fv))
                } else {
                    None
                }
            }
        }
    }
    parse_val_impl(bytes, &mut i, s, &parse_str, 0)
}

// ---------------------------------------------------------------------------
// wasm 专用：宿主内存分配
//
// wasm32-unknown-unknown 的 cdylib **不导出 malloc/free**（宿主默认是
// wasm-bindgen 生态，由 JS 侧管理内存）。但本 crate 刻意不依赖 wasm-bindgen
// —— 那样会引入 __wbindgen_placeholder__ 等导入，导致裸
// WebAssembly.instantiate 失败，SML 也就无法在纯静态站点上跑。
//
// 因此这里补两个最小分配器，供宿主写入输入字符串后再调 sml_* 系列。
// 用 cfg 门控，仅在 wasm 构建中出现，不影响 native / cdylib 的既有 ABI。
// ---------------------------------------------------------------------------

/// 分配 `n` 字节（内容清零），返回指针；失败返回 NULL。
///
/// 宿主应按 C 字符串惯例在末尾写 NUL；用完须调用 [`sml_dealloc`] 释放。
#[cfg(target_arch = "wasm32")]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub extern "C" fn sml_alloc(n: usize) -> *mut u8 {
    let mut buf: Vec<u8> = vec![0u8; n];
    let p = buf.as_mut_ptr();
    std::mem::forget(buf); // 所有权移交宿主，避免此处立即释放
    p
}

/// 释放 [`sml_alloc`] 得到的指针。
///
/// # Safety
/// `p` 必须是 [`sml_alloc`] 的返回值、`n` 必须与申请时一致，且不得重复释放。
/// ｜ `p` must come from `sml_alloc` with the same `n`, freed exactly once.
#[cfg(target_arch = "wasm32")]
#[cfg_attr(edge2024, unsafe(no_mangle))]
#[cfg_attr(not(edge2024), no_mangle)]
pub unsafe extern "C" fn sml_dealloc(p: *mut u8, n: usize) {
    if p.is_null() || n == 0 {
        return;
    }
    // len=0 / cap=n：不构造任何元素，仅回收分配，避免对未初始化内存调 drop
    let _ = Vec::<u8>::from_raw_parts(p, 0, n);
}

