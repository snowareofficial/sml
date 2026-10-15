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
