// sml-errors.mjs 查找逻辑自测（i18n）：中文默认、英文翻译、缺失回退、locale 归一、未知码。
//   node js/test-sml-errors.mjs
// 与生成器 errors/gen_js_errors.py 的 --check 互补：那边保「模块与数据一致」，这边保「查找逻辑正确」。
import {
  errorInfo, errorMessage, errorTitle, listErrorCodes, DEFAULT_LOCALE,
} from "./sml-errors.mjs";

let bad = 0;
const eq = (label, got, want) => {
  const ok = got === want;
  if (!ok) bad++;
  console.log(`${ok ? "ok  " : "FAIL"} ${label} => ${JSON.stringify(got)}${ok ? "" : "（期望 " + JSON.stringify(want) + "）"}`);
};

eq("默认 locale 是 zh", DEFAULT_LOCALE, "zh");

const zh = errorInfo("E-PARSE-006");
eq("zh title", zh && zh.title, "期望键或标识符");
eq("zh domainName", zh && zh.domainName, "语法");
eq("zh severity", zh && zh.severity, "error");

const en = errorInfo("E-PARSE-006", { locale: "en" });
eq("en title", en && en.title, "Expected key or identifier");
eq("en domainName", en && en.domainName, "Syntax");
eq("en msg 非空", typeof (en && en.msg) === "string" && en.msg.length > 0, true);

eq("locale 归一 en-US", errorTitle("E-LEX-001", { locale: "en-US" }), "Unterminated string");
eq("locale 归一 zh-CN", errorTitle("E-LEX-001", { locale: "zh-CN" }), "字符串未闭合");
eq("未知 locale 回退 zh", errorTitle("E-LEX-001", { locale: "fr" }), "字符串未闭合");

eq("未知码 errorInfo", errorInfo("E-NOPE-999"), null);
eq("未知码 errorMessage", errorMessage("E-NOPE-999"), null);
eq("空入参 errorInfo", errorInfo(null), null);
eq("errorMessage zh", errorMessage("E-PARSE-006"), "期望键或标识符，得其它记号");

// 每条码都查得到，且 title/msg 非空（中文源全覆盖）
let empty = 0;
for (const c of listErrorCodes()) {
  const i = errorInfo(c.id);
  if (!i || !i.title || !i.msg) empty++;
}
eq("所有码 title/msg 非空", empty, 0);
eq("PARSE 领域码数", listErrorCodes({ domain: "PARSE" }).length, 25);

console.log(bad === 0 ? "ALL OK" : `${bad} FAILED`);
process.exit(bad === 0 ? 0 : 1);
