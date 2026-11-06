# SPDX-License-Identifier: MulanPSL-2.0
# amberice — reader for `.ambr`, the general-purpose binary envelope.
# amberice —— `.ambr`（通用二进制信封）读取器。
# Repository / 仓库：https://gitee.com/snoware/sml

# frozen_string_literal: true

require "fiddle"

# Reader for `.ambr`.
#
# `.ambr` 读取器。
#
# A separate module on purpose: `.ambr` never depended on SML's parser, and `amberice`
# is its own product line. Requiring this file must not pull in `Sml`.
#
# 刻意独立模块：`.ambr` 从不依赖 SML 解析器，`amberice` 是独立产品线；
# require 本文件不应拖进 `Sml`。
#
# Only "bytes in, canonical text out" is promised — the two symbols of `amberice-ffi`.
# Materialise a value tree with Sml.loads(Amberice.load_text(bytes)).
#
# 只承诺「字节进、canonical 文本出」，即 `amberice-ffi` 的两个符号；
# 要值树用 Sml.loads(Amberice.load_text(bytes))。
#
# Library: `amberice_ffi`, not `amberice` (the Rust crate is a pure rlib with no C ABI).
# Build with `cargo build -p amberice-ffi`; at run time set AMBR_LIB to the artifact or
# put its directory on PATH / LD_LIBRARY_PATH.
#
# 库名是 `amberice_ffi`，不是 `amberice`（Rust crate 是纯 rlib，没有 C ABI）。
# 构建 `cargo build -p amberice-ffi`；运行期用 AMBR_LIB 指路径，或把目录加进 PATH。
#
# Messages are not translated here: `swi18n` is not built yet, and a per-binding copy
# would drift. Error text is passed through verbatim from Rust (prefixed "AMBR:").
#
# 文案不在绑定层翻译：`swi18n` 尚未建立，各端自造会漂移；错误串原样透传 Rust。
module Amberice
  ERROR_SIZE = 256

  # Capability bits — same values as `ambr_caps()` in amberice-ffi.
  # 能力位 —— 与 FFI 的 `ambr_caps()` 逐位一致。
  CAP_READ = 1 << 0
  CAP_WRITE = 1 << 1

  # Decoding failed; the message starts with "AMBR:" and comes from Rust.
  # 解码失败；消息以 "AMBR:" 开头，来自 Rust 侧。
  class Error < StandardError; end

  class << self
    # Decode `.ambr` bytes into canonical text (UTF-8).
    # 把 `.ambr` 字节解成 canonical 文本（UTF-8）。
    # Capability bits of the loaded cdylib. Query instead of assuming a symbol exists.
    # 已加载 cdylib 的能力位；用它判断，而不是假定符号存在。
    def capabilities
      fn = lib[:caps]
      fn ? fn.call : CAP_READ
    end

    def load_text(data)
      raise ArgumentError, "data must be a String / data 必须是 String" unless data.is_a?(String)
      raise Error, "AMBR: empty input" if data.empty?

      err = Fiddle::Pointer.malloc(ERROR_SIZE)
      err[0, ERROR_SIZE] = "\0" * ERROR_SIZE
      buf = Fiddle::Pointer[data]

      ptr = lib[:text].call(buf, data.bytesize, err, ERROR_SIZE)
      if ptr.nil? || ptr.null?
        msg = err.to_s.split("\0", 2).first.to_s
        raise Error, msg.empty? ? "AMBR: decode failed (no message)" : msg
      end

      begin
        ptr.to_s.force_encoding(Encoding::UTF_8)
      ensure
        lib[:free].call(ptr)
      end
    end

    # Pack SML source text into `.ambr` bytes. Requires the cdylib built with
    # `--features sml` — the write path needs a parser; the read path does not.
    #
    # 把 SML 源文本打成 `.ambr` 字节。需要带 `--features sml` 构建的 cdylib
    # —— 写路径需要解析器，读路径不需要。
    #
    # Objects keep source order, which is what canonical bytes require.
    # Round-trip: load_text(pack_text(text)) == the canonical form of text.
    #
    # 对象保留源序（canonical 字节的前提）。往返判据：
    # load_text(pack_text(text)) == text 的 canonical 形式。
    def pack_text(text)
      unless (capabilities & CAP_WRITE) != 0
        raise Error,
              "AMBR: this build is read-only — rebuild amberice-ffi with --features sml / " \
              "本次构建只读：请用 --features sml 重新构建 amberice-ffi"
      end

      src = text.to_str.encode(Encoding::UTF_8)
      raise Error, "AMBR: empty input" if src.empty?

      err = Fiddle::Pointer.malloc(ERROR_SIZE)
      err[0, ERROR_SIZE] = "\0" * ERROR_SIZE
      out_len = Fiddle::Pointer.malloc(Fiddle::SIZEOF_SIZE_T)
      out_len[0, Fiddle::SIZEOF_SIZE_T] = "\0" * Fiddle::SIZEOF_SIZE_T

      ptr = lib[:pack_text].call(Fiddle::Pointer[src], src.bytesize, out_len, err, ERROR_SIZE)
      if ptr.nil? || ptr.null?
        msg = err.to_s.split("\0", 2).first.to_s
        raise Error, msg.empty? ? "AMBR: encode failed (no message)" : msg
      end

      n = out_len[0, Fiddle::SIZEOF_SIZE_T].unpack1("J")
      begin
        ptr[0, n]
      ensure
        lib[:free_bytes].call(ptr, n)
      end
    end

    # Decode a `.ambr` file (binary read; the path is not rewritten).
    # 解一个 `.ambr` 文件（二进制读，不改写路径）。
    def load_text_file(path)
      load_text(File.binread(path))
    end

    # Decode from any IO (File, StringIO, Socket, Tempfile). Read to EOF, not closed.
    # 从任意 IO 解码（File、StringIO、Socket、Tempfile）；读到底，不负责关闭。
    def load(io)
      raise ArgumentError, "io must respond to #read / io 必须响应 #read" unless io.respond_to?(:read)

      load_text(io.read)
    end

    private

    def lib
      @lib ||= begin
        name = ENV["AMBR_LIB"]
        candidates = [
          name, "amberice_ffi", "libamberice_ffi",
          "amberice_ffi.dll", "libamberice_ffi.so", "libamberice_ffi.dylib"
        ].compact

        handle = candidates.lazy.map do |c|
          begin
            Fiddle::Handle.new(c)
          rescue LoadError, StandardError
            nil
          end
        end.find { |h| h }
        unless handle
          raise LoadError,
                "amberice_ffi not found — set AMBR_LIB or run `cargo build -p amberice-ffi` / " \
                "找不到 amberice_ffi：用 AMBR_LIB 指定路径，或先构建 amberice-ffi"
        end

        {
          text: Fiddle::Function.new(
            handle["ambr_text"],
            [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T],
            Fiddle::TYPE_VOIDP
          ),
          free: Fiddle::Function.new(
            handle["ambr_free_str"],
            [Fiddle::TYPE_VOIDP],
            Fiddle::TYPE_VOID
          ),
          pack_text: Fiddle::Function.new(
            handle["ambr_pack_text"],
            [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T, Fiddle::TYPE_VOIDP,
             Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T],
            Fiddle::TYPE_VOIDP
          ),
          free_bytes: Fiddle::Function.new(
            handle["ambr_free_bytes"],
            [Fiddle::TYPE_VOIDP, Fiddle::TYPE_SIZE_T],
            Fiddle::TYPE_VOID
          ),
          caps: begin
            Fiddle::Function.new(handle["ambr_caps"], [], Fiddle::TYPE_INT)
          rescue StandardError
            nil
          end
        }
      end
    end
  end
end
