# -*- coding: utf-8 -*-
"""
清理英文部件数据中残留的中文片段（占位符修复后的收尾）。

只处理「明显是中文短语、且非 SML 示例值」的片段：
  - 选项/说明里的中文短语（如 `@contract 校验结构`、`@is 在入口校验`）
  - task / prompt 中夹在英文句子里的中文子句
保留：SML 代码里的中文示例值（如 city: 北京、name: 大厅、中文无需引号）
"""
import os
import re
import sys
import json

sys.stdout.reconfigure(encoding="utf-8")

HERE = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, HERE)
import translate_widget_data as T  # noqa: E402

DATA = T.DATA
CJK = re.compile(r"[\u4e00-\u9fff]")

# SML 代码上下文：含这些符号时视为代码，保留中文值
CODE_CTX = re.compile(r"[:{}\[\]=]")

FILES = ("sml-quizzes", "sml-lessons", "sml-challenges")

cache = {}


def should_translate(val):
    """判断该字符串是否值得翻译（含中文且不是纯代码/SML 值）。"""
    if not isinstance(val, str) or not CJK.search(val):
        return False
    # 纯中文（无 ASCII）且短 -> 是短语，翻译
    # 含代码符号 -> 多半是示例，保留
    return True


def translate_cjk_fragment(val):
    """翻译字符串中夹着的中文短语（保留代码部分）。"""
    if not CJK.search(val):
        return val
    # 整句是中文短语（无代码符号）-> 整句翻
    if not CODE_CTX.search(val):
        tr = T.baidu(val)
        return tr or val
    # 混合：提取连续中文片段分别翻译
    segs = re.findall(r"[^\x00-\x7f][^\x00-\x7f\u3000-\u303f，。；：、！？]*", val)
    out = val
    for seg in segs:
        if len(CJK.findall(seg)) < 2:
            continue
        # 跳过纯标点
        if not re.search(r"[\u4e00-\u9fff]{2,}", seg):
            continue
        if seg in cache:
            tr = cache[seg]
        else:
            tr = T.baidu(seg)
            cache[seg] = tr
        if tr:
            out = out.replace(seg, tr)
    return out


def walk(node, path, report):
    if isinstance(node, dict):
        for k, v in node.items():
            node[k] = walk(v, f"{path}.{k}", report)
        return node
    if isinstance(node, list):
        return [walk(v, f"{path}[{i}]", report) for i, v in enumerate(node)]
    if isinstance(node, str) and CJK.search(node):
        new = translate_cjk_fragment(node)
        if new != node:
            report.append((path, node[:60], new[:60]))
            return new
    return node


def main():
    report = []
    for name in FILES:
        p = os.path.join(DATA, f"{name}_en.json")
        if not os.path.exists(p):
            continue
        print(f"\n=== {name} ===")
        d = json.load(open(p, encoding="utf-8"))
        # 只处理需要翻译的字段，避免误改 main/source 代码
        if name == "sml-quizzes":
            for key, block in d.items():
                if isinstance(block.get("title"), str):
                    block["title"] = walk(block["title"], f"{key}.title", report)
                for i, it in enumerate(block.get("items", [])):
                    for f in ("q", "explain"):
                        if f in it:
                            it[f] = walk(it[f], f"{key}.items[{i}].{f}", report)
                    if "options" in it:
                        it["options"] = walk(it["options"], f"{key}.items[{i}].options", report)
        elif name == "sml-lessons":
            for key, cfg in d.items():
                for f in ("task", "hint"):
                    if f in cfg:
                        cfg[f] = walk(cfg[f], f"{key}.{f}", report)
        elif name == "sml-challenges":
            for key, cfg in d.items():
                for f in ("title", "prompt", "hint"):
                    if f in cfg:
                        cfg[f] = walk(cfg[f], f"{key}.{f}", report)
        with open(p, "w", encoding="utf-8") as f:
            json.dump(d, f, ensure_ascii=False, indent=2)

    print("\n=== 复查 ===")
    for name in FILES:
        p = os.path.join(DATA, f"{name}_en.json")
        t = open(p, encoding="utf-8").read()
        print(f"{name:20} 中文残留={len(CJK.findall(t)):4}")

    print(f"\n共修改 {len(report)} 处")
    for path, old, new in report[:30]:
        print(f"  {path}\n    {old}\n -> {new}")


if __name__ == "__main__":
    main()
