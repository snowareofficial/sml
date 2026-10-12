#![no_main]
//! 解析器模糊（libFuzzer / 覆盖制导）：任意 UTF-8 文本喂给 `sml::parse`，
//! 验证其**不 panic / 不 abort / 不卡死 / 不 OOM**。
//!
//! 库内已有若干闸门（值嵌套深度 128、模式量词 `QUANT_MAX`、编译指令总数
//! `MAX_PROG`、include 深度/展开次数）；本 target 用覆盖制导的 libFuzzer
//! 持续逼近这些边界，捕捉人工想不到的病态输入。非 UTF-8 输入跳过 ——
//! SML 是 UTF-8 文本格式，非 UTF-8 字节不进入文本解析路径。
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = sml_parse::parse(s);
    }
});
