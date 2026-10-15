#!/usr/bin/env bash
# ============================================================================
# build_deb.sh — 把 swsml 的 Rust cdylib + C/C++ 头文件打成 Debian 包
# ----------------------------------------------------------------------------
# 一条命令出齐四类包（按后端拆分，头文件不混装）：
#   libswsml0        运行时：Rust cdylib 后端（libswsml.so.0）
#   libswsml-dev     C 开发：sml_rs.h + libswsml.so + pkg-config（只装 rs 头，不装 sml.h）
#   libswsml-cpp0    C++ 运行时：libswsml_cpp.so.0（封装 cdylib 的 RAII 层）
#   libswsml-cpp-dev C++ 开发：sml_rs.hpp + libswsml_cpp.so + pkg-config
#
# 命名约定（避开 Debian 官方 libSML / libsml-dev 智能电表生态冲突）：
#   * 所有包名/库名一律 libswsml 系，与 crate 包名 swsml 对齐。
#   * 运行时包名尾数字 = SONAME 主版本（SONAME_MAJOR 变量，ABI 换代只改这一处）。
#   * dev 包不带数字（Debian 惯例：libssl3 / libssl-dev）。
#
# pkg-config 模块名选择（任务要求「选定一种并写明」）：
#   * C   ABI → 模块名 `swsml`    （文件 swsml.pc，链接名 -lswsml）
#   * C++ 封装 → 模块名 `swsml_cpp`（文件 swsml_cpp.pc，链接名 -lswsml_cpp）
#   刻意不用 `sml`：那正是官方 SML 生态的占用名，用 swsml 系彻底避开冲突。
#
# 依赖：Linux（dpkg-deb / dpkg / g++ / objdump / ldconfig / pkg-config）。
#   Windows 下请在 WSL / 容器内运行；脚本不绑死发行版。
# 版本号唯一来源：rust/Cargo.toml（脚本读取，不硬编码）。
#
# 用法：
#   packaging/build_deb.sh                 # 架构取 dpkg --print-architecture
#   packaging/build_deb.sh --arch arm64    # 指定架构
# 产物：packaging/dist/*.deb（连同中间产物一起，已被 .gitignore 忽略）。
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
RUST_DIR="$REPO_ROOT/rust"

# ---- 架构参数（--arch=X 或位置参数；默认取本机 dpkg 架构）----
ARCH=""
while [ $# -gt 0 ]; do
  case "$1" in
    --arch=*) ARCH="${1#*=}"; shift ;;
    --arch)   ARCH="$2"; shift 2 ;;
    -h|--help) sed -n '2,40p' "$0"; exit 0 ;;
    *)        ARCH="$1"; shift ;;
  esac
done
[ -n "$ARCH" ] || ARCH="$(dpkg --print-architecture 2>/dev/null || echo amd64)"

# ---- 版本号：唯一来源 rust/Cargo.toml（不硬编码）----
VERSION="$(sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$RUST_DIR/Cargo.toml" | head -n1)"
if [ -z "$VERSION" ]; then echo "无法从 $RUST_DIR/Cargo.toml 读取 version" >&2; exit 1; fi

# ---- SONAME 主版本：ABI 换代只改这一处（0 ⇒ libswsml0；将来 1 ⇒ libswsml1 共存）----
SONAME_MAJOR=0
LIB_SONAME="libswsml.so.$SONAME_MAJOR"
LIB_REAL="libswsml.so.$VERSION"
CPP_SONAME="libswsml_cpp.so.$SONAME_MAJOR"
CPP_REAL="libswsml_cpp.so.$VERSION"

# ---- 包名 ----
PKG_RT="libswsml${SONAME_MAJOR}"        # libswsml0
PKG_DEV="libswsml-dev"                  # 不带数字
PKG_CPPRT="libswsml-cpp${SONAME_MAJOR}" # libswsml-cpp0
PKG_CPPDEV="libswsml-cpp-dev"

# ---- 多架构 triplet（库装 /usr/lib/<triplet>）----
MULTIARCH="$(gcc -dumpmachine 2>/dev/null || echo x86_64-linux-gnu)"
LIBDIR="usr/lib/$MULTIARCH"

DIST="$SCRIPT_DIR/dist"
WORK="$DIST/.build"
# 暂存目录：默认随 WORK；可经 SML_DEB_STAGE_ROOT 指到原生 FS。
# 原因：WSL 的 /mnt/c（DrvFS）上 chmod 失效、目录恒 0777，而 dpkg-deb 严格要求
# DEBIAN 控制目录 0755 —— 此时必须把暂存放到本地盘（真实 Linux 上默认即正确）。
STAGE_ROOT="${SML_DEB_STAGE_ROOT:-$WORK}"
rm -rf "$WORK"; mkdir -p "$WORK"
rm -rf "$STAGE_ROOT"; mkdir -p "$STAGE_ROOT"

MAINT="SNOWARE <dev@mail.swebase.cn>"
HOMEPAGE="https://sml.swebase.cn"

