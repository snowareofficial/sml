// sml-verify.js — 每章 / 每挑战的验证函数
// 接收 SML 解析结果 v（普通 JS 对象），返回 { ok: boolean, msg: string }
// 由 sml-playground.html / sml-challenge.html 通过 import 调用。

function get(o, path) {
  if (o == null) return undefined;
  return String(path).split(".").reduce((a, k) => (a == null ? a : a[k]), o);
}
function arrHas(arr, val) {
  return Array.isArray(arr) && arr.some((x) => x === val);
}
function isObj(x) {
  return x != null && typeof x === "object" && !Array.isArray(x);
}

// ---- 每章练习验证（key 与 sml-lessons.json 一致）----
export const VERIFIERS = {
  intro(v) {
    const ok = isObj(v) && v.name === "gateway" && v.port === 8080 && v.debug === true &&
      Array.isArray(v.tags) && arrHas(v.tags, "logging") && arrHas(v.tags, "metrics");
    return ok
      ? { ok, msg: "✓ 全部正确：name / port(整数) / debug / tags 都对了。" }
      : { ok, msg: "还差一点：需要 name=gateway、port=8080(整数而非字符串)、debug=true、tags 含 logging 与 metrics。" };
  },
  ch01(v) {
    const ok = isObj(v) && v.firstName === "John Doe" && v.state === "NY" && v.age === 27 &&
      v.ratio === 0.75 && v.enabled === true && v.note === null;
    return ok
      ? { ok, msg: "✓ 漂亮：裸词 NY、整数 27、浮点 0.75、布尔 true、null 都识别对了。" }
      : { ok, msg: "检查：firstName 需加引号(\"John Doe\")；state 用裸词 NY；age=27(整数)；ratio=0.75；enabled=true；note=null。" };
  },
  ch02(v) {
    const ok = isObj(v) && get(v, "address.city") === "New York" && get(v, "address.state") === "NY" &&
      get(v, "database.primary.port") === 5432 && get(v, "database.replica.port") === 5432 &&
      Array.isArray(v.users) && v.users.length >= 2 && get(v, "users.0.name") === "alice";
    return ok
      ? { ok, msg: "✓ 嵌套块、数组、行内对象全部正确。" }
      : { ok, msg: "检查：address 块含 city=\"New York\"/state=NY；database.primary 与 replica 各含 host/port；users 是含 name/role 的块数组。" };
  },
  ch03(v) {
    const svc = isObj(v) ? v.service : null;
    const ok = Array.isArray(svc) && svc.length >= 2 &&
      get(svc, "0.region") === "cn-north-1" && get(svc, "0.port") === 7100 && get(svc, "0.name") === "auth-svc" &&
      get(svc, "1.port") === 7200 && get(svc, "1.name") === "billing-svc";
    return ok
      ? { ok, msg: "✓ 片段继承生效：公共字段被复用，本地字段覆盖了 port/name。" }
      : { ok, msg: "检查：@base 定义 region/timeout，service auth/billing 用 &base 并覆盖 port 与 name（service 是数组，元素含 region/port/name）。" };
  },
  ch04(v) {
    const ok = isObj(v) && v.title === "My App" && v.powered === "Snoware";
    return ok
      ? { ok, msg: "✓ include 命名空间正确：通过 ui.title / ui.powered 访问到了 ui.sml 的内容。" }
      : { ok, msg: "检查：include \"ui\" as ui 后，用 ui.title 与 ui.powered 引用被包含文件的字段。" };
  },
  ch05(v) {
    const ok = isObj(v) && v.name === "大厅" && v.port === 25565 && v.region === "ap-east-1";
    return ok
      ? { ok, msg: "✓ 契约校验通过且字段值正确（strict 模式下未出现多余字段）。" }
      : { ok, msg: "检查：@contract Server strict 声明了 name/port/region；@is Server 后填 name=大厅、port=25565、region=ap-east-1。若报契约错误，看 port 是否在 [1024,65535]。" };
  },
  ch06(v) {
    const ok = isObj(v) && get(v, "secrets.apiKey") === "" && get(v, "secrets.dbPassword") === "" &&
      typeof v.banner === "string" && v.banner.includes("🚀");
    return ok
      ? { ok, msg: "✓ $env 在浏览器里解析为空串（预期），\\u{1F680} 成功转义成 🚀。" }
      : { ok, msg: "检查：secrets 块内 apiKey/dbPassword 用 $env.X（前端取不到→空串）；banner 用 \"SML \\u{1F680} 上线\" 转义 emoji。" };
  },
  ch07(v) {
    const ok = isObj(v) && v.id === "u_1" && v.email === "a@b.c" && v.role === "user";
    return ok
      ? { ok, msg: "✓ 契约 + 默认值生效：未写 role 时自动取默认 user。" }
      : { ok, msg: "检查：@contract User 含 id/email/role(enum 默认 user)；@is User 写 id=u_1、email=a@b.c，role 省略即取默认。" };
  },
  ch08(v) {
    const svc = isObj(v) ? v.service : null;
    const ok = isObj(v) && v.name === "gateway" && v.port === 9090 && v.debug === true &&
      Array.isArray(svc) && get(svc, "0.port") === 7100 && get(svc, "1.name") === "billing-svc" &&
      get(v, "database.pool.min") === 2 && Array.isArray(v.features) && v.features.length === 3;
    return ok
      ? { ok, msg: "✓ 综合实战通过：契约 + 片段 + $env + 数组全部正确。" }
      : { ok, msg: "检查：name=gateway/port=9090/debug=true；service 是数组，auth 元素 port=7100、billing 元素 name=billing-svc；database.pool.min=2；features 长度 3。" };
  },
  ch09(v) {
    const ok = isObj(v) && v.name === "gateway" && isObj(v.main) && typeof v.main.port === "number" &&
      Array.isArray(v.peers) && v.peers.length >= 1;
    return ok
      ? { ok, msg: "✓ 契约组合正确：Endpoint 被 main 字段与 array[Endpoint] 复用。" }
      : { ok, msg: "检查：@contract Endpoint{host,port}，@contract Service{name,main:Endpoint,peers:array[Endpoint] ?}；@is 后 main 是块、peers 是块数组。" };
  },
  ch10(v) {
    const ok = isObj(v) && isObj(v.auth) && v.auth.name === "auth" &&
      isObj(v.billing) && v.billing.name === "billing";
    return ok
      ? { ok, msg: "✓ 命名空间正确：auth.sml 与 billing.sml 分别装入 auth / billing 命名空间并各自 @is 校验。" }
      : { ok, msg: "检查：include \"auth.sml\" as auth 与 include \"billing.sml\" as billing；各子文件用 @is Module 且含 name 字段。" };
  },
  appendix(v) {
    const ok = isObj(v) && v.name === "John" && v.age === 27 && get(v, "address.city") === "NY" &&
      Array.isArray(v.tags) && v.tags.length === 2;
    return ok
      ? { ok, msg: "✓ 跨语言同构验证通过：这段 SML 在任意语言解析结果都一致。" }
      : { ok, msg: "检查：name=John、age=27、address.city=NY、tags 是长度 2 的数组。" };
  },
};

