//! featag **S4**：契约内的 featag **声明收集** + 声明处查重 + 落点路径校验。
//!
//! 片界：S4 只「收集 + 查重」，**不做**「未声明即拒」与参数类型校验（那是 S5）。
//! 因此本文件的用例全是**声明侧**的行为；使用侧（`__featags`）由 `featag_syntax.rs` 覆盖。
use sml::parse;

const ON: &str = "@feature enable type.featag\n";

fn err_of(src: &str) -> String {
    format!("{}", parse(src).unwrap_err())
}

#[test]
fn declarations_are_accepted() {
    // 四种写法一次过：无参 / 带参 / 带落点 / 点路径落点
    let src = format!(
        "{ON}@contract wea {{
  a: int
  !critical
  !obey: [int]
  !tls: [str] -> tls
  !cert: str -> tls.cert
}}
"
    );
    assert!(parse(&src).is_ok(), "声明应被接受：{:?}", parse(&src).err());
}

#[test]
fn duplicate_declaration_errors() {
    let src = format!("{ON}@contract wea {{\n  a: int\n  !obey: [int]\n  !obey: [int]\n}}\n");
    let err = err_of(&src);
    assert!(err.contains("E-CONTRACT-019"), "应报重复声明：{err}");
    assert!(err.contains("!obey"), "文案应点名是哪个 featag：{err}");
}

#[test]
fn target_may_not_index_into_arrays() {
    let src = format!("{ON}@contract wea {{\n  a: int\n  !x: [int] -> node[0]\n}}\n");
    let err = err_of(&src);
    assert!(err.contains("E-CONTRACT-020"), "落点穿数组应报错：{err}");
    assert!(err.contains("数组"), "文案应说清原因：{err}");
}

#[test]
fn malformed_target_path_is_rejected() {
    for bad in ["a..b", "a.", "[0]"] {
        let src = format!("{ON}@contract wea {{\n  a: int\n  !x: [int] -> {bad}\n}}\n");
        let err = err_of(&src);
        assert!(
            err.contains("E-CONTRACT-020"),
            "落点 `{bad}` 应报 E-CONTRACT-020：{err}"
        );
    }
}

#[test]
fn empty_declaration_name_errors() {
    let src = format!("{ON}@contract wea {{\n  a: int\n  ! : int\n}}\n");
    let err = err_of(&src);
    assert!(err.contains("E-PARSE-026"), "空 featag 名应报错：{err}");
}

// ---------------------------------------------------------------------------
// 标识符规则（`lex.ident`）也覆盖声明处
// ---------------------------------------------------------------------------

#[test]
fn featag_names_are_identifiers() {
    let src = "@feature enable type.featag,lex.ident\n@contract wea {\n  a: int\n  !a.b: int\n}\n";
    let err = err_of(src);
    assert!(err.contains("E-LEX-007"), "featag 名应受标识符规则管：{err}");
}

#[test]
fn field_names_are_identifiers() {
    let src = "@feature enable lex.ident\n@contract wea {\n  \"a.b\": int\n}\n";
    let err = err_of(src);
    assert!(err.contains("E-LEX-007"), "字段名应受标识符规则管：{err}");
}

#[test]
fn one_contract_must_use_a_single_script() {
    // 拉丁 + 汉字混排 ⇒ 同形字/可读性风险的机制化解法（§2.2）
    let src = "@feature enable lex.ident\n@contract wea {\n  port: int\n  端口: str\n}\n";
    let err = err_of(src);
    assert!(err.contains("E-LEX-007"), "混用语种应报错：{err}");
    assert!(err.contains("语种"), "文案应点明是语种问题：{err}");

    // 纯汉字或纯拉丁都没问题
    assert!(parse("@feature enable lex.ident\n@contract wea {\n  端口: str\n  名称: str\n}\n").is_ok());
    assert!(parse("@feature enable lex.ident\n@contract wea {\n  port: int\n  host: str\n}\n").is_ok());
    // 汉字 + 假名属同一族（日文正常写法）
    assert!(parse("@feature enable lex.ident\n@contract wea {\n  設定: str\n  ファイル: str\n}\n").is_ok());
}

#[test]
fn mixed_script_is_fine_without_the_ident_feature() {
    // 门控关掉 ⇒ 一字不改（既有契约里中英混排很常见）
    let v = parse("@contract wea {\n  port: int\n  端口: str\n}\nwea x { port: 1 端口: a }\n");
    assert!(v.is_ok(), "未开 lex.ident 时不应报语种错：{:?}", v.err());
}
