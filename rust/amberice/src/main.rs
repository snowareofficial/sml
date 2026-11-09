// SPDX-License-Identifier: MulanPSL-2.0
//! The `amberice` command line (T0 has exactly one subcommand: `vectors`).
//! `amberice` 命令行（T0 只有一个子命令：`vectors`）。
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

use amberice::vectors;

const USAGE: &str = "\
amberice — 通用二进制信封 .ambr（SML 是内置载荷之一）

用法:
  amberice info    <file>                  一眼看清 edition/fmt/段表/摘要状态
  amberice ls      <file>                  逐段列出 id/偏移/长度/算法（`--segments` 同义）
  amberice check   <file>                  结构 + 逐段摘要校验，打印每段结论
  amberice cat     <file> [<path>]         无路径输出整份；有路径取当前值（canonical 文本）
  amberice pack    [--zstd [N]] <out.ambr> <file>...
                                       把若干文件打成一个 .ambr 归档（EMBED 段）；
                                       `--zstd` 段级压缩（N = 1..19，默认 3）
  amberice unpack  <file> <destdir>        把归档里的载荷逐个写出到目录

签名与层链（T4，需 `--features sign`）:
  amberice keygen  [--out FILE]            生成 Ed25519 密钥（seed/pub 各一行十六进制）
  amberice sign    <file> --seed HEX|FILE [--patch P.ambr] [-o OUT] [--in-place]
                                      [--delegation F]
  amberice append  <file> <patch.ambr> --seed HEX|FILE [--expect-head HEX64] [-o OUT]
                                      `--expect-head` 把追加钉在写者看到的链头上（D20）
  amberice compact <file> --seed HEX|FILE [-o OUT]
                                      合并多层并**重签**（D8 唯一例外；丢弃历史，旧锚点要重建）
  amberice delegate --seed OLD --to PUBKEY --max N [--out FILE]   签一份换钥授权（D16）
  amberice verify  <file> [--trust HEX64]  验链 + 验签（全部必须通过，D36）；
                                       `--trust` 把链头钉在外部锚点上（防整条链被剥离）
  amberice tree    <file>                  打印层链：序号 / 作者 / 补丁键数 / 签名数 / 授权
  amberice history <file> <path> --at N    取第 N 层为止的值（历史回放；`cat --at` 已迁到这里）
  amberice get     <file> <path> [--bytes] 零拷贝取一棵子树（不解全树）；--bytes 出原始字节十六进制
  amberice text    <file> [--path P]       整份（或某一子树）的 canonical 文本
  amberice vectors --gen   [--file PATH] [--container-file PATH]  重新生成跨端向量
  amberice vectors --check [--file PATH] [--container-file PATH]  重新生成并逐行比对
  amberice --help | -h                     本帮助
  amberice --version | -V                  版本

说明:
  默认向量文件: tests/vectors/amberice-canonical.v1.json（自动向上找两级）
  `--check` 会先跑自检：每例 bytes → load → bytes 必须逐字节相同，
  且 text(load(bytes)) 必须等于原 text —— 否则签名与字节会各说各话。

  `vectors` 需要 sml-value 的 preserve_order（spec §18 C2：键序 = 源序）；
  `info` / `ls` / `check` 只读容器，`cat` 要解码（解码也要源序），故 `cat` 同理：
    cargo run -p amberice --features preserve_order -- vectors --check

已实现：T0 向量 / T1 容器 / T2 manifest·摘要·EMBED / T3 随机访问 / T4 层链签名 /
T5 压缩与加密（库）/ T6 互转（库在 `sml::envelope`）。施工顺序见 spec §17 / §22。

