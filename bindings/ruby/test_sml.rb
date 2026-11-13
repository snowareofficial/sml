# SPDX-License-Identifier: MulanPSL-2.0
# SML Ruby 绑定自测。
# 仓库：https://gitee.com/snoware/sml
# @version 0.1.0 (swsml 0.6.3)

# frozen_string_literal: true
# SML Ruby 绑定测试（§4 黄金向量 + 内存稳定）。
# 运行：ruby bindings/ruby/test_sml.rb  （需 SML_LIB 或 PATH 能找到 libsml）

$LOAD_PATH.unshift(File.expand_path("lib", __dir__))
require "sml"
require "tempfile"

include Sml

GOLDEN = <<~SML
  server {
    host: "example.com"
    port: 8080
    ratio: 0.75
    tls: false
    tags: [ a b c ]
  }
SML

EXPECTED = {
  "server" => {
    "host" => "example.com",
    "port" => 8080,
    "ratio" => 0.75,
    "tls" => false,
    "tags" => ["a", "b", "c"]
  }
}

$failures = 0
def check(name, cond, detail = "")
  if cond
    puts "  PASS  #{name}"
  else
    puts "  FAIL  #{name}  #{detail}"
    $failures += 1
  end
end

begin
  # 1. loads 黄金向量（类型保真）
  obj = Sml.loads(GOLDEN)
  check("1 loads(golden)==expected (port int, ratio float, tls bool, tags 3-elem)",
        obj == EXPECTED && obj["server"]["port"].is_a?(Integer) &&
        obj["server"]["ratio"].is_a?(Float) && obj["server"]["tls"] == false &&
        obj["server"]["tags"].length == 3, obj.inspect)

  # 2. 往返
  back = Sml.loads(Sml.dumps(obj))
  check("2 round-trip dumps/loads", back == EXPECTED, back.inspect)

  # 3. 富错误：未闭合块 -> E-PARSE-001 + text 非空（EOF 错 line 为 0，不强制 line>0）
  begin
    Sml.loads("bad { a: 1")
    check("3 rich error (unclosed)", false, "未抛异常")
  rescue Sml::SmlError => e
    check("3 rich error code_str==E-PARSE-001", e.code_str == "E-PARSE-001", e.code_str.inspect)
    check("3 rich error text 非空", !e.text.empty?, e.text.inspect)
    check("3 不是通用 parse failed", !e.text.downcase.include?("parse failed"), e.text)
  end

  # 4. 错误定位：第 2 行契约错误 -> line==2, E-CONTRACT-001
  begin
    Sml.loads("a: 1\n@is NoSuchContract\n")
    check("4 line location", false, "未抛异常")
  rescue Sml::SmlError => e
    check("4 line==2", e.line == 2, "line=#{e.line}")
    check("4 code_str==E-CONTRACT-001", e.code_str == "E-CONTRACT-001", e.code_str.inspect)
  end

  # 5. load_file + include
  Dir.mktmpdir do |d|
    mainp = File.join(d, "_inc_main.sml")
    subp = File.join(d, "_inc_sub.sml")
    File.write(subp, "k: 1\n")
    File.write(mainp, "include \"_inc_sub.sml\"\nm: 2\n")
    inc = Sml.load_file(mainp)
    check("5 include: 子文件字段 k=1", inc["k"] == 1, inc.inspect)
    check("5 include: 主文件字段 m=2", inc["m"] == 2, inc.inspect)
  end

  # 6. flags 沙箱
  env_doc = "x: $env.NOPE\n"
  begin
    Sml.loads(env_doc, 0)
    base_ok = true
  rescue Sml::SmlError => e
    base_ok = (e.code_str != "E-FEATURE-002")
  end
  check("6 baseline(flags=0) 不因特性报错", base_ok)

  no_env = Sml::SML_F_BASIC & ~Sml::SML_F_ENV
  begin
    Sml.loads(env_doc, no_env)
    check("6 sandbox: 无 ENV 应报错", false, "未抛异常")
  rescue Sml::SmlError => e
    check("6 sandbox code_str==E-FEATURE-002", e.code_str == "E-FEATURE-002", e.code_str.inspect)
  end

  # 7. version / features
  v = Sml.version
  check("7 version() 非空", !v.empty?, v.inspect)
  feats = Sml.features
  check("7 features() 含 'include'", feats.include?("include"), feats.inspect)
  check("7 features() 含 'env'/'contract'",
        feats.include?("env") && feats.include?("contract"), feats.inspect)

  # 8. 内存稳定
  1000.times { Sml.loads(GOLDEN) }
  check("8 1000x loads 不崩溃", true)

  # 9. get_in
  check("9 get_in 路径取值", Sml.get_in(obj, "server.host") == "example.com")

  # 10. 诊断出口（P7）：带码带行号、不阻断解析、值树与 loads 相同
  dup, diags = Sml.loads_with_diags("a: 1\na: 2\n")
  check("10 重复键 -> 1 条 W-PARSE-001 @ line 2",
        diags.length == 1 && diags[0].code_str == "W-PARSE-001" &&
          diags[0].line == 2 && diags[0].severity.zero?, diags.inspect)
  check("10 值树不变（仍是数组提升）", dup == { "a" => [1, 2] }, dup.inspect)
  clean, none = Sml.loads_with_diags("k: 1\n")
  check("10 干净文档 -> 零诊断", clean == { "k" => 1 } && none.empty?, none.inspect)
  200.times { Sml.loads_with_diags("a: 1\na: 2\n") }
  check("10 200x loads_with_diags 不崩溃", true)

  # 11. 完整选项（P7 第二片）：内置契约表 + env 注入
  bt = Sml::Builtin.new
  begin
    check("11 空表 size==0", bt.size.zero?, bt.size.inspect)
    built = bt.add_field("tls", "cert", "str", required: true) &&
            bt.add_field("tls", "port", "int") &&
            bt.set_range("tls", "port", 1, 65535) &&
            bt.add_field("tls", "mode", "enum") &&
            bt.add_enum("tls", "mode", "strict")
    check("11 建表 -> 1 份契约", built && bt.size == 1, bt.size.inspect)
    check("11 未知类型被拒（表不动）", !bt.add_field("tls", "bad", "image"))

    src = "@feature enable type.weak\n" \
          "srv {\n  @weakbind tls\n" \
          "  web { cert: 1 port: 99999 mode: nope }\n}\n" \
          "@weakbind nosuch\nother { x: 1 }\n"
    obj2, diags2 = Sml.loads_full(src, flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK,
                                       builtins: bt)
    weak = diags2.select { |d| d.code_str == "W-CONTRACT-001" }
    check("11 内置表 -> 4 条 W-CONTRACT-001", weak.length == 4, diags2.inspect)
    check("11 软校验只读：cert 仍是 int 1", obj2["srv"]["web"]["cert"] == 1, obj2.inspect)
    check("11 值树与 loads 相同",
          Sml.loads(src, Sml::SML_F_BASIC | Sml::SML_F_WEAK) == obj2)
  ensure
    bt.close
  end

  env_obj, = Sml.loads_full("k: $env.SML_RB_PROBE\n", env: { "SML_RB_PROBE" => "prod" })
  check("11 env 注入 -> k=prod", env_obj == { "k" => "prod" }, env_obj.inspect)
  check("11 env 不改进程环境", ENV["SML_RB_PROBE"].nil?)

  begin
    Sml.loads_full("@feature enable type.weak\n@weakbind tls\ns { x: 1 }\n", flags: 0)
    check("11 基线不含 type.weak -> E-FEATURE-001", false, "未抛异常")
  rescue Sml::SmlError => e
    check("11 基线不含 type.weak -> E-FEATURE-001", e.code_str == "E-FEATURE-001", e.code_str.inspect)
  end

  # 12. 弱声明三表（P7 第四片）：都不进主树，从 loads_full_lists 取出
  wsrc = "@feature enable type.weak\n" \
         "srv {\n  @weakbind tls\n  web { cert: \"x\" }\n" \
         "  @weakdata limits { max: 10 }\n}\n" \
         "@weakdoc base\n"
  wobj, _wdiags, wl = Sml.loads_full_lists(wsrc, flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK)
  check("12 三表计数 1/1/1",
        [wl.binds.length, wl.data.length, wl.docs.length] == [1, 1, 1],
        [wl.binds.length, wl.data.length, wl.docs.length].inspect)
  if wl.binds.any?
    b = wl.binds[0]
    check("12 bind 的 path/name/line",
          b[:path] == "srv.web" && b[:name] == "tls" && b[:line].positive?, b.inspect)
  end
  if wl.data.any?
    d = wl.data[0]
    check("12 weakdata 归属 srv/limits 且载荷可读",
          d[:path] == "srv" && d[:name] == "limits" && d[:value] == { "max" => 10 }, d.inspect)
  end
  check("12 profile 在顶层（path 为空串）",
        wl.docs.any? && wl.docs[0][:path] == "" && wl.docs[0][:name] == "base", wl.docs.inspect)
  check("12 主树无痕迹", !wobj["srv"].key?("limits"), wobj.inspect)
  100.times { Sml.loads_full_lists(wsrc, flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK) }
  check("12 100x loads_full_lists 不崩溃", true)

  # 13. 聚合结果（parse_all）：契约映射 + 契约表捕获 + 多错误
  pa = Sml.parse_all(
    "@contract tls { cert: str }\nsrv { @is tls\n  cert: \"x\"\n}\n",
    flags: Sml::SML_F_BASIC | Sml::SML_F_CONTRACT
  )
  check("13 契约映射 srv -> tls",
        pa.contract_map.map { |c| [c.path, c.name] } == [%w[srv tls]],
        pa.contract_map.inspect)
  check("13 契约表已捕获（1 份）", !pa.contracts.nil? && pa.contracts.size == 1,
        pa.contracts.inspect)
  _, wd, = Sml.loads_full_lists(
    "@feature enable type.weak\n@weakbind nosuch\ns { x: 1 }\n",
    flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK, builtins: pa.contracts
  )
  check("13 闭环：注入捕获的表后报 decl.unknown_contract",
        wd.map(&:code_str) == ["W-CONTRACT-001"], wd.inspect)
  pa.contracts.close

  bad = "a { @is No1\n  x: 1 }\nb { @is No2\n  y: 2 }\n"
  ba = Sml.parse_all(bad, flags: Sml::SML_F_BASIC | Sml::SML_F_CONTRACT)
  check("13 多错误：2 条且 severity=1、带行号",
        ba.errors.map { |d| [d.code_str, d.line, d.severity] } ==
          [["E-CONTRACT-001", 1, 1], ["E-CONTRACT-001", 3, 1]], ba.errors.inspect)
  check("13 多错误：值树是尽力而为的部分结果",
        ba.value == { "a" => { "x" => 1 }, "b" => { "y" => 2 } }, ba.value.inspect)

  begin
    Sml.parse_all(bad, flags: Sml::SML_F_BASIC | Sml::SML_F_CONTRACT, want: Sml::WANT_DIAGS)
    check("13 未请求 ERRORS -> 仍抛 SmlError", false, "未抛异常")
  rescue Sml::SmlError => e
    check("13 未请求 ERRORS -> 仍抛 SmlError", e.code_str == "E-CONTRACT-001", e.code_str.inspect)
  end

  100.times { Sml.parse_all(wsrc, flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK) }
  check("13 100x parse_all 不崩溃", true)

  # 14. 文件入口（load_file_full）：展开 include + 诊断 + 弱表
  require "tmpdir"
  Dir.mktmpdir("smlrb_") do |d|
    File.write(File.join(d, "sub.sml"), "k: 1\n")
    main = File.join(d, "main.sml")
    File.write(main, "@feature enable type.weak\ninclude \"sub.sml\"\n@weakdoc base\n")
    fobj, fdiags, fwl = Sml.load_file_full(
      main, flags: Sml::SML_F_BASIC | Sml::SML_F_WEAK
    )
    check("14 文件入口：include 已展开", fobj == { "k" => 1 }, fobj.inspect)
    check("14 文件入口：弱表带 @weakdoc",
          fwl.docs.map { |x| x[:name] } == ["base"], fwl.docs.inspect)
    check("14 文件入口：干净文件 -> 零诊断", fdiags.empty?, fdiags.inspect)

    begin
      Sml.load_file_full(File.join(d, "nope.sml"))
      check("14 文件不存在 -> E-IO-001", false, "未抛异常")
    rescue Sml::SmlError => e
      check("14 文件不存在 -> E-IO-001", e.code_str == "E-IO-001", e.code_str.inspect)
    end
  end
rescue StandardError => e
  puts "FATAL: #{e.class}: #{e.message}"
  puts e.backtrace.first(5).join("\n")
  $failures += 1
end

puts ""
if $failures.zero?
  puts "ALL OK"
  exit 0
else
  puts "SOME FAILED (#{$failures})"
  exit 1
end
