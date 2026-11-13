# SPDX-License-Identifier: MulanPSL-2.0
# SML 多语言宿主绑定（Ruby / fiddle）。
# 仓库：https://gitee.com/snoware/sml
# @version 由 sml_version() 提供（不在此写死包版本）

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
  # 12..18 与 c/sml_rs.h 逐位一致（守护：tools/check/check_amberice_align.py）
  SML_F_WHEN          = 1 << 12
  SML_F_FOR           = 1 << 13
  SML_F_TYPED_BLOCK   = 1 << 14
  SML_F_CASE_INSENSITIVE = 1 << 15
  SML_F_BLOCK_NAME    = 1 << 16
  SML_F_FEATAG        = 1 << 17
  SML_F_IDENT         = 1 << 18
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
    @sml_parse_with_diags = Fiddle::Function.new(h["sml_parse_with_diags"], [v, i, v, v], v)
    @sml_diag_list_free = Fiddle::Function.new(h["sml_diag_list_free"], [v], vo)
    @sml_builtin_new = Fiddle::Function.new(h["sml_builtin_new"], [], v)
    @sml_builtin_free = Fiddle::Function.new(h["sml_builtin_free"], [v], vo)
    @sml_builtin_count = Fiddle::Function.new(h["sml_builtin_count"], [v], i)
    @sml_builtin_add_field = Fiddle::Function.new(h["sml_builtin_add_field"], [v, v, v, v, i], i)
    @sml_builtin_set_range = Fiddle::Function.new(h["sml_builtin_set_range"], [v, v, v, Fiddle::TYPE_DOUBLE, Fiddle::TYPE_DOUBLE], i)
    @sml_builtin_add_enum = Fiddle::Function.new(h["sml_builtin_add_enum"], [v, v, v, v], i)
    @sml_parse_full = Fiddle::Function.new(h["sml_parse_full"], [v, v, v, v], v)
    @sml_parse_full_lists = Fiddle::Function.new(h["sml_parse_full_lists"], [v, v, v, v, v], v)
    @sml_weaklists_free = Fiddle::Function.new(h["sml_weaklists_free"], [v], vo)
    @sml_load_file_full = Fiddle::Function.new(h["sml_load_file_full"], [v, v, v, v, v], v)
    @sml_parse_all = Fiddle::Function.new(h["sml_parse_all"], [v, v, i, v], v)
    @sml_result_value = Fiddle::Function.new(h["sml_result_value"], [v], v)
    @sml_result_diags = Fiddle::Function.new(h["sml_result_diags"], [v], v)
    @sml_result_errors = Fiddle::Function.new(h["sml_result_errors"], [v], v)
    @sml_result_weak = Fiddle::Function.new(h["sml_result_weak"], [v], v)
    @sml_result_contract_map = Fiddle::Function.new(h["sml_result_contract_map"], [v], v)
    @sml_result_contracts = Fiddle::Function.new(h["sml_result_contracts"], [v], v)
    @sml_result_free = Fiddle::Function.new(h["sml_result_free"], [v], vo)
    @sml_builtin_clone = Fiddle::Function.new(h["sml_builtin_clone"], [v], v)
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

  # ---- 非致命诊断（P7）----
  # sml_diag：char* code_str(0) / char* text(8) / int line(16) / int severity(20)，共 24 字节。
  # sml_diag_list：sml_diag* items(0) / size_t len(8)，共 16 字节。
  DIAG_SIZE = 24
  DIAG_LIST_SIZE = 16

  Diagnostic = Struct.new(:code_str, :text, :line, :severity, keyword_init: true)

  def self.read_diags(dl)
    items = dl[0, 8].unpack1("Q<")
    len = dl[8, 8].unpack1("Q<")
    return [] if len.zero? || items.zero?

    base = Fiddle::Pointer.new(items)
    (0...len).map do |idx|
      off = idx * DIAG_SIZE
      code_ptr = base[off, 8].unpack1("Q<")
      text_ptr = base[off + 8, 8].unpack1("Q<")
      Diagnostic.new(
        code_str: code_ptr.zero? ? nil : read_cstr(Fiddle::Pointer.new(code_ptr)),
        text: text_ptr.zero? ? "" : read_cstr(Fiddle::Pointer.new(text_ptr)).to_s,
        line: base[off + 16, 4].unpack1("l<"),
        severity: base[off + 20, 4].unpack1("l<")
      )
    end
  end

  # 解析文本，返回 [对象, 诊断列表]。值树与 loads 逐字节相同。
  def self.loads_with_diags(text, flags = 0)
    raise SmlError.new("E-INTERNAL-001", text: "loads_with_diags: text is nil") if text.nil?

    err = new_error
    dl = Fiddle::Pointer.malloc(DIAG_LIST_SIZE)
    DIAG_LIST_SIZE.times { |k| dl[k] = 0 }
    root = @sml_parse_with_diags.call(text, flags, dl, err)
    begin
      raise_from(err) if root.null?

      [materialize(root), read_diags(dl)]
    ensure
      @sml_diag_list_free.call(dl)
      @sml_free.call(root) unless root.null?
    end
  end

  # ---- 内置契约表 + 完整选项（P7 第二片）----
  # sml_opts：size_t struct_size(0) / unsigned flags(8, +4 pad) / char** env(16) /
  #           size_t env_len(24) / sml_builtin* builtins(32)，共 40 字节。
  OPTS_SIZE = 40

  # 弱声明族（opt-in，不在基线里）：flags 是**特性上限**，要校验弱绑定得显式带上。
  SML_F_WEAK = 1 << 19

  # 取一个已绑定的 C 函数（内部用；嵌套类 Builtin 需要拿模块里的函数句柄）。
  def self.fn(name)
    instance_variable_get(:"@#{name}")
  end

  # 内置契约表（`@weakbind` 软校验用）。空表 ⇒ 软校验整段跳过。
  class Builtin
    def initialize
      @p = Sml.fn(:sml_builtin_new).call
      raise "sml_builtin_new 失败" if @p.nil? || @p.null?
    end

    # 接管一个已归本进程所有的 sml_builtin*（如 sml_builtin_clone 的产物）。
    def self.from_ptr(ptr)
      obj = allocate
      obj.instance_variable_set(:@p, ptr)
      obj
    end

    def size
      @p.null? ? 0 : Sml.fn(:sml_builtin_count).call(@p)
    end

    # ty 取 any / str / int / num / bool / enum；false = 参数非法（表不动）。
    def add_field(contract, field, ty, required: false)
      return false if @p.null?

      Sml.fn(:sml_builtin_add_field)
         .call(@p, contract.to_s, field.to_s, ty.to_s, required ? 1 : 0).zero?
    end

    def set_range(contract, field, min_value, max_value)
      return false if @p.null?

      Sml.fn(:sml_builtin_set_range)
        .call(@p, contract.to_s, field.to_s, min_value.to_f, max_value.to_f).zero?
    end

    def add_enum(contract, field, value)
      return false if @p.null?

      Sml.fn(:sml_builtin_add_enum)
        .call(@p, contract.to_s, field.to_s, value.to_s).zero?
    end

    def raw
      @p
    end

    def close
      return if @p.nil? || @p.null?

      Sml.fn(:sml_builtin_free).call(@p)
      @p = nil
    end
  end

  # 弱声明三表（@weakbind / @weakdata / @weakdoc）：都不进主树。
  WeakLists = Struct.new(:binds, :data, :docs, keyword_init: true)

  # sml_weaklists：binds(0) / n_binds(8) / data(16) / n_data(24) / docs(32) / n_docs(40)，共 48 字节。
  WEAKLISTS_SIZE = 48
  WEAKBIND_SIZE = 24   # path(0) / name(8) / line(16, int) + 4 pad
  WEAKDATA_SIZE = 32   # path(0) / name(8) / line(16, int) + 4 pad / value(24)

  def self.build_opts(flags, env, builtins)
    pairs = case env
            when nil then []
            when Hash then env.map { |k, v| "#{k}=#{v}" }
            else env.map(&:to_s)
            end
    keep = [] # 让 C 侧看到的指针活到调用返回
    env_arr = nil
    unless pairs.empty?
      env_arr = Fiddle::Pointer.malloc(pairs.length * 8)
      pairs.each_with_index do |kv, idx|
        p = Fiddle::Pointer[kv] # String -> NUL 结尾指针
        keep << p
        env_arr[idx * 8, 8] = [p.to_i].pack("Q<")
      end
    end

    opts = Fiddle::Pointer.malloc(OPTS_SIZE)
    OPTS_SIZE.times { |k| opts[k] = 0 }
    opts[0, 8] = [OPTS_SIZE].pack("Q<")
    opts[8, 4] = [flags].pack("L<")
    opts[16, 8] = [env_arr ? env_arr.to_i : 0].pack("Q<")
    opts[24, 8] = [pairs.length].pack("Q<")
    opts[32, 8] = [builtins ? builtins.raw.to_i : 0].pack("Q<")
    [opts, keep]
  end

  # 把 C 侧三表拷成 Ruby 对象（拷完即可整体释放）。
  def self.read_weaklists(wl)
    binds_ptr = wl[0, 8].unpack1("Q<")
    n_binds = wl[8, 8].unpack1("Q<")
    data_ptr = wl[16, 8].unpack1("Q<")
    n_data = wl[24, 8].unpack1("Q<")
    docs_ptr = wl[32, 8].unpack1("Q<")
    n_docs = wl[40, 8].unpack1("Q<")

    binds = []
    unless n_binds.zero? || binds_ptr.zero?
      base = Fiddle::Pointer.new(binds_ptr)
      n_binds.times do |i|
        off = i * WEAKBIND_SIZE
        binds << { path: read_cstr(Fiddle::Pointer.new(base[off, 8].unpack1("Q<"))).to_s,
                   name: read_cstr(Fiddle::Pointer.new(base[off + 8, 8].unpack1("Q<"))).to_s,
                   line: base[off + 16, 4].unpack1("l<") }
      end
    end

    data = []
    unless n_data.zero? || data_ptr.zero?
      base = Fiddle::Pointer.new(data_ptr)
      n_data.times do |i|
        off = i * WEAKDATA_SIZE
        vptr = base[off + 24, 8].unpack1("Q<")
        payload = vptr.zero? ? nil : materialize(Fiddle::Pointer.new(vptr))
        data << { path: read_cstr(Fiddle::Pointer.new(base[off, 8].unpack1("Q<"))).to_s,
                  name: read_cstr(Fiddle::Pointer.new(base[off + 8, 8].unpack1("Q<"))).to_s,
                  line: base[off + 16, 4].unpack1("l<"),
                  value: payload }
      end
    end

    docs = []
    unless n_docs.zero? || docs_ptr.zero?
      base = Fiddle::Pointer.new(docs_ptr)
      n_docs.times do |i|
        off = i * WEAKBIND_SIZE
        docs << { path: read_cstr(Fiddle::Pointer.new(base[off, 8].unpack1("Q<"))).to_s,
                  name: read_cstr(Fiddle::Pointer.new(base[off + 8, 8].unpack1("Q<"))).to_s,
                  line: base[off + 16, 4].unpack1("l<") }
      end
    end

    WeakLists.new(binds: binds, data: data, docs: docs)
  end

  # ---- 聚合结果（v3.1）----
  # `sml_parse_all` 的 want 位（与 c/sml_rs.h 的 SML_WANT_* 逐位一致）。
  WANT_DIAGS        = 1 << 0
  WANT_ERRORS       = 1 << 1
  WANT_WEAK         = 1 << 2
  WANT_CONTRACT_MAP = 1 << 3
  WANT_CONTRACTS    = 1 << 4
  WANT_ALL          = 0x1F

  # sml_contract_ref：char* path(0) / char* name(8)，共 16 字节。
  CONTRACT_REF_SIZE = 16
  # sml_contractmap：sml_contract_ref* items(0) / size_t len(8)，共 16 字节。
  CONTRACT_MAP_SIZE = 16

  ContractRef = Struct.new(:path, :name, keyword_init: true)
  ParseAll = Struct.new(:value, :diagnostics, :errors, :weak, :contract_map, :contracts,
                        keyword_init: true)

  def self.read_contract_map(ptr)
    return [] if ptr.nil? || ptr.to_i.zero?

    items = ptr[0, 8].unpack1("Q<")
    len = ptr[8, 8].unpack1("Q<")
    return [] if len.zero? || items.zero?

    base = Fiddle::Pointer.new(items)
    (0...len).map do |idx|
      off = idx * CONTRACT_REF_SIZE
      ContractRef.new(
        path: read_cstr(Fiddle::Pointer.new(base[off, 8].unpack1("Q<"))).to_s,
        name: read_cstr(Fiddle::Pointer.new(base[off + 8, 8].unpack1("Q<"))).to_s
      )
    end
  end

  # 一次解析拿全部产物：值树 + 诊断 + 多错误 + 弱表 + 契约映射 + 契约表。
  # `want` 位选（默认全要）；`contracts` 是**独立克隆**的 Builtin（调用方负责 close）。
  def self.parse_all(text, flags: 0, env: nil, builtins: nil, want: WANT_ALL)
    raise SmlError.new("E-INTERNAL-001", text: "parse_all: text is nil") if text.nil?

    opts, = build_opts(flags, env, builtins)
    err = new_error
    root = @sml_parse_all.call(text, opts, want, err)
    raise_from(err) if root.null?

    begin
      diags = (want & WANT_DIAGS).zero? ? [] : read_diags(@sml_result_diags.call(root))
      errs = (want & WANT_ERRORS).zero? ? [] : read_diags(@sml_result_errors.call(root))
      weak = if (want & WANT_WEAK).zero?
               WeakLists.new(binds: [], data: [], docs: [])
             else
               read_weaklists(@sml_result_weak.call(root))
             end
      cmap = (want & WANT_CONTRACT_MAP).zero? ? [] : read_contract_map(@sml_result_contract_map.call(root))
      # 借用的句柄不能活过结果 ⇒ 立刻克隆成宿主独占的一份。
      ctr = if (want & WANT_CONTRACTS).zero?
              nil
            else
              Builtin.from_ptr(@sml_builtin_clone.call(@sml_result_contracts.call(root)))
            end
      ParseAll.new(value: materialize(@sml_result_value.call(root)),
                   diagnostics: diags, errors: errs, weak: weak,
                   contract_map: cmap, contracts: ctr)
    ensure
      @sml_result_free.call(root)
    end
  end

  # 解析文本，带完整选项（特性上限 / $env 注入 / 内置契约表），返回 [对象, 诊断列表]。
  # flags 是**特性上限**（与 loads 同义）；env 可为 Hash 或 "KEY=VALUE" 数组。
  def self.loads_full(text, flags: 0, env: nil, builtins: nil)
    raise SmlError.new("E-INTERNAL-001", text: "loads_full: text is nil") if text.nil?

    opts, = build_opts(flags, env, builtins)
    err = new_error
    dl = Fiddle::Pointer.malloc(DIAG_LIST_SIZE)
    DIAG_LIST_SIZE.times { |k| dl[k] = 0 }
    root = @sml_parse_full.call(text, opts, dl, err)
    begin
      raise_from(err) if root.null?

      [materialize(root), read_diags(dl)]
    ensure
      @sml_diag_list_free.call(dl)
      @sml_free.call(root) unless root.null?
    end
  end

  # 同 loads_full，另交出弱声明三表（@weakbind / @weakdata / @weakdoc）。
  def self.loads_full_lists(text, flags: 0, env: nil, builtins: nil)
    raise SmlError.new("E-INTERNAL-001", text: "loads_full_lists: text is nil") if text.nil?

    opts, = build_opts(flags, env, builtins)
    err = new_error
    dl = Fiddle::Pointer.malloc(DIAG_LIST_SIZE)
    DIAG_LIST_SIZE.times { |k| dl[k] = 0 }
    wl = Fiddle::Pointer.malloc(WEAKLISTS_SIZE)
    WEAKLISTS_SIZE.times { |k| wl[k] = 0 }
    root = @sml_parse_full_lists.call(text, opts, wl, dl, err)
    begin
      raise_from(err) if root.null?

      [materialize(root), read_diags(dl), read_weaklists(wl)]
    ensure
      @sml_weaklists_free.call(wl)
      @sml_diag_list_free.call(dl)
      @sml_free.call(root) unless root.null?
    end
  end

  # 同 `Sml.load_file`，另交出诊断与弱声明三表（文件入口版的 loads_full_lists）。
  # 跨 include 时行号可能为 0（token 跨文件合并，没有可映射的单一原文）。
  def self.load_file_full(path, flags: 0, env: nil, builtins: nil)
    raise SmlError.new("E-INTERNAL-001", text: "load_file_full: path is nil") if path.nil?

    opts, = build_opts(flags, env, builtins)
    err = new_error
    dl = Fiddle::Pointer.malloc(DIAG_LIST_SIZE)
    DIAG_LIST_SIZE.times { |k| dl[k] = 0 }
    wl = Fiddle::Pointer.malloc(WEAKLISTS_SIZE)
    WEAKLISTS_SIZE.times { |k| wl[k] = 0 }
    root = @sml_load_file_full.call(path.to_s, opts, wl, dl, err)
    begin
      raise_from(err) if root.null?

      [materialize(root), read_diags(dl), read_weaklists(wl)]
    ensure
      @sml_weaklists_free.call(wl)
      @sml_diag_list_free.call(dl)
      @sml_free.call(root) unless root.null?
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
