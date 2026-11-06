#![no_main]
//! L2 **写路径**模糊：`pack` / `pack_with` — 输入被切成 1–4 段载荷。
//!
//! 为什么写路径要单独 fuzz（HANDOFF §4.3 第 5 位）：`amberice_load` 只碰读侧，
//! 而写侧有自己的一套算术（EMBED 段体的**顺序拼接**、`payloads[].len` 的累加、
//! 名字合法性、重名拒绝、空归档拒绝）。写坏了读侧不一定立刻发现 ——
//! 所以这里以「写出去 → 立刻读回来」的闭环为准。
//!
//! 断言：
//! 1. 自己 pack 的文件，`payloads` 的表项数 = 输入段数、每个载荷**逐字节**回来；
//! 2. 重名 ⇒ `DuplicatePayloadName`，空归档 ⇒ `EmptyArchive`（都是明确错误，不是 panic）；
//! 3. 压缩 + 分帧形态下同样成立（`payloads[]` 的 `len` 记的是**解压后**长度）；
//! 4. 主内容是 `EMBED` ⇒ `load` 必须明确报 `PrimaryNotValue`（§19 的 P8 契约）。
mod common;

use libfuzzer_sys::fuzz_target;
use amberice::{ContainerError, DumpOptions};

const MAX_PAYLOADS: usize = 4;
const MAX_PART: usize = 32 * 1024;

fuzz_target!(|data: &[u8]| {
    if data.len() > common::MAX_INPUT {
        return;
    }

    // 按 0x00 切载荷（载荷是二进制，NUL 是合法分隔符）。
    let mut parts: Vec<&[u8]> = Vec::new();
    let mut start = 0usize;
    for (i, b) in data.iter().enumerate() {
        if *b == 0 && parts.len() + 1 < MAX_PAYLOADS {
            parts.push(&data[start..i]);
            start = i + 1;
        }
    }
    parts.push(&data[start..]);
    let parts: Vec<&[u8]> = parts
        .into_iter()
        .map(|p| &p[..p.len().min(MAX_PART)])
        .collect();
    let names: Vec<String> = (0..parts.len()).map(|i| format!("p{i}.bin")).collect();

    let (_, _, bytes) = common::count_allocs(|| {
        let items: Vec<(&str, &[u8])> = names
            .iter()
            .map(|n| n.as_str())
            .zip(parts.iter().copied())
            .collect();

        // —— 1. 不压缩 ——
        match amberice::pack(&items) {
            Ok(file) => {
                let ps = amberice::payloads(&file).expect("自己 pack 的文件必须能读载荷表");
                assert_eq!(ps.len(), parts.len(), "载荷表项数必须等于输入段数");
                for (i, part) in parts.iter().enumerate() {
                    let body = amberice::payload(&file, &names[i]).expect("声明的载荷必须能取到");
                    assert_eq!(&body[..], *part, "载荷 {i} 的字节必须原样回来");
                }
                // 自查不许崩；`ok` 的判定是 CLI 的口径，不在这里替它立规矩。
                let _ = amberice::check(&file).expect("check 本身不许失败");
                // P8：pack 的 primary 是 EMBED ⇒ 整树解码必须明确拒绝。
                match amberice::load(&file) {
                    Err(ContainerError::PrimaryNotValue { .. }) => {}
                    other => panic!("pack 出的文件 load 应报 PrimaryNotValue，实得 {other:?}"),
                }
            }
            Err(e) => common::assert_container_error(&e),
        }

        // —— 2. 压缩 + 分帧（载荷长度记的是解压后长度）——
        let opts = DumpOptions {
            compress: Some(3),
            frame_size: Some(4096),
            ..Default::default()
        };
        match amberice::pack_with(&items, &opts) {
            Ok(file) => {
                for (i, part) in parts.iter().enumerate() {
                    let body = amberice::payload(&file, &names[i]).expect("压缩形态同样要能取载荷");
                    assert_eq!(&body[..], *part, "压缩形态下载荷 {i} 字节必须原样回来");
                }
            }
            Err(e) => common::assert_container_error(&e),
        }

        // —— 3. 重名：必须明确拒绝，不许后者覆盖前者 ——
        let dup = [("same.bin", parts[0]), ("same.bin", parts[0])];
        match amberice::pack(&dup) {
            Err(ContainerError::DuplicatePayloadName { .. }) => {}
            other => panic!("重名载荷应报 DuplicatePayloadName，实得 {other:?}"),
        }

        // —— 4. 空归档：必须明确拒绝（写一份没有内容的归档没有语义）——
        match amberice::pack(&[]) {
            Err(ContainerError::EmptyArchive) => {}
            other => panic!("空归档应报 EmptyArchive，实得 {other:?}"),
        }

        // —— 5. 名字直接来自输入：任意 UTF-8 都走一遍写→读 ——
        if let Ok(name) = std::str::from_utf8(data) {
            if !name.is_empty() && name.len() <= 4096 {
                match amberice::pack(&[(name, &data[..data.len().min(MAX_PART)])]) {
                    Ok(file) => {
                        let body = amberice::payload(&file, name).expect("刚写进去的名字必须能取");
                        assert_eq!(&body[..], &data[..data.len().min(MAX_PART)]);
                    }
                    Err(e) => common::assert_container_error(&e),
                }
            }
        }
    });

    common::assert_within_budget("amberice_pack", data.len(), bytes);
});
