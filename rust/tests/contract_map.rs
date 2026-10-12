//! B 验收（TASK-agent-w24 §4）：契约映射 API。
//!
//! 钉四件事：
//! 1. **不开启时 `contract_map` 恒为 `None`**（「逐字一致」的钉子）；
//! 2. 嵌套对象块的路径 = 键序列；
//! 3. 数组元素的路径带 `Index`；
//! 4. 块级标注（typed-block）与 `to_sml()` 的端到端输出。

use sml_parse::{parse_with, ParseOptions, PathSeg, Value};

fn k(s: &str) -> PathSeg {
    PathSeg::Key(s.into())
}

/// 钉子：`trace_contracts` 不开启 ⇒ `None`。这条红了就是向后兼容被破坏。
#[test]
fn off_by_default_no_contract_map() {
    let out = parse_with("a: 1\n", ParseOptions::new()).unwrap();
    assert!(
        out.contract_map.is_none(),
        "不开启时 contract_map 必须是 None"
    );
    // 值本身不受影响
    assert_eq!(out.value.get("a"), Some(&Value::Int(1)));
}

/// 嵌套对象块：路径 = 键序列（D5 的 `database.primary` 例）。
#[test]
fn nested_object_block_path() {
    let src = "@contract Server { host: str }\n\
               database {\n\
               \x20 primary {\n\
               \x20   @is Server\n\
               \x20   host: h1\n\
               \x20 }\n\
               }\n";
    let out = parse_with(src, ParseOptions::new().trace_contracts(true)).unwrap();
    let m = out.contract_map.as_ref().unwrap();
    assert_eq!(
        m.names_at(&[k("database"), k("primary")]),
        ["Server"],
        "嵌套路径必须逐段对上"
    );
    assert_eq!(m.len(), 1, "契约定义本身不是应用，不该被记录");
}

/// 数组元素：路径带 `Index(0)`。
#[test]
fn array_element_path() {
    let src = "@contract Item { sku: str }\n\
               items: [\n\
               \x20 { @is Item\n\
               \x20   sku: s1 }\n\
               ]\n";
    let out = parse_with(src, ParseOptions::new().trace_contracts(true)).unwrap();
    let m = out.contract_map.as_ref().unwrap();
    assert_eq!(m.names_at(&[k("items"), PathSeg::Index(0)]), ["Item"]);
    assert!(m.names_at(&[k("items")]).is_empty(), "数组本身不是实体");
}

/// 块级标注：`Server db { }` 落在**首词键**下（既有语义：
/// 裸块以首词为键、`__name` 为块名），路径 = `[Key("Server")]`。
#[test]
fn typed_block_path() {
    let src = "@feature enable typed-block\n\
               @contract Server { host: str }\n\
               Server db { host: h1 }\n";
    let out = parse_with(src, ParseOptions::new().trace_contracts(true)).unwrap();
    let m = out.contract_map.as_ref().unwrap();
    // 树形状旁证：首词是键，块名进 __name
    assert_eq!(
        out.value.get("Server").and_then(|v| v.get("__name")),
        Some(&Value::Str("db".into()))
    );
    assert_eq!(m.names_at(&[k("Server")]), ["Server"]);
}

/// `to_sml()` 端到端：与解析出的映射逐字节一致（交换表示）。
#[test]
fn to_sml_end_to_end() {
    let src = "@contract Server { host: str }\n\
               database {\n\
               \x20 primary { @is Server\n\
               \x20   host: h1 }\n\
               }\n";
    let out = parse_with(src, ParseOptions::new().trace_contracts(true)).unwrap();
    assert_eq!(
        out.contract_map.unwrap().to_sml(),
        "[\n  { path: [ database primary ] contracts: [ Server ] }\n]\n"
    );
}
