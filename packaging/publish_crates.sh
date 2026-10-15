#!/usr/bin/env bash
# ============================================================================
# publish_crates.sh — 按依赖顺序把 swsml 及其子 crate 发布到 crates.io
# ----------------------------------------------------------------------------
# 为什么需要这个脚本（而不是直接 `cargo publish`）：
#   workspace 里的子 crate 带了**未发布的新 API/新错误码**（sml-contract 的 infer、
#   sml-parse 的 AppliedContract/ContractMap/PathSeg、sml-lex 的 Span/tokenize_spans、
#   sml-codes 的新码与 SmlError.pos……），而 crates.io 版本不可覆盖。发布下游时依赖会
#   解析到 crates.io 上的**旧版**子 crate ⇒ 验证编译 E0432。故必须**先把子 crate 升
#   版本号、按依赖序发布**，最后才能发布 swsml。
#
# 顺序（叶子优先，依赖方在后）：
#   1 sml-codes    （零依赖）
#   2 sml-feature  (→ sml-value，已发布未变)
#   3 sml-contract (→ sml-codes[新], sml-pattern, sml-value)
#   4 sml-lex      (→ sml-codes[新], sml-value, sml-feature[新])
#   5 sml-parse    (→ 上述 + sml-include)
#   6 swsml        (→ 上述全部 + swsml-derive[已发布未变])
#
# 每个 crate：先 `cargo publish --dry-run` 预检（打包+干净目录验证编译），通过才上传。
# **已发布过同版本的 crate 自动跳过**（便于中断续跑 / 重复执行）。
# 任一步 dry-run 失败即停（set -e），不会带病上传。
#
# 前置条件：
#   * 已 `cargo login`（crates.io token）。
#   * **git 工作树干净**——cargo publish 默认拒绝脏树。请先 commit 本次发版改动；
#     调试期可用 --allow-dirty（正式发布不建议）。
#   * 版本号已按需升好（本脚本不擅自改版本）。
#
# 用法：
#   packaging/publish_crates.sh              # 按序 dry-run + 上传（跳过已发布）
#   packaging/publish_crates.sh --check      # 只 dry-run 不上传
#   packaging/publish_crates.sh --allow-dirty# 跳过 git-clean 检查（调试用）
#
# 已知遗留（本次不级联，留作后续补丁）：sml-pattern 的 DoS 加固（5407f57）未随其
# alpha.2 发布；因 sml-contract alpha.4 已发布、级联重发代价大，留待下个补丁版本。
# ============================================================================
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
RUST_DIR="$(cd "$SCRIPT_DIR/.." && pwd)/rust"
cd "$RUST_DIR"

CHECK_ONLY=0
ALLOW_DIRTY=""
while [ $# -gt 0 ]; do
  case "$1" in
    --check)       CHECK_ONLY=1; shift ;;
    --allow-dirty) ALLOW_DIRTY="--allow-dirty"; shift ;;
    -h|--help)     sed -n '2,40p' "$0"; exit 0 ;;
    *) echo "未知参数: $1" >&2; exit 1 ;;
  esac
done

# git-clean 前置检查（cargo publish 的硬性要求）
if [ -z "$ALLOW_DIRTY" ]; then
  if [ -n "$(git status --porcelain 2>/dev/null)" ]; then
    echo "!! git 工作树不干净，cargo publish 会拒绝。请先 commit；调试期用 --allow-dirty。" >&2
    git status --short | head -20
    exit 1
  fi
fi

local_version()    { sed -nE 's/^version[[:space:]]*=[[:space:]]*"([^"]+)".*/\1/p' "$1/Cargo.toml" | head -n1; }
published_version(){ cargo search "$1" --limit 1 2>/dev/null | sed -nE "s/^$1 = \"([^\"]+)\".*/\1/p"; }

# 发布顺序 = 依赖序（叶子优先）
CRATES=(sml-codes sml-feature sml-contract sml-lex sml-parse swsml)

for c in "${CRATES[@]}"; do
  lv="$(local_version "$c")"
  pv="$(published_version "$c")"
  if [ -n "$pv" ] && [ "$lv" = "$pv" ]; then
    echo "==> [$c] $lv 已在 crates.io，跳过"
    continue
  fi

  echo "======================================================"
  echo "==> [$c] dry-run 预检 (本地 $lv / 线上 ${pv:-无})"
  echo "======================================================"
  cargo publish --dry-run -p "$c" $ALLOW_DIRTY

  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "    --check 模式：跳过实际上传 $c"
    continue
  fi

  echo "==> [$c] 上传到 crates.io ($lv)"
  # 容错：若并发/重跑导致"已上传"，不视为失败
  set +e
  out="$(cargo publish -p "$c" $ALLOW_DIRTY 2>&1)"; rc=$?
  set -e
  if [ $rc -ne 0 ]; then
    if echo "$out" | grep -qiE 'already (uploaded|exists)|is already'; then
      echo "   [$c] 已发布过，跳过"
    else
      echo "$out" >&2
      exit $rc
    fi
  fi

  # crates.io 索引传播可能有延迟；下一个 crate 的依赖解析要能读到刚发布的版本。
  echo "   等待索引传播…"
  sleep 8
done

echo "======================================================"
echo "全部发布完成：sml-codes / sml-feature / sml-contract / sml-lex / sml-parse / swsml"
echo "后续：git tag v0.6.3 && git push --tags；更新 llms.txt 的 0.6.2/0.6.3 告诫；deploy 官网。"
