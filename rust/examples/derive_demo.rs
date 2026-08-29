// SPDX-License-Identifier: MulanPSL-2.0
//! 演示 `#[derive(SmlSerialize, SmlDeserialize)]` 的「自然」序列化。
//!
//! 运行：`cargo run --example derive_demo`

use sml::{SmlDeserialize, SmlSerialize};

/// 服务器配置块：字段名即 SML 键，`Option` 为 None 时省略。
#[derive(SmlSerialize, SmlDeserialize, Debug)]
struct Server {
    host: String,
    #[sml(default)]
    port: i32,
    #[sml(rename = "tls-enabled")]
    tls_enabled: bool,
    #[sml(skip)]
    secret: String,
    upstream: Option<String>,
}

/// 枚举单元变体 → SML 裸词（`status: active`）。
#[derive(SmlSerialize, SmlDeserialize, Debug)]
enum Status {
    Active,
    #[sml(rename = "stand-by")]
    StandBy,
}

/// 带数据变体 → 带 `__type` 的块（`shape { __type: Circle _value: 3.0 }`）。
#[derive(SmlSerialize, SmlDeserialize, Debug)]
enum Shape {
    Circle(f64),
    Rect { w: f64, h: f64 },
}

fn main() {
    let server = Server {
        host: "web.example".into(),
        port: 8080,
        tls_enabled: true,
        secret: "hunter2".into(),
        upstream: None,
    };

    // secret 是 #[sml(skip)] 字段，不参与序列化（下方文本中没有 secret 键）
    let _ = &server.secret;

    // 序列化为 SML 文本
    let text = server.to_sml();
    println!("== 序列化 ==");
    println!("{text}");

    // 从 SML 文本反序列化
    let back = Server::from_sml(&text).expect("反序列化失败");
    println!("== 反序列化 ==");
    println!("{back:?}");

    // 枚举
    println!("\nstatus: {}", Status::Active.to_sml());
    println!("shape:  {}", Shape::Circle(3.0).to_sml());
    println!("rect:   {}", Shape::Rect { w: 4.0, h: 5.0 }.to_sml());

    // 直接操作 Value
    let v = server.to_sml_value();
    println!("\n== Value ==");
    println!("{v:?}");
}