注意:
  - 加密（`dump_encrypted`）与 mmap（`MappedFile`）目前**只有库接口，没有 CLI**；
  - 层记录的 `base_hash` 已把基文档绑进签名（spec §21.1b E）；换基文档会当场被
    `verify` 抓到。要防「整条层链被剥离」，仍须用 `--trust` 记住链头。";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args) {
        Ok(msg) => {
            println!("{msg}");
            ExitCode::SUCCESS
        }
        Err((code, msg)) => {
            // 多行报告（`check` 的逐段结论）不加前缀，否则整段都被 `amberice: ` 糊住
            if msg.contains('\n') {
                eprintln!("{msg}");
            } else {
                eprintln!("amberice: {msg}");
            }
            // 只在**真的**因为缺源序而失败时给提示，别把无关错误也糊上这条；
            // 该错误在两档构建下都会出现，与 feature 无关
            if msg.contains("保序") {
                eprintln!(
                    "amberice: hint: 这个值是显式无序态（`Map::Unordered`），canonical 需要源序\n\
                     amberice:       文本用 `parse_ordered()`；程序构造用 `Map::ordered()` / `OrderedMap`"
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
        "-V" | "--version" => Ok(format!("amberice {}", env!("CARGO_PKG_VERSION"))),
        "vectors" => vectors_cmd(&args[1..]),
        "info" => read_cmd(&args[1..], |path, bytes| {
            let info = amberice::info(bytes).map_err(|e| (1, format!("{path}: {e}")))?;
            let lines = amberice::container::info_lines(path, bytes, &info)
                .map_err(|e| (1, format!("{path}: {e}")))?;
            Ok(lines.join("\n"))
        }),
        "ls" => {
            let rest: Vec<String> = args[1..]
                .iter()
                .filter(|a| a.as_str() != "--segments")
                .cloned()
                .collect();
            read_cmd(&rest, |path, bytes| {
                let info = amberice::info(bytes).map_err(|e| (1, format!("{path}: {e}")))?;
                if info.segments.is_empty() {
                    return Ok(format!("{path}: 没有段"));
                }
                let mut out = amberice::container::ls_lines(&info);
                // 归档类文件还要列载荷（名字 + 长度），否则 `ls` 看不出里面装了什么
                let pl = amberice::payloads(bytes).map_err(|e| (1, format!("{path}: {e}")))?;
                for p in pl {
                    out.push(format!("     payload {:<24} {} B", p.name, p.len));
                }
                Ok(out.join("\n"))
            })
        }
        "check" => read_cmd(&args[1..], |path, bytes| {
            let report = amberice::check(bytes).map_err(|e| (1, format!("{path}: {e}")))?;
            let mut out = report.lines.join("\n");
            out.push_str(&format!("\n\n{}: {}", path, if report.ok { "通过" } else { "未通过" }));
            if report.ok {
                Ok(out)
            } else {
                Err((1, out))
            }
        }),
        "cat" => cat_cmd(&args[1..]),
        "history" => history_cmd(&args[1..]),
        "pack" => pack_cmd(&args[1..]),
        "unpack" => unpack_cmd(&args[1..]),
        "verify" => verify_cmd(&args[1..]),
        "get" => get_cmd(&args[1..]),
        "text" => text_cmd(&args[1..]),
        "tree" => tree_cmd(&args[1..]),
        "keygen" => keygen_cmd(&args[1..]),
        "delegate" => delegate_cmd(&args[1..]),
        "sign" => sign_cmd(&args[1..], false),
        "append" => sign_cmd(&args[1..], true),
        "compact" => compact_cmd(&args[1..]),
        other => Err((
            2,
            format!("未知子命令 `{other}`（现有 info / ls / check / cat / pack / unpack / vectors）"),
        )),
    }
}

/// Read a file and hand its bytes to `f` — shared by `info` / `ls`.
/// 读文件并把字节交给 `f` —— `info` / `ls` 共用。
fn read_cmd<F>(args: &[String], f: F) -> Result<String, (u8, String)>
where
    F: FnOnce(&str, &[u8]) -> Result<String, (u8, String)>,
{
    let Some(path) = args.first() else {
        return Err((2, "需要一个文件路径".to_string()));
    };
    if args.len() > 1 {
        return Err((2, format!("多余参数 `{}`", args[1])));
    }
    let bytes = std::fs::read(path).map_err(|e| (1, format!("读 {path} 失败：{e}")))?;
    f(path, &bytes)
}

fn vectors_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut gen = false;
    let mut check = false;
    let mut file: Option<PathBuf> = None;
    let mut container_file: Option<PathBuf> = None;

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
            "--container-file" => {
                let Some(p) = args.get(i + 1) else {
                    return Err((2, "`--container-file` 需要一个路径参数".to_string()));
                };
                container_file = Some(PathBuf::from(p));
                i += 1;
            }
            "-h" | "--help" => return Ok(USAGE.to_string()),
            other => {
                return Err((
                    2,
                    format!(
                        "`vectors` 不认识参数 `{other}`（只支持 --gen / --check / --file / --container-file）"
                    ),
                ))
            }
        }
        i += 1;
    }

    let path = file.unwrap_or_else(vectors::resolve_default);
    let cpath = container_file.unwrap_or_else(vectors::resolve_container_default);
    match (gen, check) {
        (true, true) => Err((2, "`--gen` 与 `--check` 只能二选一".to_string())),
        (false, false) => Err((2, "`vectors` 需要 `--gen` 或 `--check`".to_string())),
        (true, false) => {
            let a = vectors::generate(&path).map_err(|e| (1, e))?;
            let b = vectors::generate_container(&cpath).map_err(|e| (1, e))?;
            Ok(format!("{a}\n{b}"))
        }
        (false, true) => {
            let a = vectors::check(&path).map_err(|e| (1, e))?;
            let b = vectors::check_container(&cpath).map_err(|e| (1, e))?;
            Ok(format!("{a}\n{b}"))
        }
    }
}

