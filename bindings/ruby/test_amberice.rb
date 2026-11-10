# SPDX-License-Identifier: MulanPSL-2.0
# `.ambr` cross-end vectors: hex → Amberice.load_text → byte-for-byte.
# `.ambr` 跨端对拍：hex → load_text → 与 canonical_text 逐字符比较。
#
# Run / 运行：ruby bindings/ruby/test_amberice.rb（需 AMBR_LIB 或 PATH 能找到 amberice_ffi）

# frozen_string_literal: true

$LOAD_PATH.unshift(File.expand_path("lib", __dir__))
require "amberice"
require "json"

path = File.expand_path("../../tests/vectors/amberice-container.v1.json", __dir__)
unless File.exist?(path)
  warn "vector file not found / 找不到向量文件：#{path}"
  exit 2
end

cases = JSON.parse(File.read(path))["cases"]
fails = 0
ok = 0

cases.each do |c|
  bytes = [c["ambr_hex"]].pack("H*")
  begin
    if Amberice.load_text(bytes) == c["canonical_text"]
      ok += 1
    else
      fails += 1
      warn "FAIL  #{c['id']}  text mismatch / 文本不一致"
    end
  rescue StandardError => e
    fails += 1
    warn "FAIL  #{c['id']}  #{e.message}"
  end
end

# Round-trip: pack the canonical text, read it back, expect the same bytes.
# Top-level scalars (`42`, `null`, `"x"`) are rejected by the parser with
# E-PARSE-008 — a known `sml-parse` debt (unfinished §4.2), not a binding bug.
# They are counted separately rather than silently ignored.
# 往返：顶层标量（`42` / `null` / `"x"`）被解析器以 E-PARSE-008 拒绝，这是
# `sml-parse` 的已知欠账（unfinished §4.2），不是绑定 bug；单独计数，不静默吞掉。
rt_ok = 0
rt_scalar = 0
cases.each do |c|
  begin
    packed = Amberice.pack_text(c["canonical_text"])
    if Amberice.load_text(packed) == c["canonical_text"]
      rt_ok += 1
    else
      fails += 1
      warn "FAIL  #{c['id']}  round-trip mismatch / 往返不一致"
    end
  rescue Amberice::Error => e
    if e.message.include?("E-PARSE-008")
      rt_scalar += 1
    else
      fails += 1
      warn "FAIL  #{c['id']}  pack: #{e.message}"
    end
  rescue StandardError => e
    fails += 1
    warn "FAIL  #{c['id']}  pack: #{e.message}"
  end
end

if fails.zero? && cases.length == 22 && rt_ok + rt_scalar == cases.length
  puts "#{ok}/#{cases.length} read, #{rt_ok} round-trip, #{rt_scalar} top-level scalars rejected (E-PARSE-008)"
else
  warn "#{ok}/#{cases.length} read, #{rt_ok} round-trip, #{rt_scalar} scalar, #{fails} failed"
  exit 1
end
