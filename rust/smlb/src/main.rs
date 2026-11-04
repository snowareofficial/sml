// SPDX-License-Identifier: MulanPSL-2.0
//! The `smlb` command line (T0 has exactly one subcommand: `vectors`).
//! `smlb` 命令行（T0 只有一个子命令：`vectors`）。
//!
//! Hand-rolled argument parsing, **no `clap`**: this crate's charter is “depends only on the
//! value model” (D3), and T0 has a single `vectors --gen/--check [--file PATH]` — no framework
//! needed. Revisit when T6's tool surface (`info`/`ls`/`cat`/`verify`/`append`…) grows.
//! 手写参数解析而**不引 `clap`**：定位是「只依赖值模型」（D3），T0 用不上框架。
//!
//! Exit codes (scripts depend on them) / 退出码约定：
//! - `0` success ｜ 成功；
//! - `1` failure (mismatch / IO / canonical error) ｜ 失败；
//! - `2` usage error (unknown subcommand, missing arg, `--gen` and `--check` together)
//!   ｜ 用法错误。

use std::path::PathBuf;
use std::process::ExitCode;

use smlb::vectors;

const USAGE: &str = "\
smlb — SML 的二进制形式 .smlb（T0：canonical 与测试向量）

用法:
  smlb vectors --gen   [--file PATH]   重新生成向量文件
  smlb vectors --check [--file PATH]   重新生成并与磁盘逐行比对
  smlb --help | -h                     本帮助
  smlb --version | -V                  版本

说明:
  默认向量文件: tests/vectors/smlb-canonical.v1.json（自动向上找两级）
  `--check` 会先跑自检：每例 bytes → load → bytes 必须逐字节相同，
  且 text(load(bytes)) 必须等于原 text —— 否则签名与字节会各说各话。

  本子命令需要 sml-value 的 preserve_order（spec §18 C2：键序 = 源序）：
    cargo run -p smlb --features preserve_order -- vectors --check

T1 起将增加编码器与段表；T2 起增加 info / ls / check / pack / unpack；
T4 起增加 sign / verify / append / tree（顺序见 spec §17）。";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err((code, msg)) => {
            eprintln!("smlb: {msg}");
            if !cfg!(feature = "preserve_order") {
                eprintln!(
                    "smlb: hint: 本次构建未开 `preserve_order`（spec §18 C2 要求键序 = 源序）\n\
                     smlb:       改用 `cargo run -p smlb --features preserve_order -- vectors --check`"
                );
            }
            ExitCode::from(code)
        }
    }
}

fn run(args: &[String]) -> Result<String, (u8, String)> {
    let Some(cmd) = args.first() else {
        return Err((2, format!("缺少子命令\n\n{USAGE}")));
    };
    match cmd.as_str() {
        "-h" | "--help" | "help" => Ok(USAGE.to_string()),
        "-V" | "--version" => Ok(format!("smlb {}", env!("CARGO_PKG_VERSION"))),
        "vectors" => vectors_cmd(&args[1..]),
        other => Err((
            2,
            format!(
                "未知子命令 `{other}`（T0 只有 `vectors`；`info`/`ls`/`cat` 等自 T2 起才有）"
            ),
        )),
    }
}

fn vectors_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut gen = false;
    let mut check = false;
    let mut file: Option<PathBuf> = None;

    let mut i = 0;
    while i < args.len() {
        match args[i].as_str() {
            "--gen" => gen = true,
            "--check" => check = true,
            "--file" => {
                let Some(p) = args.get(i + 1) else {
                    return Err((2, "`--file` 需要一个路径参数".to_string()));
                };
                file = Some(PathBuf::from(p));
                i += 1;
            }
            "-h" | "--help" => return Ok(USAGE.to_string()),
            other => {
                return Err((
                    2,
                    format!("`vectors` 不认识参数 `{other}`（只支持 --gen / --check / --file）"),
                ))
            }
        }
        i += 1;
    }

    let path = file.unwrap_or_else(vectors::resolve_default);
    match (gen, check) {
        (true, true) => Err((2, "`--gen` 与 `--check` 只能二选一".to_string())),
        (false, false) => Err((2, "`vectors` 需要 `--gen` 或 `--check`".to_string())),
        (true, false) => vectors::generate(&path).map_err(|e| (1, e)),
        (false, true) => vectors::check(&path).map_err(|e| (1, e)),
    }
}