/// `amberice pack <out.ambr> <file>...` — pack blobs into one archive.
/// 把若干文件打成一个 `.ambr` 归档。
fn pack_cmd(args: &[String]) -> Result<String, (u8, String)> {
    // `--zstd [N]`：N 可省（下一个 token 不是数字时就取默认 3）
    let mut compress: Option<i32> = None;
    let mut rest: Vec<&String> = Vec::new();
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--zstd" {
            compress = Some(match args.get(i + 1).and_then(|v| v.parse::<i32>().ok()) {
                Some(level) => {
                    i += 2;
                    level
                }
                None => {
                    i += 1;
                    3
                }
            });
            continue;
        }
        rest.push(&args[i]);
        i += 1;
    }

    let Some(out) = rest.first() else {
        return Err((2, "用法: amberice pack [--zstd [N]] <out.ambr> <file>...".to_string()));
    };
    let files = &rest[1..];
    if files.is_empty() {
        return Err((2, "至少给一个要打包的文件".to_string()));
    }
    // 载荷名就用调用方给的路径：解包时原样还原目录结构。
    // 直接把拥有所有权的 (name, bytes) 交给 `pack_iter_with`，不再造 `&[(&str, &[u8])]`
    // 这层影子数组 —— 那是 API 在向调用方收税（评审原话）。
    let items: Vec<(String, Vec<u8>)> = files
        .iter()
        .map(|f| {
            std::fs::read(f)
                .map(|b| ((**f).clone(), b))
                .map_err(|e| (1, format!("读 {f} 失败：{e}")))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let count = items.len();
    let opts = amberice::DumpOptions {
        compress,
        ..Default::default()
    };
    let file = amberice::pack_iter_with(items, &opts).map_err(|e| (1, e.to_string()))?;
    std::fs::write(out, &file).map_err(|e| (1, format!("写 {out} 失败：{e}")))?;
    Ok(format!(
        "已打包 {} 个载荷 → {out}（{} B{}）",
        count,
        file.len(),
        match compress {
            Some(l) => format!("，zstd level {l}"),
            None => String::new(),
        }
    ))
}

/// `amberice unpack <file> <destdir>` — write every payload out.
/// 把归档里的载荷逐个写出到目录。
///
/// 载荷名来自**文件**，可能带 `..` 或绝对路径 ⇒ 解包前必须挡住目录穿越。
/// 一个能写任意路径的「解包」等于把 tar-slip 送上门。
/// Payload names come from the file: reject `..` / absolute paths (tar-slip).
fn unpack_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let (Some(file), Some(dest)) = (args.first(), args.get(1)) else {
        return Err((2, "用法: amberice unpack <file> <destdir>".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let list = amberice::payloads(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
    if list.is_empty() {
        return Err((1, format!("{file}: 里面没有载荷（不是归档，或者 manifest 没列）")));
    }
    let mut written = 0usize;
    for p in &list {
        let path = safe_join(dest, &p.name)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| (1, format!("建目录 {} 失败：{e}", parent.display())))?;
        }
        let data = amberice::payload(&bytes, &p.name).map_err(|e| (1, format!("{file}: {e}")))?;
        std::fs::write(&path, &data)
            .map_err(|e| (1, format!("写 {} 失败：{e}", path.display())))?;
        written += 1;
    }
    Ok(format!("已解包 {written} 个载荷 → {dest}"))
}

/// Reject anything that could escape `dest` (tar-slip). ｜ 挡住目录穿越。
fn safe_join(dest: &str, name: &str) -> Result<std::path::PathBuf, (u8, String)> {
    let bad = name.is_empty()
        || name.starts_with('/')
        || name.starts_with('\\')
        || name.contains(':')
        || name.split(['/', '\\']).any(|c| c == "..");
    if bad {
        return Err((
            1,
            format!("拒绝解包：载荷名 `{name}` 是绝对路径或含 `..`（目录穿越）"),
        ));
    }
    Ok(std::path::Path::new(dest).join(name))
}

/// `amberice cat <file> [<path>]` — 无路径输出整份（Unix 直觉），有路径取当前值。
///
/// 历史回放**已拆到** [`history_cmd`]：同一个 `cat` 既当「整份输出」又当「历史查询」，
/// 正是评审说的「与 Unix 直觉相反」。现在三者的分工：
/// `cat` = 整份 / 当前值 · `get` = 零拷贝取子树 · `history` = 第 N 层为止的值。
fn cat_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut pos: Vec<&String> = Vec::new();
    for a in args {
        if a == "--at" {
            return Err((
                2,
                "历史回放已拆到 `amberice history <file> <path> --at N`（`cat` 只取当前值）"
                    .to_string(),
            ));
        }
        pos.push(a);
    }
    let Some(file) = pos.first() else {
        return Err((2, "用法: amberice cat <file> [<path>]".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;

    // 无路径 ⇒ 整份 canonical 文本（与 `text` 同源，Unix `cat` 的直觉）
    let Some(expr) = pos.get(1) else {
        if pos.len() > 1 {
            return Err((2, format!("多余参数 `{}`", pos[1])));
        }
        let store = amberice::Store::open(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
        let text = store.text().map_err(|e| (1, format!("{file}: {e}")))?;
        return Ok(text.trim_end_matches('\n').to_string());
    };

    let v = amberice::get(&bytes, expr).map_err(|e| (1, format!("{file}: {e}")))?;
    v.canonical_text().map_err(|e| (1, format!("{e}")))
}

/// `amberice history <file> <path> --at N` — 第 N 层为止的值（历史回放）。
/// History replay, deliberately its own command: `cat` should not silently change meaning.
fn history_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut pos: Vec<&String> = Vec::new();
    let mut at: Option<usize> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--at" {
            let Some(n) = args.get(i + 1) else {
                return Err((2, "`--at` 需要层号".to_string()));
            };
            at = Some(n.parse().map_err(|_| (2, format!("`--at {n}` 不是数字")))?);
            i += 2;
            continue;
        }
        pos.push(&args[i]);
        i += 1;
    }
    let (Some(file), Some(expr)) = (pos.first(), pos.get(1)) else {
        return Err((2, "用法: amberice history <file> <path> --at N".to_string()));
    };
    let Some(n) = at else {
        return Err((
            2,
            "`history` 必须给 `--at N`（只想取当前值请用 `cat` / `get`）".to_string(),
        ));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let raw = amberice::container::base_value(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
    let v = amberice::layer::history_value_at(&bytes, &raw, n, expr)
        .map_err(|e| (1, format!("{file}: {e}")))?;
    v.canonical_text().map_err(|e| (1, format!("{e}")))
}

/// `amberice verify <file>` — 验链 + 验签。
fn verify_cmd(args: &[String]) -> Result<String, (u8, String)> {
    // `--trust HEX64`：可选的外部锚点（D18）。没有它，「本来没签」与「签了又被剥掉」
    // 在字节上一模一样 —— 这正是 §21.1c 那条已知边界。
    let mut trust: Option<[u8; 32]> = None;
    let mut file: Option<&String> = None;
    let mut i = 0usize;
    while i < args.len() {
        if args[i] == "--trust" {
            let Some(hex) = args.get(i + 1) else {
                return Err((2, "用法: amberice verify <file> [--trust HEX64]".to_string()));
            };
            trust = Some(hex32(hex).ok_or_else(|| {
                (2, format!("--trust 要 64 位十六进制（链头哈希），实得 `{hex}`"))
            })?);
            i += 2;
            continue;
        }
        file = Some(&args[i]);
        i += 1;
    }
    let Some(file) = file else {
        return Err((2, "用法: amberice verify <file> [--trust HEX64]".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let algo = amberice::container::hash_algo(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;

    #[cfg(feature = "sign")]
    {
        let report = match &trust {
            Some(t) => amberice::verify_trusted(&bytes, algo, t),
            None => amberice::verify(&bytes, algo),
        }
        .map_err(|e| (1, format!("{file}: {e}")))?;
        if trust.is_none() && report.ok {
            // 没有锚点时把链头打出来 —— 让人**记得住**下一次要钉什么
            if let Ok(Some(head)) = amberice::layer::chain_head(&bytes, algo) {
                eprintln!(
                    "amberice: hint: 本次链头 {}（下次可用 `--trust` 钉住它，防整条链被剥离）",
                    to_hex(&head)
                );
            }
        }
        let mut out = report.lines.join("\n");
        out.push_str(&format!(
            "\n\n{file}: {}（{} 层）",
            if report.ok { "通过" } else { "未通过" },
            report.layers
        ));
        if report.ok {
            Ok(out)
        } else {
            Err((1, out))
        }
    }
    #[cfg(not(feature = "sign"))]
    {
        // 没有 sign feature 也能回答「结构对不对」，但不能回答「谁签的」
        let chain = amberice::layer::verify_chain(&bytes, algo)
            .map_err(|e| (1, format!("{file}: {e}")))?;
        let mut chain = chain;
        if let Some(t) = &trust {
            // 链头只用到摘要，不需要 ed25519 ⇒ 没有 `sign` 也能钉锚点
            match amberice::layer::chain_head(&bytes, algo) {
                Ok(Some(head)) if head == *t => {
                    chain.lines.push("锚点     head 与期望一致（外部锚点，D18）".to_string())
                }
                Ok(Some(head)) => {
                    chain.ok = false;
                    chain.lines.push(format!(
                        "锚点     head 不符：文件是 {}…，期望 {}…",
                        &to_hex(&head)[..16],
                        &to_hex(t)[..16]
                    ));
                }
                Ok(None) => {
                    chain.ok = false;
                    chain.lines.push(
                        "锚点     文件里一层都没有，而 --trust 给了期望值 —— 层链可能被整个剥离"
                            .to_string(),
                    );
                }
                Err(e) => {
                    chain.ok = false;
                    chain.lines.push(format!("锚点     算不出链头：{e}"));
                }
            }
        }
        let mut out = chain.lines.join("\n");
        out.push_str(
            "\n注意     本构建未启用 `sign` feature：上面只验了链结构，**签名一个字都没验**",
        );
        if chain.ok {
            Ok(out)
        } else {
            Err((1, out))
        }
    }
}

/// `amberice tree <file>` — 打印层链。
fn tree_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let Some(file) = args.first() else {
        return Err((2, "用法: amberice tree <file>".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let all = amberice::layer::layers(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
    if all.is_empty() {
        return Ok(format!("{file}: 没有层（未签名）"));
    }
    let mut out = vec![format!("{file}: {} 层", all.len())];
    for l in &all {
        let patch = match &l.patch {
            Some(amberice::Value::Object(m)) => format!("patch {} 键", m.len()),
            Some(_) => "patch（整值替换）".to_string(),
            None => "无补丁".to_string(),
        };
        let deleg = match &l.delegation {
            Some(d) => format!("delegation ≤ {}", d.max_index),
            None => "-".to_string(),
        };
        out.push(format!(
            "  层#{:<3} 作者 {}…  {}  签名 {} 条  {}",
            l.index,
            hex_prefix(&l.author),
            patch,
            l.sigs.len(),
            deleg
        ));
    }
    Ok(out.join("\n"))
}

/// `amberice keygen [--out FILE]` — 生成 Ed25519 密钥。
fn keygen_cmd(args: &[String]) -> Result<String, (u8, String)> {
    #[cfg(feature = "sign")]
    {
        let mut out_path: Option<&String> = None;
        let mut i = 0;
        while i < args.len() {
            if args[i] == "--out" {
                let Some(p) = args.get(i + 1) else {
                    return Err((2, "`--out` 需要一个路径".to_string()));
                };
                out_path = Some(p);
                i += 2;
                continue;
            }
            return Err((2, format!("`keygen` 不认识参数 `{}`", args[i])));
        }
        let kp = amberice::KeyPair::generate().map_err(|e| (1, e.to_string()))?;
        let text = format!("seed:{}\npub:{}\n", kp.seed_hex(), kp.public_hex());
        match out_path {
            Some(p) => {
                std::fs::write(p, text.as_bytes())
                    .map_err(|e| (1, format!("写 {p} 失败：{e}")))?;
                Ok(format!(
                    "已写入 {p}\n注意：seed 是私钥，别提交、别贴群里（权限请自行收紧）"
                ))
            }
            None => Ok(format!(
                "{text}\n注意：seed 是私钥，别提交、别贴群里（权限请自行收紧）"
            )),
        }
    }
    #[cfg(not(feature = "sign"))]
    {
        let _ = args;
        Err((1, "本构建未启用 `sign` feature，无法生成密钥".to_string()))
    }
}

/// `amberice sign` / `amberice append` — 追加一个已签名的层。
/// `amberice compact <file> --seed …` —— 合并多层、丢弃历史、重签（D8）。
fn compact_cmd(args: &[String]) -> Result<String, (u8, String)> {
    #[cfg(feature = "sign")]
    {
        let mut file: Option<&String> = None;
        let mut seed_src: Option<&String> = None;
        let mut out_path: Option<&String> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--seed" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--seed` 需要种子或文件".to_string()));
                    };
                    seed_src = Some(v);
                    i += 2;
                }
                "-o" | "--out" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`-o` 需要一个路径".to_string()));
                    };
                    out_path = Some(v);
                    i += 2;
                }
                other => {
                    if other.starts_with('-') {
                        return Err((2, format!("不认识的参数 `{other}`")));
                    }
                    if file.is_none() {
                        file = Some(&args[i]);
                    } else {
                        return Err((2, format!("多余参数 `{other}`")));
                    }
                    i += 1;
                }
            }
        }
        let Some(file) = file else {
            return Err((2, "用法: amberice compact <file> --seed HEX|FILE [-o OUT]".to_string()));
        };
        let Some(seed_src) = seed_src else {
            return Err((2, "必须给 `--seed`（compact 会重签）".to_string()));
        };
        let seed = read_seed(seed_src)?;
        let kp = amberice::KeyPair::from_seed_hex(&seed).map_err(|e| (1, e.to_string()))?;
        let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
        let algo = amberice::container::hash_algo(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
        let before = amberice::layer::layers(&bytes).map(|l| l.len()).unwrap_or(0);
        let fresh = amberice::compact(&bytes, &kp, algo).map_err(|e| (1, e.to_string()))?;
        let dest = out_path.map(|s| s.as_str()).unwrap_or(file.as_str());
        std::fs::write(dest, &fresh).map_err(|e| (1, format!("写 {dest} 失败：{e}")))?;
        Ok(format!(
            "已合并 {before} 层 → {dest}（重签为 1 层，{} B；历史已丢弃，旧 --trust 锚点需重建）",
            fresh.len()
        ))
    }
    #[cfg(not(feature = "sign"))]
    {
        let _ = args;
        Err((1, "本构建未启用 `sign` feature，无法重签".to_string()))
    }
}

fn sign_cmd(args: &[String], need_patch: bool) -> Result<String, (u8, String)> {
    #[cfg(feature = "sign")]
    {
        let mut file: Option<&String> = None;
        let mut patch_path: Option<&String> = None;
        let mut seed_src: Option<&String> = None;
        let mut out_path: Option<&String> = None;
        let mut deleg_path: Option<&String> = None;
        let mut expect_head: Option<[u8; 32]> = None;
        let mut in_place = false;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--seed" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--seed` 需要一个十六进制种子或文件路径".to_string()));
                    };
                    seed_src = Some(v);
                    i += 2;
                }
                "--patch" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--patch` 需要一个 .ambr 路径".to_string()));
                    };
                    patch_path = Some(v);
                    i += 2;
                }
                "-o" | "--out" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`-o` 需要一个路径".to_string()));
                    };
                    out_path = Some(v);
                    i += 2;
                }
                "--expect-head" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--expect-head` 需要 64 位十六进制链头".to_string()));
                    };
                    expect_head = Some(hex32(v).ok_or_else(|| {
                        (2, format!("--expect-head 需要 64 位十六进制，实得 `{v}`"))
                    })?);
                    i += 2;
                }
                "--delegation" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--delegation` 需要一个文件路径".to_string()));
                    };
                    deleg_path = Some(v);
                    i += 2;
                }
                "--in-place" => {
                    in_place = true;
                    i += 1;
                }
                other => {
                    if other.starts_with('-') {
                        return Err((2, format!("不认识的参数 `{other}`")));
                    }
                    if file.is_none() {
                        file = Some(&args[i]);
                    } else if patch_path.is_none() {
                        patch_path = Some(&args[i]);
                    } else {
                        return Err((2, format!("多余参数 `{other}`")));
                    }
                    i += 1;
                }
            }
        }
        let Some(file) = file else {
            return Err((2, "用法: amberice sign <file> --seed HEX|FILE [--patch P.ambr]".to_string()));
        };
        if need_patch && patch_path.is_none() {
            return Err((2, "`append` 必须给一个补丁文件".to_string()));
        }
        let Some(seed_src) = seed_src else {
            return Err((2, "必须给 `--seed`（十六进制种子，或含 `seed:` 行的文件）".to_string()));
        };

        // 种子：既接受 64 位十六进制，也接受 keygen 写出的文件
        let seed = if seed_src.len() == 64 && seed_src.chars().all(|c| c.is_ascii_hexdigit()) {
            seed_src.clone()
        } else {
            let text = std::fs::read_to_string(seed_src)
                .map_err(|e| (1, format!("读 {seed_src} 失败：{e}")))?;
            text.lines()
                .find_map(|l| l.strip_prefix("seed:").map(|s| s.trim().to_string()))
                .ok_or((1, format!("{seed_src} 里没有 `seed:` 行")))? 
        };
        let kp = amberice::KeyPair::from_seed_hex(&seed).map_err(|e| (1, e.to_string()))?;

        let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
        let algo = amberice::container::hash_algo(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
        let patch = match patch_path {
            Some(p) => {
                let pb = std::fs::read(p).map_err(|e| (1, format!("读 {p} 失败：{e}")))?;
                Some(amberice::load(&pb).map_err(|e| (1, format!("{p}: {e}")))?)
            }
            None => None,
        };

        // 换钥授权（D16）：由**上一位作者**离线签好，随本层一起走
        let delegation = match deleg_path {
            Some(p) => {
                let text =
                    std::fs::read_to_string(p).map_err(|e| (1, format!("读 {p} 失败：{e}")))?;
                Some(parse_delegation(&text)?)
            }
            None => None,
        };

        // D20：给了 `--expect-head` 就走带冲突检测的追加，否则沿用无条件追加
        let signed = match &expect_head {
            Some(h) => amberice::append_layer_checked(&bytes, &kp, patch.clone(), delegation, algo, h),
            None => amberice::append_layer(&bytes, &kp, patch.clone(), delegation, algo),
        }
        .map_err(|e| (1, e.to_string()))?;
        let dest = match (out_path, in_place) {
            (Some(p), _) => p.as_str(),
            (None, _) => file.as_str(), // 默认原地（与 `git commit` 同直觉）
        };
        std::fs::write(dest, &signed).map_err(|e| (1, format!("写 {dest} 失败：{e}")))?;
        let n = amberice::layer::layers(&signed).map(|l| l.len()).unwrap_or(0);
        Ok(format!(
            "已追加 1 层 → {dest}（现有 {n} 层，{} B）",
            signed.len()
        ))
    }
    #[cfg(not(feature = "sign"))]
    {
        let _ = (args, need_patch);
        Err((1, "本构建未启用 `sign` feature，无法签名".to_string()))
    }
}

fn hex_prefix(k: &[u8; 32]) -> String {
    k[..4].iter().map(|b| format!("{b:02x}")).collect()
}

/// `amberice delegate --seed OLD --to PUBKEY --max N [--out FILE]` — 签一份换钥授权。
fn delegate_cmd(args: &[String]) -> Result<String, (u8, String)> {
    #[cfg(feature = "sign")]
    {
        let mut seed_src: Option<&String> = None;
        let mut to: Option<&String> = None;
        let mut max: Option<u32> = None;
        let mut out_path: Option<&String> = None;
        let mut i = 0;
        while i < args.len() {
            match args[i].as_str() {
                "--seed" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--seed` 需要一个十六进制种子或文件路径".to_string()));
                    };
                    seed_src = Some(v);
                    i += 2;
                }
                "--to" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--to` 需要新作者的公钥（64 位十六进制）".to_string()));
                    };
                    to = Some(v);
                    i += 2;
                }
                "--max" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--max` 需要一个层号".to_string()));
                    };
                    max = Some(v.parse().map_err(|_| (2, format!("`--max {v}` 不是数字")))?);
                    i += 2;
                }
                "--out" => {
                    let Some(v) = args.get(i + 1) else {
                        return Err((2, "`--out` 需要一个路径".to_string()));
                    };
                    out_path = Some(v);
                    i += 2;
                }
                other => return Err((2, format!("`delegate` 不认识参数 `{other}`"))),
            }
        }
        let (Some(seed_src), Some(to), Some(max)) = (seed_src, to, max) else {
            return Err((
                2,
                "用法: amberice delegate --seed OLD --to PUBKEY --max N [--out FILE]".to_string(),
            ));
        };
        let seed = read_seed(seed_src)?;
        let kp = amberice::KeyPair::from_seed_hex(&seed).map_err(|e| (1, e.to_string()))?;
        let to_bytes = hex32(to).ok_or((2, "`--to` 需要 64 位十六进制公钥".to_string()))?;
        let d = kp.delegate(&to_bytes, max);
        let text = format!(
            "from:{}\nsig:{}\nmax:{}\n",
            to_hex(&d.from),
            to_hex(&d.sig),
            d.max_index
        );
        match out_path {
            Some(p) => {
                std::fs::write(p, text.as_bytes()).map_err(|e| (1, format!("写 {p} 失败：{e}")))?;
                Ok(format!("已写入 {p}（换钥授权：≤ 第 {max} 层）"))
            }
            None => Ok(text),
        }
    }
    #[cfg(not(feature = "sign"))]
    {
        let _ = args;
        Err((1, "本构建未启用 `sign` feature，无法签授权".to_string()))
    }
}