// ---- 翻译挑战验证（key 与 sml-challenges.json 一致）----
export const CH_VERIFIERS = {
  caddyfile(v) {
    const blk = isObj(v) ? v["example.com"] : null;
    const ok = isObj(blk) && blk.reverse_proxy === "localhost:8080" && blk.tls === "internal" && blk.encode === "gzip";
    return ok
      ? { ok, msg: "✓ 完美：Caddyfile 的块与指令都被等价翻译成了 SML 嵌套块与键值。" }
      : { ok, msg: "检查：example.com 是块，内含 reverse_proxy: localhost:8080、tls: internal、encode: gzip。" };
  },
  "docker-compose"(v) {
    const s = isObj(v) ? v.services : null;
    const ok = isObj(s) && s.web && s.web.image === "nginx:1.27" && arrHas(s.web.ports, "80:80") &&
      arrHas(s.web.depends_on, "db") && s.db && s.db.image === "postgres:16" &&
      isObj(s.db.environment) && s.db.environment.POSTGRES_PASSWORD === "secret";
    return ok
      ? { ok, msg: "✓ 高难度通过：YAML 的数组/嵌套块被准确映射成 SML。" }
      : { ok, msg: "检查：services.web.image=nginx:1.27、ports 含 \"80:80\"、depends_on 含 db；services.db.image=postgres:16、environment.POSTGRES_PASSWORD=secret。" };
  },
  nginx(v) {
    const s = isObj(v) ? v.server : null;
    const ok = isObj(s) && s.listen === 80 && s.server_name === "example.com" &&
      isObj(s.location) && s.location["/"] && s.location["/"].proxy_pass === "http://app:3000" &&
      s.location["/api"] && s.location["/api"].proxy_pass === "http://api:4000";
    return ok
      ? { ok, msg: "✓ 高难度通过：nginx 的 listen/server_name/location 都被等价翻译。" }
      : { ok, msg: "检查：server.listen=80(整数)、server_name=example.com；location 是块，\"/\" 与 \"/api\" 各含 proxy_pass（值含空格需引号）。" };
  },
};
