#!/usr/bin/env bash
# 阶段 0：用 Linux `perf` 取真实硬件计数器（缓存命中率 / IPC）。
# Windows 本机没有可靠的 per-process 计数器，别用 Get-Counter 冒充。
#
# 用法（在 Linux 盒子或 CI 的 rust job 里）：
#   tools/perf_counters.sh ./target/release/deps/profile-<hash> --quick
#   tools/perf_counters.sh ./target/release/deps/perf-<hash>
#
# 说明：`-r 3` 重复 3 次取中位数口径；计数器由内核 multiplex，
# 单次数字有抖动，比较看比例不抠 5%。

set -euo pipefail

if [[ $# -lt 1 ]]; then
  echo "用法: $0 <可执行文件> [参数...]" >&2
  exit 2
fi

if ! command -v perf >/dev/null 2>&1; then
  echo "缺少 perf（Linux 上装 linux-tools / perf）" >&2
  exit 2
fi

BIN="$1"
shift

EVENTS="cycles,instructions,cache-references,cache-misses,L1-dcache-load-misses,LLC-load-misses,branch-misses,page-faults"

echo "# perf stat: $BIN $*"
perf stat -r 3 -e "$EVENTS" -- "$BIN" "$@"
