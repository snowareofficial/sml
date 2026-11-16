//! 函数级/阶段级热点归因：用**不同文档形状的差分**拆出「词法 / 结构构建 / 契约 / 数组」的成本。
//!
//! 为什么不用 profiler：本仓不引 dev 依赖，`perf` 只报总量。而阶段占比可以用**控制变量**得到：
//! 同一规模下换一种写法，两两相减就是那段语法的净成本。
//!
//! 为什么每个形状跑多批：本机 bench 漂移实测可达 ±8%（同一份代码两次 3.70 / 3.90 ms），
//! 单次测量无法比较。这里每形状跑 `BATCHES` 批，**同时给出 min 与 median** ——
//! 两者差距大就说明当轮环境不稳，数字不能用来做 A/B。
//!
//! 用法：`cargo bench --bench hotspots`

mod common;

use std::hint::black_box;
use std::time::Instant;

/// 每形状的批数。取 9：min 抗抖动，median 看典型值。
const BATCHES: usize = 15;

/// 每批内重复次数（够到毫秒级，又不至于让一轮跑太久）。
const REPS: usize = 5;

// 保留给下一轮微观归因：当前 bench 主体还没切到采样版，先别按 dead_code 删掉。
#[allow(dead_code)]
fn sample<F: FnMut()>(mut f: F) -> (f64, f64) {
    // 热身：让惰性初始化与 CPU 频率先稳定。
    f();
    let mut v = Vec::with_capacity(BATCHES);
    for _ in 0..BATCHES {
        let t = Instant::now();
        for _ in 0..REPS {
            f();
        }
        v.push(t.elapsed().as_nanos() as f64 / REPS as f64);
    }
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[0], v[v.len() / 2])
}

/// 基线形状：`k0: v0` —— 只有词法 + 标量 coerce + 一次对象插入。
fn scalars(n: usize) -> String {
    let mut s = String::with_capacity(n * 16);
    for i in 0..n {
        s.push_str(&format!("k{i}: v{i}\n"));
    }
    s
}

/// 块形状：每项是一个只有 1 个字段的块 —— 相比基线多的就是「块帧 + __type 之类」。
fn blocks(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("b{i} {{ x: 1 }}\n"));
    }
    s
}

/// 数组形状：每项一个 5 元素数组 —— 相比基线多的是数组构建。
fn arrays(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("a{i}: [ 1 2 3 4 5 ]\n"));
    }
    s
}

/// 契约形状：声明一份契约，然后每个块 `@is` 应用一次 —— 相比块形状多的是契约校验/回填。
fn with_contract(n: usize) -> String {
    let mut s = String::from("@contract C {\n    a: int\n    b: str\n}\n");
    s.reserve(n * 32);
    for i in 0..n {
        s.push_str(&format!("c{i} {{ @is C a: {i} b: s }}\n"));
    }
    s
}

/// 同规模、同字段，但**不写 `@is`** —— 与 `with_contract` 相减即 `@is` 的纯成本。
fn contract_plain(n: usize) -> String {
    let mut s = String::with_capacity(n * 24);
    for i in 0..n {
        s.push_str(&format!("c{i} {{ a: {i} b: s }}\n"));
    }
    s
}

/// 同上，但契约是 `loose` —— 与严格版相减即「严格性遍历（未声明字段检查）」的成本。
fn with_contract_loose(n: usize) -> String {
    let mut s = String::from("@contract C loose {\n    a: int\n    b: str\n}\n");
    s.reserve(n * 32);
    for i in 0..n {
        s.push_str(&format!("c{i} {{ @is C a: {i} b: s }}\n"));
    }
    s
}

/// 嵌套契约：字段类型是另一份契约（`x: Sub`）—— 专门测「组合字段」那条路径。
///
/// 这条路径曾经有冗余：`check_type` 会克隆子对象并跑一遍 `apply_contract` 后丢弃结果，
/// 调用方为回填再克隆一次、再应用一次。两种实现（优化前后）在本形状上的差值就是收益。
fn with_contract_nested(n: usize) -> String {
    let mut s = String::from(
        "@contract Sub {\n    a: int\n    b: str\n}\n@contract C {\n    x: Sub\n}\n",
    );
    s.reserve(n * 40);
    for i in 0..n {
        s.push_str(&format!("c{i} {{\n    @is C\n    x {{ a: {i} b: s }}\n}}\n"));
    }
    s
}

/// 同规模、同结构，但**不写 `@is`** —— 与 `with_contract_nested` 相减即组合契约的成本。
fn nested_plain(n: usize) -> String {
    let mut s = String::with_capacity(n * 32);
    for i in 0..n {
        s.push_str(&format!("c{i} {{\n    x {{ a: {i} b: s }}\n}}\n"));
    }
    s
}

/// 宽契约：10 个字段，块里只写 2 个（其余走 default）—— 与窄契约相减即「按字段数增长」的部分。
fn with_contract_wide(n: usize) -> String {
    let mut s = String::from("@contract C {\n");
    for k in 0..10 {
        s.push_str(&format!("    f{k}: int default {k}\n"));
    }
    s.push_str("}\n");
    s.reserve(n * 32);
    for i in 0..n {
        s.push_str(&format!("c{i} {{ @is C f0: {i} f1: 1 }}\n"));
    }
    s
}

