// GET /badge/live/gitee.svg —— 动态徽章（Gitee stars + forks 合一）
//
// Gitee 的 v5 API 匿名可读公开仓库，但有频率限制；靠边缘缓存挡住重复请求。
// 若被限流（403/429）就走兜底徽章，不影响页面渲染。

import { badgeSvg, fallbackSvg, fetchJson, PALETTE, svgResponse } from "../../_lib/badge.js";

const REPO = "snoware/sml";

export async function onRequestGet() {
  try {
    const j = await fetchJson(`https://gitee.com/api/v5/repos/${REPO}`);
    const svg = badgeSvg(
      [
        ["Gitee", REPO, PALETTE.dark2, PALETTE.fg],
        ["stars", String(j.stargazers_count ?? 0), PALETTE.green, "#ffffff"],
        ["forks", String(j.forks_count ?? 0), PALETTE.green2, "#ffffff"],
      ],
      { iconBg: PALETTE.green },
    );
    return svgResponse(svg);
  } catch (e) {
    return svgResponse(fallbackSvg("Gitee"));
  }
}
