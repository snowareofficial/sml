//! @type 自定义类型（模式校验）接入契约系统
//!
//! 核心目标：把「裸写号码导致静默损坏」变成显式报错。
//!   证件号: 221099 1988 0987 1211   → 裸写会被词法切成数字，只剩 1211 且不报错
//!   证件号: "221099 1988 0987 1211" → 字符串，交给模式校验

fn ok<S: AsRef<str>>(src: S) -> sml::Value {
    let s = src.as_ref();
    sml::parse(s).unwrap_or_else(|e| panic!("应解析成功，实际失败: {e}\n---\n{s}\n---"))
}

fn err<S: AsRef<str>>(src: S) -> String {
    sml::parse(src.as_ref()).unwrap_err().to_string()
}

const DOC: &str = r#"
@type name: 手机号 {
    序列: [ { 名: 号, 类: 数字, 次: 11 } ]
}

@type name: 身份证分组 {
    序列: [
        { 名: 一, 类: 数字, 次: 6 }
        { 字面: " " }
        { 名: 二, 类: 数字, 次: 4 }
        { 字面: " " }
        { 名: 三, 类: 数字, 次: 4 }
        { 字面: " " }
        { 名: 四, 类: 数字, 次: 4 }
    ]
}

@type name: 区号电话 {
    序列: [
        { 名: 区号, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 号码, 类: 数字, 次: 8 }
    ]
}

@contract 办事人 strict {
    姓名: str
    手机: 手机号
    证件号: 身份证分组
    电话: 区号电话
}

@is 办事人
姓名: 张三
手机: "13800138000"
证件号: "221099 1988 0987 1211"
电话: "0571-23116789"
"#;

#[test]
fn 合法数据通过校验() {
    let v = ok(DOC);
    assert_eq!(v.get("姓名").and_then(|x| x.as_str()), Some("张三"));
    assert_eq!(v.get("手机").and_then(|x| x.as_str()), Some("13800138000"));
    assert_eq!(
        v.get("证件号").and_then(|x| x.as_str()),
        Some("221099 1988 0987 1211"),
        "分组身份证应原样保留"
    );
}

#[test]
fn 格式不符被拦截() {
    // 手机号少一位
    let e = err(DOC.replace(r#"手机: "13800138000""#, r#"手机: "1380013800""#));
    assert!(e.contains("手机"), "应指出字段名，实际: {e}");
    assert!(e.contains("手机号"), "应指出类型名，实际: {e}");

    // 分组身份证缺一段
    let e = err(DOC.replace(
        r#"证件号: "221099 1988 0987 1211""#,
        r#"证件号: "221099 1988 0987 121""#,
    ));
    assert!(e.contains("证件号"), "实际: {e}");

    // 区号电话缺连字符
    let e = err(DOC.replace(r#"电话: "0571-23116789""#, r#"电话: "057123116789""#));
    assert!(e.contains("电话"), "实际: {e}");
}

/// 这是整个功能的立身之本：裸写号码原本会被静默切成数字。
#[test]
fn 裸写单个号码被类型检查拦下() {
    // 单个长号裸写 → 被 coerce 成 Int，类型检查应明确指出需要引号
    let e = err(DOC.replace(r#"手机: "13800138000""#, "手机: 13800138000"));
    assert!(e.contains("引号"), "应提示用引号包裹，实际: {e}");
    assert!(e.contains("手机"), "应指出字段名，实际: {e}");
}

/// 分组号码裸写时，空格会把它切成多个键；strict 契约应拒绝而非静默丢弃。
#[test]
fn 裸写分组号码不再静默损坏() {
    let e = err(DOC.replace(
        r#"证件号: "221099 1988 0987 1211""#,
        "证件号: 221099 1988 0987 1211",
    ));
    assert!(!e.is_empty(), "分组号码裸写必须报错，不能静默通过");
}

#[test]
fn 数字类型仍按数字处理() {
    // 非 @type 的内置类型行为不受影响
    let v = ok("@contract C { port: int min 1 max 65535 }\n@is C\nport: 8080\n");
    assert_eq!(v.get("port"), Some(&sml::Value::Int(8080)));
}

#[test]
fn 未定义的类型名回落为契约引用() {
    // 未声明 @type 时，未知类型名仍按「契约引用」处理（向后兼容）
    let e = err("@contract C { addr: 不存在 }\n@is C\naddr: x\n");
    assert!(
        e.contains("未定义的契约") || e.contains("应为块"),
        "应回落为契约引用并报错，实际: {e}"
    );
}

#[test]
fn 类型可组合引用() {
    // @type 之间通过 用: 组合
    let src = r#"
@type name: 日期ISO {
    序列: [
        { 名: 年, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 月, 类: 数字, 次: 2 }
        { 字面: "-" }
        { 名: 日, 类: 数字, 次: 2 }
    ]
}

@type name: 日志行 {
    序列: [
        { 名: 日期, 用: 日期ISO }
        { 类: 空白, 次: "+" }
        { 名: 级别, 任一: [ ERROR WARN INFO ] }
        { 类: 空白, 次: "+" }
        { 名: 消息, 直到: 行尾 }
    ]
}

@contract 条目 strict {
    行: 日志行
}

@is 条目
行: "2026-09-06 INFO 系统启动"
"#;
    let v = ok(src);
    assert_eq!(
        v.get("行").and_then(|x| x.as_str()),
        Some("2026-09-06 INFO 系统启动")
    );

    // 级别不在白名单 → 拒绝
    let e = err(src.replace("2026-09-06 INFO 系统启动", "2026-09-06 TRACE 启动"));
    assert!(e.contains("行"), "实际: {e}");
}

/// 模式是线性时间匹配器，恶意输入不会拖垮解析。
#[test]
fn 病态输入不拖垮校验() {
    let src = r#"
@type name: 病态 {
    序列: [
        { 名: 前缀, 组: [ { 类: 数字, 次: "+" } ], 次: "+" }
        { 字面: "x" }
    ]
}

@contract C strict {
    值: 病态
}

@is C
值: "#
        .to_string()
        + &"1".repeat(60)
        + r#""#
        + "\n";
    let t0 = std::time::Instant::now();
    let _ = sml::parse(&src);
    let dt = t0.elapsed();
    assert!(dt.as_millis() < 1000, "耗时 {dt:?} 过长，疑似存在回溯");
}
