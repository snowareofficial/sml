// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0

//! include 指令演示：主文件引入共享片段，形成一份完整配置。
//!
//! 运行：
//!   cargo run --example include_demo
//!
//! 启用 serde 后额外打印 JSON：
//!   cargo run --example include_demo --features serde

use sml::{parse_file, to_sml};
use std::fs;

fn main() {
    // 演示目录放在临时目录，避免污染工作区
    let dir = std::env::temp_dir().join("sml_include_demo");
    let _ = fs::remove_dir_all(&dir);
    fs::create_dir_all(dir.join("conf.d")).expect("create demo dir");

    // 共享片段：数据库与缓存的公共配置
    fs::write(
        dir.join("conf.d/common.sml"),
        "region: cn-north-1\nretry: 3\n",
    )
    .unwrap();

    // 被嵌套引入的片段（相对自身目录解析）
    fs::write(dir.join("conf.d/db.sml"), "include \"common.sml\"\nhost: db.internal\nport: 5432\n").unwrap();

    // 主文件：顶层字段 + 块内 include
    fs::write(
        dir.join("app.sml"),
        "app: resender\n\n\
         database {\n\
         \x20   include \"conf.d/db.sml\"\n\
         \x20   pool: 16\n\
         }\n\n\
         cache {\n\
         \x20   include \"conf.d/common.sml\"\n\
         \x20   ttl: 300\n\
         }\n",
    )
    .unwrap();

    let path = dir.join("app.sml");
    println!("--- app.sml ---");
    println!("{}", fs::read_to_string(&path).unwrap());

    let value = match parse_file(&path) {
        Ok(v) => v,
        Err(e) => {
            eprintln!("parse error: {e}");
            std::process::exit(1);
        }
    };

    println!("--- resolved ---");
    println!("{}", to_sml(&value));

    println!("--- query ---");
    println!("app      = {:?}", value.get("app").and_then(|v| v.as_str()));
    println!("db.host  = {:?}", value.get("database.host").and_then(|v| v.as_str()));
    println!("db.port  = {:?}", value.get("database.port"));
    println!("db.region= {:?}", value.get("database.region").and_then(|v| v.as_str()));
    println!("cache.ttl= {:?}", value.get("cache.ttl"));

    // 同一个 fragment 被两处引用，验证 include 可重复引入
    assert_eq!(
        value.get("database.region").and_then(|v| v.as_str()),
        Some("cn-north-1")
    );
    assert_eq!(
        value.get("cache.region").and_then(|v| v.as_str()),
        Some("cn-north-1"),
        "common.sml 应能被多处引用"
    );

    #[cfg(feature = "serde")]
    {
        println!("--- json (serde) ---");
        println!("{}", serde_json::to_string_pretty(&value).unwrap());
    }

    let _ = fs::remove_dir_all(&dir);
}
