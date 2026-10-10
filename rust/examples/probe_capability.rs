//! probe_capability.rs — 「capability / @<a,b> / enum 浮点」写法探针
//!
//! 目的：把用户提案里的每一处写法拿到当前实现上真跑一遍，得到**事实**而非推测。
//! 只读探针，不改变任何语义。
use sml::{parse, to_sml};

fn probe(label: &str, src: &str) {
    println!("=== {label} ===");
    match parse(src) {
        Ok(v) => {
            let text = to_sml(&v);
            println!("[OK]\n{}", text.trim_end());
        }
        Err(e) => println!("[ERR] {e}"),
    }
    println!();
}

fn main() {
    // 让 $env.MAIL_TLS_CERT 有值，避免「未定义环境变量」掩盖其它错误
    std::env::set_var("MAIL_TLS_CERT", "/etc/ssl/mail.pem");

    // A. 用户原文（@version 4a）
    probe(
        "A 原文（@version 4a）",
        r#"@version 4a
@contract capability tls {
  provides: [encryption-in-transit]
  requires: [cert_ref: str required]
  cert_ref: str
  min_tls_version: enum [1.2 1.3] default 1.3
}

@contract capability retry {
  provides: [transient-error-recovery]
  max_attempts: int default 3 min 1
  backoff_ms: int default 200 min 0
}

sender mail {
  @<tls, retry>
  cert_ref: $env.MAIL_TLS_CERT
  max_attempts: 5
}
"#,
    );

    // B. 只把版本改回 4，其余原文
    probe(
        "B 原文但 @version 4",
        r#"@version 4
@contract capability tls {
  provides: [encryption-in-transit]
  requires: [cert_ref: str required]
  cert_ref: str
  min_tls_version: enum [1.2 1.3] default 1.3
}

@contract capability retry {
  provides: [transient-error-recovery]
  max_attempts: int default 3 min 1
  backoff_ms: int default 200 min 0
}

sender mail {
  @<tls, retry>
  cert_ref: $env.MAIL_TLS_CERT
  max_attempts: 5
}
"#,
    );

    // C. 双词契约名：capability tls
    probe(
        "C 双词契约名 `capability tls`",
        r#"@version 4
@contract capability tls {
  cert_ref: str
}
"#,
    );

    // D. 单名契约 + @is 应用（基线写法，用于对照）
    probe(
        "D 单名契约 + @is（对照）",
        r#"@version 4
@contract tls {
  cert_ref: str required
  min_tls_version: enum [1.2 1.3] default 1.3
}

mail {
  @is tls
  cert_ref: "pem"
}
"#,
    );

    // E. 枚举里带小数：值恰好命中列表，能否通过？
    probe(
        "E enum [1.2 1.3] 显式填 1.3（命中列表）",
        r#"@version 4
@contract tls {
  min_tls_version: enum [1.2 1.3] default 1.3
}

mail {
  @is tls
  min_tls_version: 1.3
}
"#,
    );

    // F. 只吃默认值（不显式写）是否能过
    probe(
        "F enum 默认值 1.3（不显式写）",
        r#"@version 4
@contract tls {
  min_tls_version: enum [1.2 1.3] default 1.3
}

mail {
  @is tls
}
"#,
    );

    // G. requires/provides 用「裸词数组」vs「类型数组」
    probe(
        "G `provides: [str]`（类型数组）",
        r#"@version 4
@contract Cap {
  provides: [str]
}
"#,
    );
    probe(
        "G2 `provides: [encryption-in-transit]`（裸词数组）",
        r#"@version 4
@contract Cap {
  provides: [encryption-in-transit]
}
"#,
    );
    probe(
        "G3 `requires: [cert_ref: str required]`（数组内塞字段规格）",
        r#"@version 4
@contract Cap {
  requires: [cert_ref: str required]
}
"#,
    );

    // H. `@<a, b>` 组合语法
    probe(
        "H `@<tls, retry>` 组合",
        r#"@version 4
@contract tls { cert_ref: str required }
@contract retry { max_attempts: int default 3 }

sender "mail" {
  @<tls, retry>
  cert_ref: "pem"
  max_attempts: 5
}
"#,
    );

    // H2. 裸块参数 `sender mail`（不引号）
    probe(
        "H2 `sender mail { }` 裸块参数",
        r#"@version 4
sender mail {
  x: 1
}
"#,
    );
    probe(
        "H2b `sender mail { }`（@version 1）",
        r#"@version 1
sender mail {
  x: 1
}
"#,
    );

    // H3. `@is a b` 空格分隔的两个契约名
    probe(
        "H3 `@is tls retry`",
        r#"@version 4
@contract tls { cert_ref: str required }
@contract retry { max_attempts: int default 3 }

sender "mail" {
  @is tls retry
  cert_ref: "pem"
  max_attempts: 5
}
"#,
    );

    // E2. enum 浮点：改用引号串能否绕过
    probe(
        "E2 enum [1.2 1.3] 填 \"1.3\"（加引号）",
        r#"@version 4
@contract tls {
  min_tls_version: enum [1.2 1.3] default 1.3
}

mail {
  @is tls
  min_tls_version: "1.3"
}
"#,
    );

    // E3. enum 里写成字符串字面量
    probe(
        "E3 enum [\"1.2\" \"1.3\"] 填裸 1.3",
        r#"@version 4
@contract tls {
  min_tls_version: enum ["1.2" "1.3"] default 1.3
}

mail {
  @is tls
  min_tls_version: 1.3
}
"#,
    );

    // I. $env.X 作字段值（基线能力）
    probe(
        "I `cert_ref: $env.MAIL_TLS_CERT`（基线能力）",
        r#"@version 4
cert_ref: $env.MAIL_TLS_CERT
"#,
    );

    // K. 带点的契约名（点是否为词内字符）
    probe(
        "K `@contract capability.tls` 点号名",
        r#"@version 4
@contract capability.tls { cert_ref: str required }

mail {
  @is capability.tls
  cert_ref: "pem"
}
"#,
    );

    // L. 引号契约名（含空格）
    probe(
        "L `@contract \"capability tls\"` 引号名",
        r#"@version 4
@contract "capability tls" { cert_ref: str required }

mail {
  @is "capability tls"
  cert_ref: "pem"
}
"#,
    );

    // M. 同一块内两次 @is（后一个覆盖还是报错？）
    probe(
        "M 同一块内两条 @is",
        r#"@version 4
@contract tls { cert_ref: str required }
@contract retry { max_attempts: int default 3 }

mail {
  @is tls
  @is retry
  cert_ref: "pem"
  max_attempts: 5
}
"#,
    );

    // N. 组合的「零新语法」候选：契约内嵌子契约字段（ContractRef）
    probe(
        "N 契约组合：子块字段引用另一契约",
        r#"@version 4
@contract tls { cert_ref: str required }
@contract retry { max_attempts: int default 3 }
@contract mailCap {
  tls: tls
  retry: retry
}

sender "mail" {
  @is mailCap
  tls { cert_ref: "pem" }
  retry { max_attempts: 5 }
}
"#,
    );

    // J. typed-block：`tls mail { }` 免 @is
    probe(
        "J typed-block（需 @feature enable typed-block）",
        r#"@version 4
@feature enable typed-block
@contract tls { cert_ref: str required }

tls mail {
  cert_ref: "pem"
}
"#,
    );
}