/// 特性形状：头部带 `@version` + `@feature` —— 相比基线多的是特性扫描与版本求交。
fn with_features(n: usize) -> String {
    let mut s = String::from("@version v1\n@feature enable when, typed-block\n");
    s.push_str(&scalars(n));
    s
}

/// 同上，但把两个特性 **disable**：正文完全一样（不使用它们）。
/// 与 enable 版相减 = 「运行时特性判断」在当前实现里的实际成本。
fn with_features_disabled(n: usize) -> String {
    let mut s = String::from("@version v1\n@feature disable when, typed-block\n");
    s.push_str(&scalars(n));
    s
}

fn main() {
    let quick = common::quick();
    let n = if quick { 1000 } else { 4000 };

    println!("# 阶段归因（多批取 min / median）\n");
    println!(
        "- 口径：每形状 {} 批（交错）× {} 次；min 抗抖动，median 看典型值\n- 机器漂移参考：两者差距 >10% 时本轮数字不可用于 A/B\n",
        BATCHES, REPS
    );
    println!("| 形状 | 规模 | tokenize min | parse min | parse median | min/median 差 | 构建段（parse−tokenize） |");
    println!("|---|---|---|---|---|---|---|");

    let nc = n / 4; // 契约形状的块数少一些（单个块贵一个量级），但仍足够稳定
    let mut shapes: Vec<(&str, String)> = vec![
        ("标量（基线）", scalars(n)),
        ("块 ×1 字段", blocks(n)),
        ("数组 ×5 元素", arrays(n)),
    ];
    // —— 契约差分：plain / strict / loose / wide 四条一组，两两相减定位成本来源 ——
    shapes.push(("块 ×2 字段（无 @is）", contract_plain(nc)));
    shapes.push(("契约 @is（严格）", with_contract(nc)));
    shapes.push(("契约 @is（loose）", with_contract_loose(nc)));
    shapes.push(("契约 @is（10 字段）", with_contract_wide(nc)));
    // —— 组合字段（`x: Sub`）：这条路径曾经每字段克隆两次 + 应用两遍 ——
    shapes.push(("嵌套块（无 @is）", nested_plain(nc)));
    shapes.push(("嵌套契约 @is", with_contract_nested(nc)));
    // —— 特性开关对照：enable / disable 同一份正文，看运行时判断到底花不花钱 ——
    shapes.push(("带 @feature enable", with_features(n)));
    shapes.push(("带 @feature disable", with_features_disabled(n)));

    // —— 交错测量：每批把所有形状各跑一次 ——
    //
    // 为什么不能一个形状一个形状地跑完再比：本机漂移 ±20%，形状 A 跑在「机器忙」、
    // 形状 B 跑在「机器闲」，差值就变成环境噪声（实测同一对形状两次测出 0.64 与 1.58 µs）。
    // 交错后每批内部的相对关系才是可比的，min 取的是「各形状都处在最好状态」的那一批。
    for (_, text) in &shapes {
        let _ = black_box(sml_lex::tokenize(black_box(text.as_str())));
        let _ = black_box(sml_parse::parse(black_box(text.as_str())));
    }
    let m = shapes.len();
    let mut tk: Vec<Vec<f64>> = vec![Vec::with_capacity(BATCHES); m];
    let mut ps: Vec<Vec<f64>> = vec![Vec::with_capacity(BATCHES); m];
    for _ in 0..BATCHES {
        for (i, (_, text)) in shapes.iter().enumerate() {
            let t = Instant::now();
            let _ = black_box(sml_lex::tokenize(black_box(text.as_str())));
            tk[i].push(t.elapsed().as_nanos() as f64);
            let t = Instant::now();
            let _ = black_box(sml_parse::parse(black_box(text.as_str())));
            ps[i].push(t.elapsed().as_nanos() as f64);
        }
    }

    for (i, (label, text)) in shapes.iter().enumerate() {
        let size = format!("{:.1} KiB", text.len() as f64 / 1024.0);
        tk[i].sort_by(|a, b| a.partial_cmp(b).unwrap());
        ps[i].sort_by(|a, b| a.partial_cmp(b).unwrap());
        let tk_min = tk[i][0];
        let p_min = ps[i][0];
        let p_med = ps[i][ps[i].len() / 2];
        let drift = if p_min > 0.0 {
            (p_med - p_min) / p_min * 100.0
        } else {
            0.0
        };
        println!(
            "| {} | {} | {:.0} | {:.0} | {:.0} | {:+.1}% | {:.0}（{:.0}%） |",
            label,
            size,
            tk_min,
            p_min,
            p_med,
            drift,
            p_min - tk_min,
            (p_min - tk_min) / p_min * 100.0
        );
    }

    println!("\n> 读法：`构建段` = parse 减 tokenize，含 coerce、对象/数组插入、契约校验。");
    println!("> 与「标量（基线）」那行相减，即该形状**额外**引入的成本。");
}
