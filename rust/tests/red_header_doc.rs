//! 红头文件（党政机关公文）识别 —— @type 的中文政务实战
//!
//! 发文字号格式：`机关代字〔年份〕序号号`，例如 `某政办发〔2026〕15号`。
//! 这里的关键适配点：`类: 字母` 用 Unicode 语义，汉字能被 `Alpha` 匹配，
//! 因此「机关代字」这类中文段落无需额外规则。

fn ok(src: &str) -> sml::Value {
    sml::parse(src).unwrap_or_else(|e| panic!("应解析成功，实际失败: {e}\n---\n{src}\n---"))
}

fn err<S: AsRef<str>>(src: S) -> String {
    sml::parse(src.as_ref()).unwrap_err()
}

const DOC: &str = r#"
@type name: 日期ISO {
    序列: [
        { 名: 年, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 月, 类: 数字, 次: 2 }
        { 字面: "-" }
        { 名: 日, 类: 数字, 次: 2 }
    ]
}

@type name: 发文字号 {
    序列: [
        { 名: 机关代字, 类: 字母, 次: "+" }
        { 字面: "〔" }
        { 名: 年份, 类: 数字, 次: 4 }
        { 字面: "〕" }
        { 名: 序号, 类: 数字, 次: "+" }
        { 字面: "号" }
    ]
}

@type name: 联系电话 {
    序列: [ { 名: 号, 类: 数字, 次: 11 } ]
}

@contract 红头文件 strict {
    发文机关: str
    发文字号: 发文字号
    标题: str
    成文日期: 日期ISO
    联系人电话: 联系电话
    密级: enum [ 公开 内部 秘密 机密 绝密 ] default 公开
}

@is 红头文件
发文机关: 某某市人民政府办公室
发文字号: "某政办发〔2026〕15号"
标题: 关于加强政务数据安全管理的通知
成文日期: "2026-09-06"
联系人电话: "13800138000"
密级: 秘密
"#;

#[test]
fn 合法红头文件通过校验() {
    let v = ok(DOC);
    assert_eq!(
        v.get("发文字号").and_then(|x| x.as_str()),
        Some("某政办发〔2026〕15号")
    );
    assert_eq!(v.get("密级").and_then(|x| x.as_str()), Some("秘密"));
    assert_eq!(v.get("发文机关").and_then(|x| x.as_str()), Some("某某市人民政府办公室"));
}

#[test]
fn 发文字号格式错误被拦截() {
    // 用方括号而非六角括号
    let e = err(DOC.replace("某政办发〔2026〕15号", "某政办发[2026]15号"));
    assert!(e.contains("发文字号"), "应指出字段名，实际: {e}");

    // 年份只有两位
    let e = err(DOC.replace("某政办发〔2026〕15号", "某政办发〔26〕15号"));
    assert!(e.contains("发文字号"), "实际: {e}");

    // 缺「号」字
    let e = err(DOC.replace("某政办发〔2026〕15号", "某政办发〔2026〕15"));
    assert!(e.contains("发文字号"), "实际: {e}");
}

#[test]
fn 多种机关代字都能识别() {
    // 中文机关代字长度不同，均靠 类: 字母 + 次: "+" 覆盖
    for 字号 in [
        "国发〔2026〕1号",
        "浙政办发〔2026〕15号",
        "某厅函〔2025〕203号",
    ] {
        let d = DOC.replace("某政办发〔2026〕15号", 字号);
        ok(&d);
    }
}

#[test]
fn 密级越界被拦截() {
    let e = err(DOC.replace("密级: 秘密", "密级: 绝密级"));
    assert!(e.contains("密级"), "实际: {e}");
}

/// 与项目自带的 MiniRegex 做同语义吞吐对比。
/// 注意：MiniRegex 不支持 \d / {n}，故用它能表达的等价写法（11 个 [0-9]）。
#[test]
fn 性能对比_手机号模式() {
    use std::time::Instant;

    const N: usize = 20_000;
    let text = "13800138000";

    // 本引擎：11 位数字
    use sml::pattern::{Class, Pat};
    let pat = Pat::Repeat {
        pat: Box::new(Pat::Class(Class::Digit)),
        min: 11,
        max: Some(11),
    };
    let t0 = Instant::now();
    let mut hits = 0;
    for _ in 0..N {
        if sml::pattern::is_match(&pat, text).unwrap() {
            hits += 1;
        }
    }
    let dt_loom = t0.elapsed();
    assert_eq!(hits, N);

    // MiniRegex：等价的 11 个 [0-9]
    let re = sml::compile_regex("^[0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9][0-9]$");
    let t1 = Instant::now();
    let mut hits2 = 0;
    for _ in 0..N {
        if sml::regex_matches(&re, text) {
            hits2 += 1;
        }
    }
    let dt_re = t1.elapsed();
    assert_eq!(hits2, N, "两者语义应一致");

    println!(
        "PERF N={N}  loom={:?}  miniregex={:?}  ratio={:.2}x",
        dt_loom,
        dt_re,
        dt_loom.as_secs_f64() / dt_re.as_secs_f64().max(1e-9)
    );
}
