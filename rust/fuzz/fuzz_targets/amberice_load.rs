#![no_main]
//! L2 容器层模糊：任意字节当作 `.ambr`，把**所有只读入口**都撞一遍。
//!
//! 覆盖（HANDOFF §4.2 L2）：`info` / `check` / `load` / `base_value` / `payloads` /
//! `payload` / `Store::open`（含摘要校验）/ `Store::open_unverified` / `parse_frame_index`
//! / 层链（`verify_chain` + `verify`）。
//!
//! 最强的一条断言是**往返固定点**（HANDOFF §4.5 第 4 条）：
//! `load` 成功 ⇒ `dump` ⇒ 再 `load` ⇒ 值与键序不变、再 `dump` 逐字节相同。
//! 它一次就能抓到「能解析但不能重写」的隐性 bug。
//!
//! 不断言「重写 == 原始输入」：合法文件可以带 `EMBED` 载荷、补丁层、压缩段，
//! 重写只会得到**值**，字节必然不同（HANDOFF §6「已知边界」）。收敛性用固定点判定。
//!
//! 分配额度按**每次调用**核对（见 `common::check_call`）：压缩段合法解压上限由
//! 库自己的 `MAX_DECOMPRESSED` 决定，不该被 fuzz 判成 OOM。
mod common;

use libfuzzer_sys::fuzz_target;
use amberice::HashAlgo;
use sml_value::CanonicalValue;

/// 取值路径：固定集合（路径解析 + 零拷贝导航的常见分支）。
const PATHS: &[&str] = &["a", "a.b", "[0]", "a[0]", "", ".", "a..b"];

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }
    let len = data.len();
    let quota = common::decompression_allowance(data);

    // —— 头 / 段表 ——
    if let Err(e) = common::check_call("info", len, quota, || amberice::info(data)) {
        common::assert_container_error(&e);
    }

    // —— 自描述校验（`ok = false` 也是正常结果，不是 Err）——
    match common::check_call("check", len, quota, || amberice::check(data)) {
        Ok(report) => assert!(report.lines.len() < 100_000, "check 报告行数异常"),
        Err(e) => common::assert_container_error(&e),
    }

    // —— 整树解码 + 往返固定点 ——
    match common::check_call("load", len, quota, || amberice::load(data)) {
        Ok(v) => {
            let cv = match CanonicalValue::try_from(&v) {
                Ok(cv) => cv,
                Err(e) => panic!("load 出的值不是 canonical 形态：{e}"),
            };
            let b1 = match common::check_call("dump", len, 0, || amberice::dump(&cv)) {
                Ok(b) => b,
                Err(e) => panic!("load 成功却 dump 不出来：{e}"),
            };
            let v1 = match amberice::load(&b1) {
                Ok(v) => v,
                Err(e) => panic!("自己 dump 的文件 load 不回来：{e}"),
            };
            assert!(v.eq_ordered(&v1), "load→dump→load 值/键序变了");

            let cv1 = match CanonicalValue::try_from(&v1) {
                Ok(cv) => cv,
                Err(e) => panic!("二次 load 出的值不是 canonical 形态：{e}"),
            };
            let b2 = match amberice::dump(&cv1) {
                Ok(b) => b,
                Err(e) => panic!("二次 dump 失败：{e}"),
            };
            assert_eq!(b1, b2, "dump 不是固定点（重复写会漂字节）");

            // 随机访问视图必须与整树解码给出同一个值。
            match amberice::Store::open_unverified(data) {
                Ok(s) => match s.value() {
                    Ok(sv) => assert!(sv.eq_ordered(&v), "Store::value 与 load 值不一致"),
                    Err(e) => common::assert_container_error(&e),
                },
                Err(e) => common::assert_container_error(&e),
            }
        }
        Err(e) => common::assert_container_error(&e),
    }

    // —— 基文档（不套补丁层）——
    if let Err(e) = common::check_call("base_value", len, quota, || amberice::base_value(data)) {
        common::assert_container_error(&e);
    }

    // —— 载荷表：声明的每个名字都必须能取（或明确报错）——
    match common::check_call("payloads", len, quota, || amberice::payloads(data)) {
        Ok(ps) => {
            assert!(ps.len() < 100_000, "载荷表项数异常");
            for p in &ps {
                match amberice::payload(data, &p.name) {
                    Ok(body) => assert_eq!(body.len(), p.len as usize, "载荷长度与 manifest 不一致"),
                    Err(e) => common::assert_container_error(&e),
                }
            }
        }
        Err(e) => common::assert_container_error(&e),
    }

    // —— 随机访问（打开时只解析头 + 段表 + manifest）——
    match common::check_call("Store::open", len, quota, || amberice::Store::open(data)) {
        Ok(s) => {
            for p in PATHS {
                if let Err(e) = s.get(p) {
                    common::assert_container_error(&e);
                }
            }
            for p in s.payloads() {
                let _ = s.payload(&p.name);
            }
            // 分帧文件的「整树」路径：逐帧解压，必须与零拷贝视图自洽。
            let _ = s.value();
        }
        Err(e) => common::assert_container_error(&e),
    }

    // —— 帧索引：段体格式自描述，畸形必须报 BadFrameIndex 而不是 panic ——
    if let Err(e) = common::check_call("parse_frame_index", len, quota, || {
        amberice::container::parse_frame_index(data)
    }) {
        common::assert_container_error(&e);
    }

    // —— 层链：读链 → 校验链 → 验签（错误路径不许 panic）——
    if let Err(e) = amberice::layer::verify_chain(data, HashAlgo::Sha256) {
        assert!(!e.to_string().is_empty(), "层链错误文案为空");
    }
    let _ = amberice::verify(data, HashAlgo::Sha256);
    let _ = amberice::layer::chain_head(data, HashAlgo::Sm3);
});