/// 读种子：既接受 64 位十六进制，也接受 `keygen --out` 写出的文件。
#[cfg(feature = "sign")]
fn read_seed(src: &str) -> Result<String, (u8, String)> {
    if src.len() == 64 && src.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(src.to_string());
    }
    let text = std::fs::read_to_string(src).map_err(|e| (1, format!("读 {src} 失败：{e}")))?;
    text.lines()
        .find_map(|l| l.strip_prefix("seed:").map(|s| s.trim().to_string()))
        .ok_or((1, format!("{src} 里没有 `seed:` 行")))
}

/// 解析 `delegate` 写出的文件（`from:` / `sig:` / `max:` 三行）。
#[cfg(feature = "sign")]
fn parse_delegation(text: &str) -> Result<amberice::Delegation, (u8, String)> {
    let mut from = None;
    let mut sig = None;
    let mut max = None;
    for line in text.lines() {
        if let Some(v) = line.strip_prefix("from:") {
            from = Some(hex32(v.trim()).ok_or((2, "delegation.from 需 64 位十六进制".to_string()))?);
        } else if let Some(v) = line.strip_prefix("sig:") {
            sig = Some(hex_n(v.trim()).ok_or((2, "delegation.sig 需十六进制".to_string()))?);
        } else if let Some(v) = line.strip_prefix("max:") {
            max = Some(v.trim().parse().map_err(|_| (2, "delegation.max 不是数字".to_string()))?);
        }
    }
    match (from, sig, max) {
        (Some(from), Some(sig), Some(max_index)) => Ok(amberice::Delegation {
            from,
            max_index,
            sig,
        }),
        _ => Err((2, "delegation 文件缺 `from:` / `sig:` / `max:`".to_string())),
    }
}

