#!/usr/bin/env bash
# ============================================================================
# publish_crates.sh — 按依赖顺序把 swsml 及其子 crate 发布到 crates.io
# ----------------------------------------------------------------------------
# 为什么需要这个脚本（而不是直接 `cargo publish`）：
#   workspace 里的子 crate 带了**未发布的新 API/新错误码**（sml-contract 的 infer、
#   sml-parse 的 AppliedContract/ContractMap/PathSeg、sml-codes 的新码……），
#   而 crates.io 版本不可覆盖。发布 swsml 时依赖会解析到 crates.io 上的**旧版**
#   子 crate ⇒ 验证编译 E0432。故必须**先把子 crate 升版本号、按依赖序发布**，
#   最后才能发布 swsml。
#
# 顺序（叶子优先，依赖方在后）：
#   1 sml-codes    （零依赖）
#   2 sml-feature  (→ sml-value，已发布未变)
#   3 sml-contract (→ sml-codes[新], sml-pattern, sml-value)
#   4 sml-parse    (→ sml-codes[新], sml-feature[新], sml-contract[新], sml-lex, sml-include, ...)
#   5 swsml        (→ 上述全部 + swsml-derive[已发布未变])
#
# 每个 crate：先 `cargo publish --dry-run` 预检（打包+干净目录验证编译），
# 通过后才真正上传。任一步 dry-run 失败即停（set -e），不会带病上传。
#
# 前置条件：
#   * 已 `cargo login`（crates.io token）。
#   * **git 工作树干净**——cargo publish 默认拒绝脏树。请先 commit 本次发版改动。
#     （--allow-dirty 可跳过，但正式发布不建议。）
#   * 版本号已按 packaging/ 里的说明升好（本脚本不擅自改版本）。
#
# 用法：
#   packaging/publish_crates.sh              # 按序 dry-run + 实际上传
#   packaging/publish_crates.sh --check      # 只 dry-run 不上传（后几个可能因依赖未发布而失败，属正常）
#   packaging/publish_crates.sh --allow-dirty# 跳过 git-clean 检查（调试用）
#
# 注意：smltools 是独立二进制 crate、版本自带（0.2.2），若也改了需在 swsml 之后单独发。
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
    echo "!! git 工作树不干净，cargo publish 会拒绝。请先 commit 本次发版改动；" >&2
    echo "   调试期可用 --allow-dirty 跳过（正式发布不建议）。" >&2
    git status --short | head -20
    exit 1
  fi
fi

CRATES=(sml-codes sml-feature sml-contract sml-parse swsml)

for c in "${CRATES[@]}"; do
  echo "======================================================"
  echo "==> [$c] dry-run 预检"
  echo "======================================================"
  cargo publish --dry-run -p "$c" $ALLOW_DIRTY

  if [ "$CHECK_ONLY" -eq 1 ]; then
    echo "    --check 模式：跳过实际上传 $c"
    continue
  fi

  echo "==> [$c] 上传到 crates.io"
  cargo publish -p "$c" $ALLOW_DIRTY

  # crates.io 索引传播可能有延迟；下一个 crate 的依赖解析要能读到刚发布的版本。
  echo "   等待索引传播…"
  sleep 8
done

echo "======================================================"
echo "全部发布完成：sml-codes / sml-feature / sml-contract / sml-parse / swsml"
echo "后续：git tag v0.6.3 && git push --tags；更新 llms.txt 的 0.6.2/0.6.3 告诫；deploy 官网。"
