// GET /badge/live/swsml.svg —— 动态徽章（crates.io 多数据合一）
//
// 与仓库里的静态快照 badge/swsml.svg 的区别：这个是**每次请求现拉 API**，
// 版本号/下载量会自己跟上，不用等人重跑脚本。
//
// 为什么不做成静态快照的自动刷新：
//   - Gitee 是权威源、GitHub 是镜像，让 CI 往仓库回写图片会破坏「单向镜像」纪律；
//   - 而官网本来就跑在 Cloudflare Pages 上，加个 Function 最省事，
//     边缘缓存（见 _lib/badge.js 的 cache-control）还能挡掉对 crates.io 的重复请求。
// README 仍然用仓库内快照 —— 那两个平台不会执行我们的函数。

import { badgeSvg, fallbackSvg, fetchJson, human, PALETTE, svgResponse } from "../../_lib/badge.js";

const CRATE = "swsml";

export async function onRequestGet() {
  try {
    const j = await fetchJson(`https://crates.io/api/v1/crates/${CRATE}`);
    const c = j.crate || {};
    const version = c.max_version || c.newest_version || "?";
    const svg = badgeSvg(
      [
        ["crates.io", "v" + version, PALETTE.dark2, PALETTE.fg],
        ["downloads", human(c.downloads), PALETTE.blue, "#ffffff"],
        ["versions", String(c.num_versions || 0), PALETTE.dark2, PALETTE.fg],
      ],
      { iconBg: PALETTE.blue },
    );
    return svgResponse(svg);
  } catch (e) {
    // 拉不到就退化成兜底徽章：宁可显示 unavailable，也不要让官网上出现裂图
    return svgResponse(fallbackSvg("crates.io"));
  }
}
