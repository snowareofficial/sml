// SPDX-License-Identifier: MulanPSL-2.0
//! 同一份 no_std 探针，换到 **QEMU 的 Cortex-M** 上跑。
//!
//! 与 wasm 版的分工：wasm 证「32 位 + 零 std 的语义正确」；这里多证一层 ——
//! **裸机链接（链接脚本 / 向量表 / .bss 清零）与真 ARM 核心上的执行**。
//! 输出走 semihosting，`debug::exit` 的退出码直接交给 QEMU。
//!
//! 跑法：`python tools/check/check_nostd_qemu.py`。

#![no_std]
#![no_main]

use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hprintln};

#[entry]
fn main() -> ! {
    hprintln!("nostd-probe on cortex-m: start");

    let code = nostd_probe::workflow();

    if code == 0 {
        hprintln!("nostd-probe: run() = 0 (全部通过)");
        debug::exit(debug::EXIT_SUCCESS);
    } else {
        hprintln!("nostd-probe: run() = {} (第 {} 步失败)", code, code);
        debug::exit(debug::EXIT_FAILURE);
    }

    loop {
        core::hint::spin_loop();
    }
}
