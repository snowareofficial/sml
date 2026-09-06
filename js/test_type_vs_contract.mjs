// 实测：type（loom）与 contract 的真实关系
//  1. type 能否脱离 contract 独立声明
//  2. contract 字段引用 type 是否生效（合法/非法值）
//  3. loom 片段（@规则名{}）+ 用: 组合能否独立工作
//  4. 反向：pattern 内引用 contract 会怎样（验证依赖是否单向）
import { parseSafe } from "./sml.mjs";

function run(name, src) {
  const r = parseSafe(src);
  const ok = r.ok;
  console.log(`${ok ? "ok  " : "ERR "}  ${name}${ok ? "" : "  ->  " + r.error}`);
  return r;
}

// 1. 只有 @type，没有任何 contract
run("1. type 独立声明（无 contract）", `
@type name: 手机号 {
    序列: [
        { 字面: "1" }
        { 名: 后续, 类: 数字, 次: 10 }
    ]
}
探针: x
`);

// 2. contract 字段引用 type —— 合法值
run("2a. contract 引用 type（合法值 13800138000）", `
@type name: 手机号 {
    序列: [
        { 字面: "1" }
        { 名: 后续, 类: 数字, 次: 10 }
    ]
}
@contract 办事人 strict {
    姓名: str
    手机: 手机号
}
@is 办事人
姓名: 张三
手机: "13800138000"
`);

// 2b. 非法值（首位不是 1）应被拒
run("2b. contract 引用 type（非法值 23800138000，应报错）", `
@type name: 手机号 {
    序列: [
        { 字面: "1" }
        { 名: 后续, 类: 数字, 次: 10 }
    ]
}
@contract 办事人 strict {
    姓名: str
    手机: 手机号
}
@is 办事人
姓名: 张三
手机: "23800138000"
`);

// 3. loom 纯片段 + 用: 组合（无 @type、无 contract）
run("3. loom 片段 + 用: 组合（无 @type / 无 contract）", `
@日期ISO {
    序列: [
        { 名: 年, 类: 数字, 次: 4 }
        { 字面: "-" }
        { 名: 月, 类: 数字, 次: 2 }
    ]
}
@时间戳 {
    序列: [
        { 名: 日期, 用: 日期ISO }
    ]
}
探针: x
`);

// 4. 反向：pattern 内用: 引用一个 contract（契约不是模式）
run("4. 反向：pattern 内 用: 引用 contract（观察行为）", `
@contract 办事人 strict {
    姓名: str
}
@type name: 怪类型 {
    序列: [
        { 名: 人, 用: 办事人 }
    ]
}
探针: x
`);

// 5. 对照：引用完全不存在的模式名 —— 若也静默通过，说明 用: 存在静默兜底
run("5. 对照：用: 引用完全不存在的名字（应报错）", `
@type name: 怪类型2 {
    序列: [
        { 名: 人, 用: 压根不存在 }
    ]
}
探针: x
`);

// 6. 让契约字段使用第 4 点的“怪类型”，看它真正匹配时是什么行为
run("6. 字段用怪类型匹配值 abc（观察是否放行）", `
@contract 办事人 strict {
    姓名: str
}
@type name: 怪类型 {
    序列: [
        { 名: 人, 用: 办事人 }
    ]
}
@contract 容器 strict {
    项: 怪类型
}
@is 容器
项: abc
`);
