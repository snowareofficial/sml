# SPDX-License-Identifier: MulanPSL-2.0
# SML 多语言宿主绑定（Ruby / fiddle）。
# 仓库：https://gitee.com/snoware/sml
# @version 0.1.0 (swsml 0.6.3)

# frozen_string_literal: true
# SML 宿主绑定（Ruby / fiddle）—— 经 C-ABI 调 Rust cdylib（sml / libsml）。
#
# 读路径（loads / load_file / 取值）直接遍历 v3 值树，不经 JSON：类型保真、
# 富错误直接到手。仅 dumps 按文档走官方桥 obj -> JSON -> sml_dump。
# ABI 宪法 = c/sml_rs.h（Rust cdylib v3 全集）。不重写 SML 解析器。
#
# 内存纪律（§0.3）：
#   * sml_loads / sml_load_file 返回的根：用完 sml_free。
#   * sml_str_dup / sml_key_at / sml_dumps / sml_dump / sml_version_str 返回的
#     char*：读后 sml_free_str（sml_key_at 的键也是堆副本，必须 free）。
#   * sml_get / sml_at / sml_get_path 返回借用指针：不释放、不跨根释放使用。
#   * sml_version 返回静态串：只读，不释放。

require "fiddle"
require "json"

module Sml
  # ---- 特性位常量（与 c/sml_rs.h 逐位一致） ----
  SML_F_BAREWORD_STR = 1 << 0
  SML_F_INCLUDE       = 1 << 1
  SML_F_ENV           = 1 << 2
  SML_F_CONTRACT      = 1 << 3
  SML_F_FRAGMENT      = 1 << 4
  SML_F_TOP_ARRAY     = 1 << 5
  SML_F_NAMESPACE     = 1 << 6
  SML_F_IMPLICIT_NS   = 1 << 7
  SML_F_MULTI_INCLUDE = 1 << 8
  SML_F_GLOB_INCLUDE  = 1 << 9
  SML_F_REGEX_INCLUDE = 1 << 10
  SML_F_EXT_REWRITE   = 1 << 11
  SML_F_BASIC = SML_F_BAREWORD_STR | SML_F_INCLUDE | SML_F_FRAGMENT

  # ---- 值类型 ----
  T_NULL = 0
  T_BOOL = 1
  T_INT = 2
  T_FLOAT = 3
  T_STR = 4
  T_ARRAY = 5
  T_OBJECT = 6

  # ---- sml_error 布局（逐字节对齐 c/sml_rs.h，小端） ----
  # int code(0) / int line(4) / int column(8) / pad(12) /
  # size_t position(16,8) / char source[128](24) / char text[256](152) /
  # char code_str[16](408)。共 424 字节。
  ERROR_SIZE = 424
  OFF_CODE = 0
  OFF_LINE = 4
  OFF_COLUMN = 8
  OFF_POSITION = 16
  OFF_SOURCE = 24
  OFF_TEXT = 152
  OFF_CODE_STR = 408

  class SmlError < StandardError
    attr_reader :code_str, :code, :line, :column, :position, :source, :text

    def initialize(code_str, code = 0, line = 0, column = 0, position = 0,
                   source = "", text = "")
      @code_str = code_str
      @code = code
      @line = line
      @column = column
      @position = position
      @source = source
      @text = text
      loc = (line.positive? || column.positive?) ? " (line #{line}, column #{column})" : ""
      super("#{code_str}: #{text}#{loc}")
    end
  end

  # ---- 加载库 ----
  def self.lib
    @lib ||= begin
      name = ENV["SML_LIB"]
      candidates = [name, "sml", "libsml", "sml.dll", "libsml.dll",
                    "libsml.so", "libsml.dylib"].compact
      handle = nil
      candidates.each do |c|
        begin
          handle = Fiddle.dlopen(c)
          break
        rescue Fiddle::DLError
          next
        end
      end
      raise "无法加载 SML cdylib（sml / libsml）。请设置 SML_LIB 指向 " \
            "libsml.{dll,so,dylib}，或将其目录加入 PATH。" unless handle

      bind(handle)
      handle
    end
  end

  def self.bind(h)
    v = Fiddle::TYPE_VOIDP
    i = Fiddle::TYPE_INT
    ll = Fiddle::TYPE_LONG_LONG
    vo = Fiddle::TYPE_VOID

    @sml_loads = Fiddle::Function.new(h["sml_loads"], [v, i, v], v)
    @sml_load_file = Fiddle::Function.new(h["sml_load_file"], [v, i, v], v)
    @sml_free = Fiddle::Function.new(h["sml_free"], [v], vo)
    @sml_free_str = Fiddle::Function.new(h["sml_free_str"], [v], vo)
    @sml_typeof = Fiddle::Function.new(h["sml_typeof"], [v], i)
    @sml_get = Fiddle::Function.new(h["sml_get"], [v, v], v)
    @sml_at = Fiddle::Function.new(h["sml_at"], [v, ll], v)
    @sml_size = Fiddle::Function.new(h["sml_size"], [v], ll)
    @sml_key_at = Fiddle::Function.new(h["sml_key_at"], [v, ll], v)
    @sml_str_dup = Fiddle::Function.new(h["sml_str_dup"], [v], v)
    @sml_int_value = Fiddle::Function.new(h["sml_int_value"], [v], ll)
    @sml_real_value = Fiddle::Function.new(h["sml_real_value"], [v], Fiddle::TYPE_DOUBLE)
    @sml_bool_value = Fiddle::Function.new(h["sml_bool_value"], [v], i)
    @sml_dumps = Fiddle::Function.new(h["sml_dumps"], [v, i], v)
    @sml_dump = Fiddle::Function.new(h["sml_dump"], [v], v)
    @sml_dump_err = Fiddle::Function.new(h["sml_dump_err"], [v, v], v)
    @sml_version = Fiddle::Function.new(h["sml_version"], [], v)
    @sml_features_mask = Fiddle::Function.new(h["sml_features_mask"], [], i)
    @sml_feature_name = Fiddle::Function.new(h["sml_feature_name"], [i], v)
  end

  # ---- 字符串读 / 释放 ----
  def self.read_cstr(p)
    return nil if p.nil? || (p.respond_to?(:null?) && p.null?)

    s = p.to_s
    s.force_encoding("UTF-8")
    s
  end

  def self.take(p)
    return nil if p.nil? || (p.respond_to?(:null?) && p.null?)

    s = read_cstr(p)
    @sml_free_str.call(p)
    s
  end

  def self.new_error
    ptr = Fiddle::Pointer.malloc(ERROR_SIZE)
    ERROR_SIZE.times { |k| ptr[k] = 0 }
    ptr
  end

  def self.cstr_at(ptr, off, len)
    bytes = ptr[off, len]
    nul = bytes.bytes.index(0)
    str = nul ? bytes.b[0...nul] : bytes
    str.b.force_encoding("UTF-8").encode("UTF-8")
  rescue StandardError
    ""
  end

  def self.err_fields(ptr)
    code = ptr[OFF_CODE, 4].unpack1("l<")
    line = ptr[OFF_LINE, 4].unpack1("l<")
    column = ptr[OFF_COLUMN, 4].unpack1("l<")
    position = ptr[OFF_POSITION, 8].unpack1("q<")
    source = cstr_at(ptr, OFF_SOURCE, 128)
    text = cstr_at(ptr, OFF_TEXT, 256)
    code_str = cstr_at(ptr, OFF_CODE_STR, 16)
    [code_str, code, line, column, position, source, text]
  end

  def self.raise_from(ptr)
    cs, code, line, column, position, source, text = err_fields(ptr)
    raise SmlError.new(cs, code, line, column, position, source, text)
  end

  def self.materialize(v)
    t = @sml_typeof.call(v)
    return nil if t == T_NULL
    return @sml_bool_value.call(v) != 0 if t == T_BOOL
    return @sml_int_value.call(v) if t == T_INT
    return @sml_real_value.call(v) if t == T_FLOAT
    return take(@sml_str_dup.call(v)) if t == T_STR

    if t == T_ARRAY
      n = @sml_size.call(v)
      (0...n).map { |idx| materialize(@sml_at.call(v, idx)) }
    elsif t == T_OBJECT
      n = @sml_size.call(v)
      h = {}
      n.times do |idx|
        kp = @sml_key_at.call(v, idx)
        key = take(kp)
        next if key.nil?

        h[key] = materialize(@sml_get.call(v, key))
      end
      h
    else
      nil
    end
  end

  def self.loads(text, flags = 0)
    raise SmlError.new("E-INTERNAL-001", text: "loads: text is nil") if text.nil?

    err = new_error
    root = @sml_loads.call(text, flags, err)
    if root.null?
      raise_from(err)
    else
      begin
        materialize(root)
      ensure
        @sml_free.call(root)
      end
    end
  end

  def self.load_file(path, flags = 0)
    raise SmlError.new("E-INTERNAL-001", text: "load_file: path is nil") if path.nil?

    err = new_error
    root = @sml_load_file.call(path.to_s, flags, err)
    if root.null?
      raise_from(err)
    else
      begin
        materialize(root)
      ensure
        @sml_free.call(root)
      end
    end
  end

  def self.dumps(obj, flags = 0)
    js = JSON.generate(obj)
    out = @sml_dump.call(js)
    return take(out) unless out.null?

    err = new_error
    @sml_dump_err.call(js, err)
    raise_from(err)
  end

  def self.version
    read_cstr(@sml_version.call) || ""
  end

  def self.features
    mask = @sml_features_mask.call
    names = []
    b = 0
    while (1 << b) <= mask && b < 64
      n = read_cstr(@sml_feature_name.call(b))
      names << n if n && !n.empty?
      b += 1
    end
    names
  end

  # 在已物化对象上按 'a.b.c' / 'a.0' 路径便利取值（宿主侧，不走 ABI）。
  def self.get_in(obj, path)
    cur = obj
    path.split(".").each do |seg|
      return nil if cur.nil?
      return nil unless cur.is_a?(Hash) || cur.is_a?(Array)

      if cur.is_a?(Hash)
        cur = cur[seg]
      else
        idx = seg.to_i
        return nil unless idx.to_s == seg

        cur = cur[idx]
      end
    end
    cur
  end

  # 模块加载即绑定库（要求 SML_LIB 可定位 libsml）。
  lib
end
