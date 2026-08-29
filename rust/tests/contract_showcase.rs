// SPDX-License-Identifier: MulanPSL-2.0
// 验证 showcase_contract.sml 可被真实解析（文档不撒谎）
//
// 该文件位于仓库根（rust/ 之外），而 `cargo package` / `cargo publish`
// 只打包 rust/ 目录内的文件，因此打出的 crate 中**没有**这个文件。
// 故先判断存在性：仓库内正常校验，crate 内自动跳过，避免阻断发布。

use sml::parse_file;
use std::path::Path;

#[test]
fn showcase_contract_parses_and_applies() {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../showcase_contract.sml");
    if !Path::new(path).exists() {
        // 发布产物中不含仓库根的示例文件，跳过即可
        eprintln!("跳过：未找到 {path}（crate 中未包含仓库根的示例文件）");
        return;
    }
    let v = match parse_file(path) {
        Ok(v) => v,
        Err(e) => panic!("showcase_contract.sml 解析失败: {e}"),
    };

    // primary：port/tls 由 default 填充
    assert_eq!(
        v.get("database.primary.host").unwrap().as_str(),
        Some("db1.internal")
    );
    assert_eq!(
        v.get("database.primary.port"),
        Some(&sml::Value::Int(5432)),
        "primary 省略 port，应填 default 5432"
    );
    assert_eq!(
        v.get("database.primary.tls"),
        Some(&sml::Value::Bool(false)),
        "primary 省略 tls，应填 default false"
    );
    assert_eq!(
        v.get("database.primary.status").unwrap().as_str(),
        Some("active")
    );

    // replica：显式给出的值不被默认值覆盖
    assert_eq!(
        v.get("database.replica.port"),
        Some(&sml::Value::Int(5433)),
        "显式值不应被 default 覆盖"
    );
    assert_eq!(
        v.get("database.replica.tls"),
        Some(&sml::Value::Bool(true))
    );
    assert_eq!(
        v.get("database.replica.status").unwrap().as_str(),
        Some("standby")
    );

    // 组合：address 子块按 Address 契约校验，且 country 由 default 填充
    assert_eq!(
        v.get("database.primary.address.city").unwrap().as_str(),
        Some("Beijing")
    );
    assert_eq!(
        v.get("database.primary.address.country").unwrap().as_str(),
        Some("CN"),
        "子块缺失字段应填被引用契约的 default"
    );
    assert_eq!(
        v.get("database.replica.address.zip").unwrap().as_str(),
        Some("200120")
    );

    // 严格模式：Server 未声明额外字段；Metrics 标记 loose 故允许
    assert_eq!(
        v.get("metrics.latency"),
        Some(&sml::Value::Float(12.5))
    );
    assert!(
        v.get("metrics.customCounter").is_some(),
        "loose 契约应保留未声明字段"
    );
    assert_eq!(
        v.get("database.primary.prot"),
        None,
        "严格契约下的拼错字段不应被静默接受"
    );

    // 契约定义本身不应出现在解析结果中
    assert_eq!(v.get("contract"), None);
    assert_eq!(v.get("Server"), None);
    assert_eq!(v.get("Address"), None);
    assert_eq!(v.get("Metrics"), None);
}
