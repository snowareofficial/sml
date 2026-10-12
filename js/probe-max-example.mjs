// SPDX-License-Identifier: MulanPSL-2.0
// 能力上限示例的 golden 探针（JS 端）：把「全特性」文档的解析结论钉住、防回归。
//
//   node js/probe-max-example.mjs
//
// 一句话：SML 能写多复杂的文档 —— 这份微服务平台配置同时用到
//   typed-block / @is 单契约 / @is 叠加(mixin) / 组合(形状+1) / @type 模式 /
//   片段 &base / $env / enum / 数组 / 默认值回填 / 契约映射(叠加记多名)。
//
// 三个已记边界（**不在本探针断言**，仅注释说明，供读者知悉）：
//   ① 叠加（mixin，`@is a b`）**仅 Rust 实现**；JS 对它**响亮拒绝** E-CONTRACT-017
//      （本探针末尾有一条断言钉住这点）。Rust 侧还须走**带行号**的 parse 入口 ——
//      `smltools -i 文件`（parse_file）跨文件 token 流无行号，`@is` 多词按「宁少收」
//      退化为单契约。故示例的叠加只在 Rust `parse`（如 stdin）生效。
//   ② 顶层数组不能跟在 `@contract` 等指令之后（Rust：指令后顶层按块解析，`[` 当键报
//      E-PARSE-006）—— 故示例用顶层块 `platform { }` 承载，数组在块内展示。
//   ③ JS **不递归回填**被引用契约（组合）的默认值：顶层契约默认值会回填（如 log_level），
//      但嵌套组合（db: Database）的默认值 JS 暂不回填（Rust 会）—— 跨端差异。

import { parse, contractMapToSml } from "./sml.mjs";

// $env 走 process.env / globalThis.__SML_ENV__（见 sml.mjs envLookup）；
// 探针用后者 —— 确定、不依赖真实环境变量。
globalThis.__SML_ENV__ = { DEPLOY_API_KEY: "sk-demo-123456" };

const SRC = `@version v1
@feature enable typed-block

@base {
    region: cn-north-1
    owner: 平台组
    api_key: $env.DEPLOY_API_KEY
}

@type name: semver {
    序列: [ { 类: 数字, 次: "+" } { 字面: "." } { 类: 数字, 次: "+" } { 字面: "." } { 类: 数字, 次: "+" } ]
}

@contract Observable loose {
    metrics: bool default true
    log_level: enum(debug, info, warn, error) default info
}
@contract Scalable loose {
    replicas: int default 2 min 0 max 50
    hpa: bool default false
}
@contract Database {
    engine: enum(postgres, mysql)
    port: int default 5432 min 1 max 65535
}
@contract Service loose {
    name: str
    version: semver
    db: Database
    tags: [str] optional
}

platform {
    Service api {
        name: gateway
        version: 1.4.2
        db: { engine: postgres port 6432 }
        tags: [ edge public ]
        &base
    }
    worker {
        @is Service
        name: worker
        version: 0.9.0
        db: { engine: mysql }
        &base
    }
    cache {
        @is Observable               # 单契约（叠加 mixin 仅 Rust 实现，见文件头边界①）
        name: redis
        metrics: false
        replicas: 3
    }
}
`;

let bad = 0;
const ok = (c, m, extra = "") => { if (c) console.log("ok    " + m); else { bad++; console.log("FAIL  " + m + (extra ? "  " + extra : "")); } };
const K = (s) => ({ key: s });
const pathEq = (a, b) => {
  if (a.length !== b.length) return false;
  for (let k = 0; k < a.length; k++) {
    const x = a[k], y = b[k];
    const xi = x.index !== undefined, yi = y.index !== undefined;
    if (xi !== yi) return false;
    if (xi) { if (x.index !== y.index) return false; } else { if (x.key !== y.key) return false; }
  }
  return true;
};
const namesAt = (map, path) => { for (const e of map) if (pathEq(e.path, path)) return e.names; return null; };

const r = parse(SRC, { traceContracts: true });
if (!r || r.value === undefined) { console.log("FAIL  解析失败"); process.exit(1); }
const v = r.value;
const map = r.contractMap;
const P = v.platform;

ok(!!P, "全特性文档解析成功，platform 根存在");

// typed-block：`Service api { }` → __type / __name
ok(!!P.Service && P.Service.__type === "Service" && P.Service.__name === "api",
   "typed-block：Service api 记 __type/__name");

// 组合（形状+1）：db 识别为 Database。⚠️ 边界③：JS **不递归回填**被引用契约的默认值
// （worker.db 缺 port 仍是缺；Rust 会回填 5432）—— 顶层契约默认值 JS 会回填（见 log_level），
// 但**嵌套组合**的默认值 JS 暂不回填，属跨端差异。
ok(!!P.worker && P.worker.db && P.worker.db.engine === "mysql",
   "组合 Database：worker.db 识别为 Database（engine=mysql）", JSON.stringify(P.worker && P.worker.db));

// $env 注入（经 &base 片段带入）
ok(!!P.Service && P.Service.api_key === "sk-demo-123456", "$env.DEPLOY_API_KEY 注入 api_key");

// 片段 &base 展开（region / owner 进块内；中文标识符原样）
ok(!!P.Service && P.Service.region === "cn-north-1" && P.Service.owner === "平台组",
   "片段 &base 展开 region/owner（中文裸词）");

// @type 模式校验 + 值
ok(!!P.Service && P.Service.version === "1.4.2", "@type semver：version 过模式校验");

// 数组
ok(!!P.Service && Array.isArray(P.Service.tags) && P.Service.tags.join(",") === "edge,public",
   "数组 tags: [ edge public ]");

// 单契约 @is + 默认值回填：cache 拿 Observable 的 log_level 默认；replicas 为额外字段（loose 放行）
ok(!!P.cache && P.cache.log_level === "info", "@is 单契约：Observable 默认 log_level=info");
ok(!!P.cache && P.cache.metrics === false && P.cache.replicas === 3, "@is 单契约：显式字段 metrics/replicas（loose 允额外）");

// 契约映射（W24 API）：单契约/typed-block 各记一个名字
ok((namesAt(map, [K("platform"), K("cache")]) || []).join(",") === "Observable",
   "映射：cache 记 Observable（@is 单契约）", JSON.stringify(namesAt(map, [K("platform"), K("cache")])));
ok((namesAt(map, [K("platform"), K("worker")]) || []).join(",") === "Service",
   "映射：worker 记 Service（@is 单契约）");
ok((namesAt(map, [K("platform"), K("Service")]) || []).join(",") === "Service",
   "映射：Service 块记 Service（typed-block）");

// 确定性：同输入两次解析逐字节相同；契约映射 to_sml 稳定
ok(JSON.stringify(parse(SRC, { traceContracts: true }).value) === JSON.stringify(v),
   "确定性：同输入两次解析一致");
ok(contractMapToSml(map) === contractMapToSml(map), "契约映射 to_sml 确定（同输入两次相等）");

// 边界①钉子：JS 端对多契约写法**响亮拒绝** E-CONTRACT-017（叠加仅 Rust 实现，绝不静默给树）
{
  let code = null;
  try {
    parse("@contract A loose { x: int }\n@contract B loose { y: int }\nfoo { @is A B x: 1 y: 2 }\n", {});
  } catch (e) { code = e.code || null; }
  ok(code === "E-CONTRACT-017", "边界①：JS 对 @is 多契约响亮拒绝 E-CONTRACT-017（叠加仅 Rust）", String(code));
}

console.log(bad === 0 ? "ALL OK" : (bad + " FAILED"));
process.exit(bad === 0 ? 0 : 1);
