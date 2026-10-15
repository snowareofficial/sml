//! 验证「空契约当标签」：空契约不注入任何字段，只被 ContractMap 记录（零新机制）。

use sml::parse;

const SRC: &str = "\
@feature enable typed-block
@feature enable block-name
@contract Security { }
@contract Rule { action: enum [ allow deny ] }

Rule Security ssh {
    action: allow
}
";

#[test]
fn empty_contract_works_as_a_tag_without_polluting_data() {
    let v = parse(SRC).expect("空契约应可定义并叠加");
    println!("{v:#?}");
    // 叠加两份契约：Security（空）+ Rule。
    // 结果树里不应出现 Security 注入的任何字段 —— 空契约 = 纯标签。
    // ⚠️ typed block 的键是**首词**（类型名 `Rule`），实例名 `ssh` 在 `__name` 里
    let block = v.get("Rule").expect("typed block 以首词为键");
    assert_eq!(block.get("__name").and_then(|x| x.as_str()), Some("ssh"));
    let keys: Vec<_> = match block {
        sml::Value::Object(m) => m.keys().cloned().collect(),
        _ => panic!("应为对象"),
    };
    assert!(
        !keys.iter().any(|k| k == "Security"),
        "空契约不应往数据里注入字段，实际键：{keys:?}"
    );
    // 只剩契约声明的 action 与语法元数据 __type / __name
    assert!(keys.contains(&"action".to_string()), "实际键：{keys:?}");
    assert!(keys.contains(&"__name".to_string()), "实际键：{keys:?}");
}

const TYPO: &str = "\
@contract Rule { action: enum [ allow deny ] }
Rule Securty ssh {
    action: allow
}
";

/// 拆雷后：未开 `block-name` 时，typed-block 模式下认不出来的词**直接报错**。
/// 原先拼错的 `Securty` 会被静默当成块名（契约写错＝静默失效），现在立刻暴露。
#[test]
fn typo_contract_name_errors_without_block_name() {
    let src = format!("@feature enable typed-block\n{TYPO}");
    let err = parse(&src).unwrap_err().to_string();
    assert!(
        err.contains("block-name"),
        "应提示需启用 block-name，实际报错：{err}"
    );
    assert!(err.contains("E-CONTRACT-001"), "实际报错：{err}");
}

/// 开了 `block-name` 才恢复旧行为：首个非契约词当块名。
/// 此时拼错**仍会被吞** —— 这是显式 opt-in 的代价，写在文档里让人知情。
#[test]
fn typo_contract_name_absorbed_when_block_name_enabled() {
    let src = format!("@feature enable typed-block\n@feature enable block-name\n{TYPO}");
    let v = parse(&src).expect("开启 block-name 后非契约词作块名");
    let block = v.get("Rule").unwrap();
    assert_eq!(block.get("__name").and_then(|x| x.as_str()), Some("Securty"));
    assert!(block.get("__args").is_some(), "ssh 被挤进 __args");
}

/// 普通裸块（首词未命中契约表）不受 `block-name` 影响 —— 回归红线
/// `server web prod { }` ⇒ __type: server / __name: web / __args: [prod]。
#[test]
fn plain_bare_block_keeps_legacy_naming() {
    let src = "@contract Rule { action: enum [ allow deny ] }\nserver web prod { port: 8080 }\n";
    let v = parse(src).expect("普通裸块不应受 block-name 影响");
    let block = v.get("server").unwrap();
    assert_eq!(block.get("__type").and_then(|x| x.as_str()), Some("server"));
    assert_eq!(block.get("__name").and_then(|x| x.as_str()), Some("web"));
    assert!(block.get("__args").is_some());
}
