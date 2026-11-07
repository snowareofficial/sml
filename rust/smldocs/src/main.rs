// Copyright (C) SNOWARE
// SPDX-License-Identifier: MulanPSL-2.0
//! `smldocs` —— 本仓库 `docs/` 的索引与门禁（**内部工具，不发布**）。
//!
//! 与 `smltools` 的分工：`smltools` 是给装 SML 的人用的产品 CLI；本程序只认本仓库的
//! `docs/meta.sml` 目录布局，属仓库治理，故独立成 crate 且 `publish = false`。
//!
//! ```text
//! cargo run -p smldocs -- --index              # 生成 docs/index.md
//! cargo run -p smldocs -- --report             # 生成 docs/report.md
//! cargo run -p smldocs -- --check              # CI 门禁：任一命中即非零退出
//! cargo run -p smldocs -- --fix                # 只补 updated / status，不猜内容
//! cargo run -p smldocs -- --check --docs-root ..   # 指定仓库根
//! ```
//!
//! 参数手写解析（4 个开关 + 1 个取值），不为此引入 clap。

mod docs;

use std::path::PathBuf;
use std::process::ExitCode;

const HELP: &str = "\
smldocs —— 本仓库 docs/ 的索引与门禁（内部工具，不发布）

用法：
  smldocs --index              生成 docs/index.md（按 owner 分组，显示 status 与替代者）
  smldocs --report             生成 docs/report.md（缺字段 / 超期 / 断链 / stale 无替代者 / 未登记）
  smldocs --check              CI 门禁（只读，不写任何文件 ｜ read-only）：任一命中即非零退出
  smldocs --fix                只补 updated / status，不猜内容
选项：
  --docs-root <DIR>            仓库根（缺省时在当前目录与上一级找 docs/meta.sml）
  -h, --help                   显示本帮助

组合：`--check` 纯只读 ｜ read-only；`--fix` 可与其它开关同用。
写入生成物：内容相同则不写 ｜ skip if unchanged；内容要变且目标有未提交改动时拒绝覆盖 ｜ refuse to overwrite uncommitted files.
";

fn main() -> ExitCode {
    let mut opts = docs::Opts::default();
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--index" => opts.index = true,
            "--report" => opts.report = true,
            "--check" => opts.check = true,
            "--fix" => opts.fix = true,
            "--docs-root" => match it.next() {
                Some(v) => opts.root = Some(PathBuf::from(v)),
                None => {
                    eprintln!("smldocs: `--docs-root` 需要一个目录参数");
                    return ExitCode::from(2);
                }
            },
            "-h" | "--help" => {
                print!("{HELP}");
                return ExitCode::SUCCESS;
            }
            other => {
                eprintln!("smldocs: 未知参数 `{other}`（--index / --report / --check / --fix / --docs-root DIR）");
                return ExitCode::from(2);
            }
        }
    }
    if !(opts.index || opts.report || opts.check || opts.fix) {
        print!("{HELP}");
        return ExitCode::from(2);
    }
    docs::run(opts)
}