log(){ echo "[build_deb] $*"; }

# ============================================================================
# 1) 构建 Rust cdylib 并注入 SONAME（Rust 默认不设 SONAME）
#    不改 [lib] name（否则破坏 use sml::{...} 与 rlib 名）——改名只在打包层。
# ============================================================================
log "构建 cdylib (v$VERSION, soname=$LIB_SONAME, arch=$ARCH)"
# 默认把 target 放 packaging/dist/.build/target（随产物一起被 gitignore）；
# 允许外部覆盖（如 WSL/容器里指到本地盘，避免 DrvFS 慢）。
: "${CARGO_TARGET_DIR:=$WORK/target}"
export CARGO_TARGET_DIR
export RUSTFLAGS="-C link-arg=-Wl,-soname,$LIB_SONAME"
( cd "$RUST_DIR" && cargo build --release )
CARGO_SO="$CARGO_TARGET_DIR/release/libsml.so"
[ -f "$CARGO_SO" ] || { echo "cdylib 产物缺失: $CARGO_SO" >&2; exit 1; }
cp -f "$CARGO_SO" "$WORK/$LIB_REAL"
# 供 g++ -lswsml 链接用的符号链（构建期用，安装期由 dev 包提供 libswsml.so）
ln -sf "$LIB_REAL" "$WORK/$LIB_SONAME"
ln -sf "$LIB_SONAME" "$WORK/libswsml.so"
GOT_SONAME="$(objdump -p "$WORK/$LIB_REAL" | awk '/SONAME/{print $NF}')"
[ "$GOT_SONAME" = "$LIB_SONAME" ] || { echo "DT_SONAME 校验失败: got=$GOT_SONAME want=$LIB_SONAME" >&2; exit 1; }
log "cdylib DT_SONAME = $GOT_SONAME (OK)"

# ============================================================================
# 2) 构建 C++ 封装共享库（链 libswsml，注入自身 SONAME）
#    sml_rs.cpp 经 #include "../c/sml_rs.h" 回退找到 rs 头（安装布局改走 <sml/sml_rs.h>）。
# ============================================================================
log "构建 C++ 封装 $CPP_SONAME"
g++ -std=c++17 -O2 -fPIC -shared \
    -I"$REPO_ROOT/cpp" -I"$REPO_ROOT/c" \
    "$REPO_ROOT/cpp/sml_rs.cpp" \
    -L"$WORK" -lswsml \
    -Wl,-soname,"$CPP_SONAME" \
    -o "$WORK/$CPP_REAL"
ln -sf "$CPP_REAL" "$WORK/$CPP_SONAME"
ln -sf "$CPP_SONAME" "$WORK/libswsml_cpp.so"
GOT_CPP_SONAME="$(objdump -p "$WORK/$CPP_REAL" | awk '/SONAME/{print $NF}')"
[ "$GOT_CPP_SONAME" = "$CPP_SONAME" ] || { echo "cpp DT_SONAME 校验失败: got=$GOT_CPP_SONAME" >&2; exit 1; }
log "cpp DT_SONAME = $GOT_CPP_SONAME (OK)"

# ============================================================================
# 辅助：ldconfig 维护脚本
# ============================================================================
write_ldconfig_scripts(){
  local d="$1/DEBIAN"
  cat > "$d/postinst" <<'EOF'
#!/bin/sh
# 注册新装的共享库
set -e
if [ "$1" = "configure" ] || [ "$1" = "abort-upgrade" ]; then
    ldconfig
fi
exit 0
EOF
  cat > "$d/postrm" <<'EOF'
#!/bin/sh
# 移除后刷新 ldconfig 缓存
set -e
if [ "$1" = "remove" ] || [ "$1" = "purge" ]; then
    ldconfig
fi
exit 0
EOF
  chmod 0755 "$d/postinst" "$d/postrm"
}

# 辅助：写 control
# $1=stage  $2=Package  $3=Depends  $4=Description(单行)  $5=Extended desc
write_control(){
  local d="$1/DEBIAN" pkg="$2" depends="$3" short="$4" longd="$5"
  mkdir -p "$d"
  {
    echo "Package: $pkg"
    echo "Version: $VERSION"
    echo "Architecture: $ARCH"
    echo "Maintainer: $MAINT"
    echo "Homepage: $HOMEPAGE"
    echo "Section: libs"
    echo "Priority: optional"
    [ -n "$depends" ] && echo "Depends: $depends"
    echo "Multi-Arch: same"
    echo "Description: $short"
    echo " $longd"
  } > "$d/control"
}

build_deb(){
  local stage="$1" pkg="$2"
  local out="$DIST/${pkg}_${VERSION}_${ARCH}.deb"
  chmod -R go-w "$stage"                     # dpkg 要求组/其他无可写位
  find "$stage/DEBIAN" -type f \( -name 'post*' -o -name 'pre*' \) -exec chmod 0755 {} +
  dpkg-deb --root-owner-group --build "$stage" "$out" >/dev/null
  log "生成 $out"
}

