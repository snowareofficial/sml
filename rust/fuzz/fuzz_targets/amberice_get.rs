#![no_main]
//! L2 路径取值的模糊：**文件字节**与**路径文本**同时由输入控制。
//!
//! 为什么单独一个 target（HANDOFF §4.3 第 2 位）：`Store::get` 是零拷贝导航
//! （按 `subtree_len` 跳兄弟节点 + 只解命中子树），越界 / 长度算错的风险集中在
//! 这条路径上，而 `amberice_load` 走的是整树解码，撞不到它。
//!
//! 输入布局：`<文件字节> 0x00 <路径文本>`。没有 `0x00` 时全部当文件（路径用固定集合）。
//!
//! 最强断言：**两条实现必须同答** —— 零拷贝的 `Store::get` 与
//! 「整树解码 + `get_path`」的 `amberice::get` 在同一个 (文件, 路径) 上若都成功，
//! 值必须相等；若一个成功一个失败，那就是两者语义漂了。
mod common;

use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;

/// 固定路径集：点号 / 下标 / 空段 / 越界 / 深路径。
const PATHS: &[&str] = &[
    "a", "a.b", "a[0]", "[0]", "0", "a.0", "a..b", "a.", "", "a[", "a[-1]", "a.b.c[0].d",
];

/// 固定文档：只在进程内编一次，用来单独 fuzz「路径文本」这一半输入。
fn fixed_doc() -> &'static Vec<u8> {
    static DOC: OnceLock<Vec<u8>> = OnceLock::new();
    DOC.get_or_init(|| {
        let v = sml_parse::parse_ordered(
            "a {\n  b: [1 2 3]\n  c: \"x\"\n}\nd: [1 2 3]\ne: null\n",
        )
        .expect("固定文档必须能解析");
        let cv = amberice::CanonicalValue::try_from(&v).expect("固定文档必须是 canonical 形态");
        amberice::dump(&cv).expect("固定文档必须能 dump")
    })
}

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }

    let (file, path_input) = match data.iter().position(|b| *b == 0) {
        Some(i) => (&data[..i], Some(&data[i + 1..])),
        None => (data, None),
    };
    let len = file.len();
    let quota = common::decompression_allowance(file);

    // —— A. 任意文件 + 固定路径：越界 / 截断 / 段表畸形都从这里进 ——
    //
    // 交叉验证只在「整树解码也成功」时做：`load` 是整文件级的闸门
    // （能力位 `caps.required` 拒绝、加密段缺钥等），它失败说明这份文件
    // 根本不该被读，不能拿来判两套实现的路径语义。
    let store = common::check_call("Store::open", len, quota, || amberice::Store::open(file));
    if let (Ok(s), Ok(full_doc)) = (&store, common::check_call("load", len, quota, || {
        amberice::load(file)
    })) {
        for p in PATHS {
            let zero = common::check_call("Store::get", len, quota, || s.get(p));
            let full = common::check_call("amberice::get", len, quota, || amberice::get(file, p));
            match (zero, full) {
                (Ok(zero_copy), Ok(full)) => assert!(
                    zero_copy.eq_ordered(&full),
                    "Store::get 与 amberice::get 结果不一致（路径 {p:?}）"
                ),
                // 两套实现里有一个把这份文件当成「可读」、另一个不是 ⇒ 语义漂移。
                // 但 `load` 已成功时，`amberice::get` 必然也走到同一套闸门 ⇒ 只可能是 Store 单边失败/成功。
                (Ok(_), Err(e)) | (Err(e), Ok(_)) => panic!(
                    "Store::get 与 amberice::get 一个成功一个失败（路径 {p:?}）：{e}"
                ),
                (Err(e), Err(_)) => common::assert_container_error(&e),
            }
        }
        // 整树也必须同答（`Store::value` 走补丁合并，`load` 也走）。
        match common::check_call("Store::value", len, quota, || s.value()) {
            Ok(sv) => assert!(sv.eq_ordered(&full_doc), "Store::value 与 load 结果不一致"),
            Err(e) => common::assert_container_error(&e),
        }
    } else if let Ok(s) = &store {
        // 打不开整树时，仍要保证随机访问本身不 panic、错误可分类。
        for p in PATHS {
            if let Err(e) = common::check_call("Store::get", len, quota, || s.get(p)) {
                common::assert_container_error(&e);
            }
        }
    }

    // —— B. 任意路径文本 + 固定文档：路径语法 / 下标解析 / 类型不匹配 ——
    if let Some(raw) = path_input {
        if let Ok(p) = std::str::from_utf8(raw) {
            let doc = fixed_doc();
            let store = amberice::Store::open(doc).expect("固定文档必须能打开");
            match (store.get(p), amberice::get(doc, p)) {
                (Ok(zero_copy), Ok(full)) => {
                    assert!(zero_copy.eq_ordered(&full), "路径 {p:?}：零拷贝与整树解码结果不一致")
                }
                (Ok(_), Err(e)) | (Err(e), Ok(_)) => {
                    panic!("路径 {p:?}：零拷贝与整树解码一个成功一个失败：{e}")
                }
                (Err(e), Err(_)) => common::assert_container_error(&e),
            }

            // 路径语法解析本身也必须分类正确（非法段 ⇒ Err，不许 panic）。
            if let Err(e) = sml_value::Path::parse(p) {
                assert!(!e.to_string().is_empty(), "路径错误文案为空");
            }
        }
    }
});
