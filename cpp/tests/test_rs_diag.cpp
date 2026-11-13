// SPDX-License-Identifier: MulanPSL-2.0
// test_rs_diag.cpp — Rust cdylib 后端的诊断出口（sml::loads_with_diags）。
//
// 这个用例盯的是**C-ABI 的功能对等**（docs/swsml/plan/crossend-sync.md §0.6 / P7）：
// Rust 的 `parse_with_features_warnings` 能交出的诊断，绑定量必须也能拿到，
// 且**值树与 loads() 逐字节相同**（诊断只报告，不改语义）。
//
// 需要 Rust cdylib：`python cpp/build_verify.py` 会带上 -L$(SML_RUST_LIB) -lsml。
#include "../sml_rs.hpp"

#include <cstdio>
#include <cstdlib>
#include <string>

static int failures = 0;

static void check(bool cond, const char *what, const std::string &detail = "") {
    if (cond) {
        std::printf("  ok: %s\n", what);
    } else {
        std::printf("FAIL: %s%s%s\n", what, detail.empty() ? "" : " -> ", detail.c_str());
        failures++;
    }
}

int main() {
    std::printf("[RS-DIAG] diagnostics over the C ABI\n");

    // ① 重复键：带码、带行号、不阻断解析
    {
        const std::string src = "a: 1\na: 2\n";
        sml::DiagLoadResult r = sml::loads_with_diags(src);
        bool ok = r.ok() && r.diagnostics.size() == 1
            && r.diagnostics[0].code_str == "W-PARSE-001"
            && r.diagnostics[0].line == 2
            && r.diagnostics[0].severity == sml::Severity::Warning;
        check(ok, "重复键 -> W-PARSE-001 @ line 2",
              ok ? "" : ("n=" + std::to_string(r.diagnostics.size()) + " code=" +
                         (r.diagnostics.empty() ? std::string("<none>") : r.diagnostics[0].code_str)));

        // 值树与 loads() 逐字节相同
        sml::LoadResult plain = sml::loads(src);
        check(r.ok() && plain.ok() && r.value.dumps() == plain.value.dumps(),
              "值树与 loads() 逐字节相同");
    }

    // ② 片段覆盖（真丢值）：W-PARSE-002
    {
        sml::DiagLoadResult r = sml::loads_with_diags(
            "@base { x: 1 y: 2 }\nw {\n  x: 9\n  &base\n}\n");
        bool ok = r.ok() && r.diagnostics.size() == 1
            && r.diagnostics[0].code_str == "W-PARSE-002";
        check(ok, "片段覆盖 -> W-PARSE-002",
              ok ? "" : ("n=" + std::to_string(r.diagnostics.size())));
    }

    // ③ 干净文档：空诊断表，值仍可用
    {
        sml::DiagLoadResult r = sml::loads_with_diags("server { port: 8080 }\n");
        auto port = r.value.path("server.port").as_int();
        check(r.ok() && r.diagnostics.empty() && port == 8080,
              "干净文档 -> 零诊断，值可读");
    }

    // ④ 失败路径：致命错误只走 error，诊断表为空
    {
        sml::DiagLoadResult r = sml::loads_with_diags("a {\n");
        check(!r.ok() && r.diagnostics.empty() && !r.error.code_str.empty(),
              "语法错误 -> 空诊断 + 带码的 error",
              r.error.code_str);
    }

    // ⑤ 内置契约表注入：`W-CONTRACT-001` 经 C-ABI 真的可达（声明侧 + 块内容侧）
    {
        sml::Builtin bt = sml::Builtin::create();
        check(bt.valid() && bt.count() == 0, "空表 -> count()==0");
        bool built = bt.add_field("tls", "cert", "str", true)
            && bt.add_field("tls", "port", "int", false)
            && bt.set_range("tls", "port", 1, 65535)
            && bt.add_field("tls", "mode", "enum", false)
            && bt.add_enum("tls", "mode", "strict");
        check(built && bt.count() == 1, "建表 -> 1 份契约（tls）");
        check(!bt.add_field("tls", "bad", "image", false),
              "未知类型 -> 拒绝且不动表");

        sml::Opts opts;
        // `type.weak` 是 opt-in、不在基线里；flags 是**特性上限**，必须显式允许
        opts.flags = SML_F_BASIC | SML_F_WEAK;
        opts.builtins = &bt;
        sml::DiagLoadResult r = sml::loads_full(
            "@feature enable type.weak\n"
            "srv {\n  @weakbind tls\n  web { cert: 1 port: 99999 mode: nope }\n}\n"
            "@weakbind nosuch\nother { x: 1 }\n",
            opts);
        int weak = 0;
        for (const auto &d : r.diagnostics) {
            if (d.code_str == "W-CONTRACT-001") weak++;
        }
        check(r.ok() && weak == 4 && r.diagnostics.size() == 4,
              "内置表 -> 4 条 W-CONTRACT-001（类型/区间/枚举/未知契约）",
              "n=" + std::to_string(r.diagnostics.size()));
        // 只读：值树没被填默认值 / 改类型
        auto cert = r.value.path("srv.web.cert").as_int();
        check(cert.has_value() && *cert == 1, "软校验只读：cert 仍是 int 1");
    }

    // ⑥ env 注入：走覆盖表，不碰进程环境
    {
        sml::Opts opts;
        opts.env.push_back("SML_CPP_ENV_PROBE=prod");
        sml::DiagLoadResult r = sml::loads_full("k: $env.SML_CPP_ENV_PROBE\n", opts);
        auto k = r.value.path("k").str();
        check(r.ok() && k.has_value() && *k == "prod", "env 注入 -> k=prod");
    }

    // ⑦ 坏选项要响亮失败：struct_size 由封装层填，故只验「未知类型不被静默接受」已在上文
    {
        sml::Opts opts; // flags=0 = 基线，没有 type.weak
        sml::DiagLoadResult r = sml::loads_full("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n", opts);
        check(!r.ok() && r.error.code_str == "E-FEATURE-001",
              "基线不含 type.weak -> E-FEATURE-001（不是静默忽略）",
              r.error.code_str);
    }

    // ⑧ 弱声明三表：@weakbind / @weakdata / @weakdoc 都不进主树，从这里交出
    {
        sml::Opts opts;
        opts.flags = SML_F_BASIC | SML_F_WEAK;
        sml::WeakLists lists;
        sml::DiagLoadResult r = sml::loads_full(
            "@feature enable type.weak\n"
            "srv {\n  @weakbind tls\n  web { cert: \"x\" }\n"
            "  @weakdata limits { max: 10 }\n}\n"
            "@weakdoc base\n",
            opts, &lists);
        check(r.ok(), "弱表：解析成功", r.error.text);
        check(lists.n_binds() == 1 && lists.n_data() == 1 && lists.n_docs() == 1,
              "弱表：1 绑定 / 1 数据 / 1 profile",
              std::to_string(lists.n_binds()) + "/" + std::to_string(lists.n_data()) + "/" +
                  std::to_string(lists.n_docs()));
        if (lists.n_binds() == 1) {
            const sml_weakbind &b = lists.binds()[0];
            check(std::string(b.path ? b.path : "") == "srv.web"
                      && std::string(b.name ? b.name : "") == "tls" && b.line > 0,
                  "弱表：bind 的 path/name/line",
                  std::string(b.path ? b.path : "?") + " " + std::string(b.name ? b.name : "?"));
        }
        if (lists.n_data() == 1) {
            const sml_weakdata &d = lists.data()[0];
            sml::Ref payload(d.value);
            auto maxv = payload.path("max").as_int();
            check(std::string(d.path ? d.path : "") == "srv"
                      && std::string(d.name ? d.name : "") == "limits",
                  "弱表：weakdata 归属 srv/limits",
                  std::string(d.path ? d.path : "?"));
            check(maxv.has_value() && *maxv == 10, "弱表：weakdata 载荷可读（max=10）");
        }
        // 主树里查不到任何弱声明的痕迹
        check(r.value.path("srv.limits").valid() == false
                  && r.value.path("srv.web.__weak").valid() == false,
              "弱表：主树无痕迹");
        lists.reset();
        check(lists.n_binds() == 0 && lists.binds() == nullptr, "弱表：reset 后清零");
    }

    // ⑨ 文件入口：展开 include + 诊断 + 弱表
    {
        std::string dir = "t_rs_diag_tmp";
        std::string sub = dir + "/sub.sml";
        std::string main = dir + "/main.sml";
        // 仓库纪律：临时文件落在构建目录旁，跑完删掉
        std::string mk = "mkdir " + dir;
        std::string rm = "rmdir /s /q " + dir + " 2>NUL";
        if (std::system(mk.c_str()) != 0) {
            check(false, "文件入口：建立临时目录", dir);
        } else {
            FILE *f = std::fopen(sub.c_str(), "wb");
            if (f) {
                std::fputs("k: 1\n", f);
                std::fclose(f);
            }
            f = std::fopen(main.c_str(), "wb");
            if (f) {
                std::fputs("@feature enable type.weak\ninclude \"sub.sml\"\nm: 2\n@weakdoc base\n", f);
                std::fclose(f);
            }
            sml::Opts opts;
            opts.flags = SML_F_BASIC | SML_F_WEAK;
            sml::WeakLists lists;
            sml::DiagLoadResult r = sml::load_file_full(main, opts, &lists);
            check(r.ok() && r.value.path("k").as_int() == 1
                      && r.value.path("m").as_int() == 2,
                  "文件入口：include 已展开", r.error.text);
            check(lists.n_docs() == 1
                      && std::string(lists.docs()[0].name ? lists.docs()[0].name : "") == "base",
                  "文件入口：@weakdoc 进了弱表");
            std::system(rm.c_str());
        }
    }

    // ⑩ 聚合结果：契约表 + 契约映射 + 多错误，一次解析全都拿到
    {
        sml::Opts o;
        o.flags = SML_F_BASIC | SML_F_CONTRACT;
        sml::Result r = sml::Result::parse(
            "@contract tls { cert: str }\nsrv { @is tls\n  cert: \"x\"\n}\n", o,
            sml::WantDiags | sml::WantContractMap | sml::WantContracts);
        check(r.valid(), "结果句柄：解析成功");
        check(r.value().path("srv.cert").str().value_or("") == "x", "结果句柄：值树可读");

        const sml_contractmap *cm = r.contract_map();
        bool mapOk = cm && cm->len == 1 && std::string(cm->items[0].path) == "srv"
            && std::string(cm->items[0].name) == "tls";
        check(mapOk, "结果句柄：契约映射 srv -> tls",
              cm ? std::to_string(cm->len) + " 条" : "null");

        const sml_builtin *captured = r.contracts();
        check(captured != nullptr, "结果句柄：契约表已捕获");

        // §6.1 的完整闭环：用 SML 写的契约表 → 再当内置表注入 → 弱绑定软校验命中
        sml::Builtin borrowed = sml::Builtin::borrow(captured);
        check(borrowed.count() == 1, "捕获的契约表可当内置表用（1 份）",
              std::to_string(borrowed.count()));
        sml::Opts o2;
        o2.flags = SML_F_BASIC | SML_F_WEAK;
        o2.builtins = &borrowed;
        sml::DiagLoadResult w = sml::loads_full(
            "@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n", o2);
        int weak = 0;
        for (const auto &d : w.diagnostics) {
            if (d.code_str == "W-CONTRACT-001") weak++;
        }
        check(w.ok() && weak == 1, "闭环：注入捕获的表后报 decl.unknown_contract", std::to_string(weak));

        // 克隆：脱离结果句柄独立存在（`sml_builtin_clone` 的存在理由）
        sml::Builtin owned = r.contracts_owned();
        check(owned.count() == 1, "克隆的契约表可独立存在", std::to_string(owned.count()));
    }

    // ⑪ 多错误：`SML_WANT_ERRORS` 让语法/契约错也返回句柄，错误表 severity = 1
    {
        sml::Opts o;
        o.flags = SML_F_BASIC | SML_F_CONTRACT;
        sml::Error err;
        sml::Result r = sml::Result::parse(
            "a { @is NoSuch1\n  x: 1 }\nb { @is NoSuch2\n  y: 2 }\n", o,
            sml::WantErrors, &err);
        check(r.valid(), "多错误：仍返回句柄（不因首个错误 NULL）", err.text);
        auto errs = r.errors();
        check(errs.size() == 2, "多错误：收到 2 条", std::to_string(errs.size()));
        bool sevOk = !errs.empty();
        for (const auto &d : errs) {
            if (d.severity != sml::Severity::Error) sevOk = false;
            if (d.code_str.empty()) sevOk = false;
        }
        check(sevOk, "多错误：severity == Error 且带码");
        check(!errs.empty() && errs[0].code_str == "E-CONTRACT-001",
              "多错误：码是 E-CONTRACT-001", errs.empty() ? "" : errs[0].code_str);

        // 不请求 ERRORS 时行为与 sml_parse_full 一致：首个错误即 NULL + err
        sml::Error err2;
        sml::Result r2 = sml::Result::parse(
            "a { @is NoSuch1\n  x: 1 }\n", o, sml::WantDiags, &err2);
        check(!r2.valid() && err2.code_str == "E-CONTRACT-001", "未请求 ERRORS -> NULL + 带码 err",
              err2.code_str);
    }

    if (failures == 0) {
        std::printf("ALL RS-DIAG TESTS PASSED\n");
        return 0;
    }
    std::printf("%d FAILURES\n", failures);
    return 1;
}
