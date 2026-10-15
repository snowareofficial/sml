// emit_ddl.rs — 原型：把 SML schema 发射成 SQL DDL（SQLite 方言）。
//
// 目的：验证「SML 当数据库前端」最小闭环是否成立——人写 SML schema，
// 发射 CREATE TABLE。仅原型，未接入 emit 体系、无迁移/多方言。
//
// schema 约定（由解析实测决定）：
//   * 顶层每个块 = 一张表（块名即表名）；块内字段 = 列。
//   * `&base` 由**解析器**负责拼接（无需本程序处理），共享列自动并入各表；
//     `@base { }` 定义块本身不会作为顶层键出现。
//   * 每列的值是**数组** `[ 类型 修饰符... ]`：
//       - 实测 `id: int primary`（尾随裸词）会 E-PARSE-006，故必须数组包裹；
//       - `-> 表.列` 会变成独立键、挂不到列上，故外键用数组里的 `references 表.列`。
//   修饰符：primary/pk · unique · not_null · default <v> · references <表.列>
//           · enum <v...>（类型位，其余 token 全为枚举值）· min <n> · max <n>
//
// 运行：cargo run --example emit_ddl
use sml::{parse, Value};
use std::collections::BTreeMap;

fn sql_type(t: &str) -> &'static str {
    match t {
        "int" | "integer" | "bool" => "INTEGER",
        "real" | "num" | "float" | "double" => "REAL",
        _ => "TEXT", // text/str/uuid/timestamp/… 一律 TEXT（SQLite 动态类型）
    }
}

// 标量统一转字符串：数组里数字（如 min 0 的 0）是 Value::Int，不能只用 as_str() 丢。
fn scalar_str(v: &Value) -> Option<String> {
    match v {
        Value::Str(s) => Some(s.clone()),
        Value::Int(n) => Some(n.to_string()),
        Value::Float(f, _) => Some(f.to_string()),
        Value::Bool(b) => Some(b.to_string()),
        _ => None,
    }
}

// 把一列的值（数组或裸串）解释成 SQL 列定义。
fn column_sql(name: &str, v: &Value) -> String {
    let toks: Vec<String> = match v {
        Value::Str(s) => vec![s.clone()],
        Value::Array(a) => a.iter().filter_map(scalar_str).collect(),
        _ => vec!["TEXT".into()],
    };
    let ty = toks.first().cloned().unwrap_or_else(|| "TEXT".into());

    // enum 在「类型位」：`[ enum a b c ]` 首 token 即 enum，其余全是取值。
    if ty == "enum" {
        let vals: Vec<String> = toks[1..].iter().map(|s| format!("'{}'", s)).collect();
        return format!("  {} TEXT CHECK ({} IN ({}))", name, name, vals.join(", "));
    }

    let mut parts = vec![sql_type(&ty).to_string()];
    let m = &toks[1..];
    let mut i = 0;
    while i < m.len() {
        match m[i].as_str() {
            "primary" | "pk" => parts.push("PRIMARY KEY".into()),
            "unique" => parts.push("UNIQUE".into()),
            "not_null" | "notnull" => parts.push("NOT NULL".into()),
            "references" => {
                if let Some(t) = m.get(i + 1) {
                    // users.id -> users(id)
                    let fk = if let Some(dot) = t.find('.') {
                        format!("{}({})", &t[..dot], &t[dot + 1..])
                    } else {
                        t.clone()
                    };
                    parts.push(format!("REFERENCES {}", fk));
                    i += 1;
                }
            }
            "default" => {
                if let Some(d) = m.get(i + 1) {
                    parts.push(format!("DEFAULT '{}'", d));
                    i += 1;
                }
            }
            "min" => {
                if let Some(n) = m.get(i + 1) {
                    parts.push(format!("CHECK ({} >= {})", name, n));
                    i += 1;
                }
            }
            "max" => {
                if let Some(n) = m.get(i + 1) {
                    parts.push(format!("CHECK ({} <= {})", name, n));
                    i += 1;
                }
            }
            _ => {} // 未知修饰符：原型先忽略（真实版应报错/告警）
        }
        i += 1;
    }
    format!("  {} {}", name, parts.join(" "))
}

fn main() {
    let schema = r#"
@base { id: [ int primary ], created_at: [ text default now ] }

users {
    &base
    email: [ text unique not_null ]
    role: [ enum admin member guest ]
}

orders {
    &base
    user_id: [ int references users.id ]
    total: [ real min 0 ]
    status: [ text default pending ]
}
"#;

    let doc = match parse(schema) {
        Ok(v) => v,
        Err(e) => { eprintln!("parse failed [{}]: {}", e.code(), e.message()); return; }
    };
    let tables: &BTreeMap<String, Value> = match &doc {
        Value::Object(m) => m,
        _ => { eprintln!("schema 顶层必须是对象（每块一表）"); return; }
    };

    let mut ddl = String::new();
    for (tname, tdef) in tables {
        let cols = match tdef {
            Value::Object(m) => m,
            _ => continue,
        };
        let lines: Vec<String> = cols.iter().map(|(c, v)| column_sql(c, v)).collect();
        ddl.push_str(&format!("CREATE TABLE {} (\n{}\n);\n\n", tname, lines.join(",\n")));
    }
    print!("{}", ddl);
}