# ============================================================================
# 3) libswsml0 —— 运行时（Rust cdylib 后端）
# ============================================================================
S="$STAGE_ROOT/stage-$PKG_RT"; rm -rf "$S"
mkdir -p "$S/$LIBDIR"
install -m 0644 "$WORK/$LIB_REAL" "$S/$LIBDIR/$LIB_REAL"
ln -sf "$LIB_REAL" "$S/$LIBDIR/$LIB_SONAME"     # SONAME 符号链（链接/加载用）
write_control "$S" "$PKG_RT" "libc6, libgcc-s1" \
  "SML runtime library (Rust cdylib backend)" \
  "Shared library exposing the SML C-ABI (sml_* symbols) backed by the Rust crate swsml. Provides the full v1..v4 feature set. SONAME $LIB_SONAME."
write_ldconfig_scripts "$S"
build_deb "$S" "$PKG_RT"

# ============================================================================
# 4) libswsml-dev —— C 开发（只装 sml_rs.h，不装 sml.h / sml_codes.h）
#    那两个属于纯 C 后端（将来的 libswsml-c-dev），混装会诱导同时 include 两个不可混用后端。
# ============================================================================
S="$STAGE_ROOT/stage-$PKG_DEV"; rm -rf "$S"
mkdir -p "$S/usr/include/sml" "$S/$LIBDIR/pkgconfig"
install -m 0644 "$REPO_ROOT/c/sml_rs.h" "$S/usr/include/sml/sml_rs.h"
ln -sf "$LIB_SONAME" "$S/$LIBDIR/libswsml.so"   # 开发期链接符号链
cat > "$S/$LIBDIR/pkgconfig/swsml.pc" <<EOF
prefix=/usr
libdir=\${prefix}/lib/$MULTIARCH
includedir=\${prefix}/include

Name: swsml
Description: SML (SNOWARE Markup Language) C-ABI, Rust cdylib backend
Version: $VERSION
Cflags: -I\${includedir}
Libs: -L\${libdir} -lswsml
EOF
write_control "$S" "$PKG_DEV" "$PKG_RT (= $VERSION)" \
  "SML C-ABI development files (Rust cdylib backend)" \
  "Headers and pkg-config for linking against libswsml (the Rust cdylib C-ABI). Installs only sml_rs.h; the pure-C backend header sml.h is a separate package."
build_deb "$S" "$PKG_DEV"

# ============================================================================
# 5) libswsml-cpp0 —— C++ 运行时（封装层）
# ============================================================================
S="$STAGE_ROOT/stage-$PKG_CPPRT"; rm -rf "$S"
mkdir -p "$S/$LIBDIR"
install -m 0644 "$WORK/$CPP_REAL" "$S/$LIBDIR/$CPP_REAL"
ln -sf "$CPP_REAL" "$S/$LIBDIR/$CPP_SONAME"
write_control "$S" "$PKG_CPPRT" "$PKG_RT (= $VERSION), libc6, libgcc-s1, libstdc++6" \
  "SML C++ wrapper runtime (RAII Value/Ref over the C-ABI)" \
  "C++17 shared library wrapping the SML C-ABI (libswsml) with RAII Value / non-owning Ref. SONAME $CPP_SONAME."
write_ldconfig_scripts "$S"
build_deb "$S" "$PKG_CPPRT"

# ============================================================================
# 6) libswsml-cpp-dev —— C++ 开发（sml_rs.hpp + libswsml_cpp.so + pkg-config）
# ============================================================================
S="$STAGE_ROOT/stage-$PKG_CPPDEV"; rm -rf "$S"
mkdir -p "$S/usr/include/sml" "$S/$LIBDIR/pkgconfig"
install -m 0644 "$REPO_ROOT/cpp/sml_rs.hpp" "$S/usr/include/sml/sml_rs.hpp"
ln -sf "$CPP_SONAME" "$S/$LIBDIR/libswsml_cpp.so"   # 开发期链接符号链
cat > "$S/$LIBDIR/pkgconfig/swsml_cpp.pc" <<EOF
prefix=/usr
libdir=\${prefix}/lib/$MULTIARCH
includedir=\${prefix}/include

Name: swsml_cpp
Description: SML C++ RAII wrapper over the Rust cdylib C-ABI
Version: $VERSION
Requires: swsml
Cflags: -I\${includedir}
Libs: -L\${libdir} -lswsml_cpp
EOF
write_control "$S" "$PKG_CPPDEV" "$PKG_CPPRT (= $VERSION), $PKG_DEV (= $VERSION)" \
  "SML C++ RAII wrapper development files" \
  "Header and pkg-config for the C++ wrapper (libswsml_cpp). Installs sml_rs.hpp; compile your own translation unit, link with -lswsml_cpp."
build_deb "$S" "$PKG_CPPDEV"

log "全部完成。产物在 $DIST："
ls -1 "$DIST"/*.deb