// 不加 cfg：`verify --trust` 在没有 `sign` feature 的构建里也要解析/打印链头。
fn to_hex(d: &[u8]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}

// 同上：`hex32` 依赖它。
fn hex_n(s: &str) -> Option<Vec<u8>> {
    if !s.len().is_multiple_of(2) || !s.is_ascii() {
        return None;
    }
    let mut out = Vec::with_capacity(s.len() / 2);
    for i in (0..s.len()).step_by(2) {
        out.push(u8::from_str_radix(&s[i..i + 2], 16).ok()?);
    }
    Some(out)
}

// 同上：`--trust HEX64` 的解析与签名无关。
fn hex32(s: &str) -> Option<[u8; 32]> {
    let v = hex_n(s)?;
    v.as_slice().try_into().ok()
}

/// `amberice get <file> <path> [--bytes]` — 随机访问取一棵子树（T3）。
fn get_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut pos: Vec<&String> = Vec::new();
    let mut raw = false;
    for a in args {
        if a == "--bytes" {
            raw = true;
        } else {
            pos.push(a);
        }
    }
    let (Some(file), Some(expr)) = (pos.first(), pos.get(1)) else {
        return Err((2, "用法: amberice get <file> <path> [--bytes]".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let store = amberice::Store::open(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;

    if raw {
        // 零拷贝：直接把子树字节打成十六进制（含 6 B 节点头），给调试与跨端比对用
        let sub = store
            .subtree_bytes(expr)
            .map_err(|e| (1, format!("{file}: {e}")))?;
        return Ok(sub.iter().map(|b| format!("{b:02x}")).collect::<String>());
    }
    let (v, touched) = store
        .get_with_stats(expr)
        .map_err(|e| (1, format!("{file}: {e}")))?;
    let mut out = v.canonical_text().map_err(|e| (1, format!("{e}")))?;
    // 把「看了几个节点」写进 stderr：判据要能被人看见，而不是只在测试里
    eprintln!("amberice: 导航看过 {touched} 个节点（未遍历全树）");
    out = out.trim_end_matches('\n').to_string();
    Ok(out)
}

/// `amberice text <file> [--path P]` — 整份或某一子树的 canonical 文本。
fn text_cmd(args: &[String]) -> Result<String, (u8, String)> {
    let mut file: Option<&String> = None;
    let mut path: Option<&String> = None;
    let mut i = 0;
    while i < args.len() {
        if args[i] == "--path" {
            let Some(v) = args.get(i + 1) else {
                return Err((2, "`--path` 需要一个路径".to_string()));
            };
            path = Some(v);
            i += 2;
            continue;
        }
        if file.is_none() {
            file = Some(&args[i]);
        } else {
            return Err((2, format!("多余参数 `{}`", args[i])));
        }
        i += 1;
    }
    let Some(file) = file else {
        return Err((2, "用法: amberice text <file> [--path P]".to_string()));
    };
    let bytes = std::fs::read(file).map_err(|e| (1, format!("读 {file} 失败：{e}")))?;
    let store = amberice::Store::open(&bytes).map_err(|e| (1, format!("{file}: {e}")))?;
    let text = match path {
        Some(p) => store.text_of(p),
        None => store.text(),
    }
    .map_err(|e| (1, format!("{file}: {e}")))?;
    Ok(text.trim_end_matches('\n').to_string())
}
