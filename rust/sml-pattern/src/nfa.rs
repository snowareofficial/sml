//! Thompson NFA：编译 + 并行推进模拟。
//!
//! 无回溯是**结构性保证**而非参数调优：量词编译成 ε-转移，模拟时所有线程
//! 同时推进，任何字符都不会被「回头重试」。因此最坏时间是 O(文本长 × 程序长)，
//! 不存在灾难性回溯（ReDoS）。

use crate::{Class, Pat};

#[derive(Debug, Clone)]
pub(crate) enum Inst {
    /// 消费一个字符
    Char(Class),
    /// ε 分叉：两条分支同时尝试（优先级：先 x 后 y）
    Split(usize, usize),
    /// 无条件跳转
    Jmp(usize),
    Match,
}

/// 把模式编译为指令序列（Thompson 构造）。
///
/// `Repeat` 的处理是安全性的关键：
/// - 有上界 → 展开为固定份数的可选副本（不产生回边）
/// - 无上界（`+` / `*`）→ 一条回边 Split，**不复制模式本体**，
///   因此不会出现 regex 里 `a+a+` 那样的指数路径。
pub(crate) fn compile(pat: &Pat) -> Vec<Inst> {
    let mut prog = Vec::new();
    emit(pat, &mut prog);
    prog.push(Inst::Match);
    prog
}

fn emit(pat: &Pat, prog: &mut Vec<Inst>) {
    match pat {
        Pat::Class(c) => prog.push(Inst::Char(c.clone())),
        Pat::Lit(s) => {
            for ch in s.chars() {
                prog.push(Inst::Char(Class::One(ch)));
            }
        }
        Pat::Seq(items) => {
            for it in items {
                emit(it, prog);
            }
        }
        Pat::Named { pat, .. } => emit(pat, prog),
        Pat::Alt(alts) => {
            if alts.is_empty() {
                return;
            }
            // 依次：Split(本支, 下一支起点)
            let mut jmps = Vec::new();
            for (i, alt) in alts.iter().enumerate() {
                let split_pc = prog.len();
                if i + 1 < alts.len() {
                    prog.push(Inst::Split(0, 0)); // 占位，稍后回填
                }
                let start = prog.len();
                emit(alt, prog);
                if i + 1 < alts.len() {
                    let jmp_pc = prog.len();
                    prog.push(Inst::Jmp(0));
                    let after = prog.len();
                    prog[split_pc] = Inst::Split(start, after);
                    jmps.push(jmp_pc);
                }
            }
            let end = prog.len();
            for j in jmps {
                prog[j] = Inst::Jmp(end);
            }
        }
        Pat::Repeat { pat, min, max } => {
            match max {
                Some(mx) if *mx >= *min => {
                    for _ in 0..*min {
                        emit(pat, prog);
                    }
                    // 余下 (mx-min) 份：每份用 Split 跳过
                    for _ in 0..(mx - min) {
                        let split_pc = prog.len();
                        prog.push(Inst::Split(0, 0));
                        let start = prog.len();
                        emit(pat, prog);
                        let end = prog.len();
                        prog[split_pc] = Inst::Split(start, end);
                    }
                }
                Some(_) => {
                    // max < min：不可能匹配，退化为「至少 min 次」以给出可诊断行为
                    for _ in 0..*min {
                        emit(pat, prog);
                    }
                }
                None => {
                    // 无上界：min 份 + 一个循环（单条回边，不复制本体）
                    for _ in 0..*min {
                        emit(pat, prog);
                    }
                    let split_pc = prog.len();
                    prog.push(Inst::Split(0, 0));
                    let start = prog.len();
                    emit(pat, prog);
                    prog.push(Inst::Jmp(split_pc));
                    let end = prog.len();
                    prog[split_pc] = Inst::Split(start, end);
                }
            }
        }
        Pat::Until { pat, .. } => {
            // 两条分支：① 终止条件成立 → 结束；② 否则消费一个字符并回到起点。
            //
            // 注意：早期版本把 ② 写成了「直接退出」，导致消费分支位于 Jmp 之后
            // 而永远不可达（until 只对紧邻终止符的输入成立）。这里必须让
            // Split 的第二出口指向「消费字符」，而非 end。
            let split_pc = prog.len();
            prog.push(Inst::Split(0, 0)); // 回填：Split(try_end, consume)
            let try_end = prog.len();
            emit(pat, prog);
            prog.push(Inst::Jmp(0)); // 回填为 end
            let jmp_pc = prog.len() - 1;
            let consume = prog.len();
            prog.push(Inst::Char(Class::Any));
            prog.push(Inst::Jmp(split_pc));
            let end = prog.len();
            prog[split_pc] = Inst::Split(try_end, consume);
            prog[jmp_pc] = Inst::Jmp(end);
        }
    }
}

/// 模拟：并行推进所有线程。返回是否匹配（全文）。
///
/// `budget` 是防御性的：无回溯已保证单次推进是多项式的，但仍给一个
/// 步数上限，避免病态大规则在极端输入下耗时过长。
pub(crate) fn run(prog: &[Inst], text: &str, budget: u64) -> Result<bool, String> {
    let match_pc = prog.len() - 1;
    let mut clist: Vec<usize> = Vec::new();
    let mut nlist: Vec<usize> = Vec::new();
    let mut steps: u64 = 0;

    add_thread(&mut clist, prog, 0);

    for ch in text.chars() {
        nlist.clear();
        for &pc in clist.iter() {
            steps += 1;
            if steps > budget {
                return Err(format!(
                    "sml: 模式匹配超出步数预算 {budget}（疑似病态规则或超长输入）"
                ));
            }
            if let Inst::Char(c) = &prog[pc] {
                if c.matches(ch) {
                    add_thread(&mut nlist, prog, pc + 1);
                }
            }
        }
        std::mem::swap(&mut clist, &mut nlist);
        if clist.is_empty() {
            return Ok(false);
        }
    }
    Ok(clist.contains(&match_pc))
}

/// ε 闭包：把 Split / Jmp 能到达的位置全部加入当前线程集合。
/// 用局部 visited 防止 ε 环导致无限展开。
fn add_thread(list: &mut Vec<usize>, prog: &[Inst], pc: usize) {
    let mut stack = vec![pc];
    let mut seen = vec![pc];
    while let Some(p) = stack.pop() {
        match &prog[p] {
            Inst::Split(x, y) => {
                for &nx in [y, x].iter() {
                    if !seen.contains(&nx) {
                        seen.push(*nx);
                        stack.push(*nx);
                    }
                }
            }
            Inst::Jmp(x) => {
                if !seen.contains(x) {
                    seen.push(*x);
                    stack.push(*x);
                }
            }
            Inst::Char(_) | Inst::Match => {
                if !list.contains(&p) {
                    list.push(p);
                }
            }
        }
    }
}
