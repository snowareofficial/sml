# -*- coding: utf-8 -*-
"""
修复 *_en.json 中残留的 XQZ 占位符。

成因：百度机翻会改写占位符（如插入空格 XQZ 0 XQZ），导致精确 replace 失败。
修复：对含残留的条目，用中文源重新翻译，并改用模糊正则还原（允许占位符内部空格）。
同时打印残留的中文片段，便于人工确认。
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
PH_STRICT = re.compile(r"XQZ\d+XQZ")
PH_FUZZY = re.compile(r"XQZ\s*\d+\s*XQZ")
CJK = re.compile(r"[\u4e00-\u9fff]")

cache = {}


def protect(text):
    ph_map = {}
    counter = [0]

    def _sub(m):
        ph = f"XQZ{counter[0]}XQZ"
        counter[0] += 1
        ph_map[ph] = m.group(0)
        return ph

    masked = text
    for pat in T.PROTECT_PATS:
        masked = pat.sub(_sub, masked)
    return masked, ph_map


def translate_fixed(text):
    """翻译 + 模糊还原占位符。"""
    if not text or not CJK.search(text):
        return text
    masked, ph_map = protect(text)
    if not CJK.search(masked):
        return text
    tr = T.baidu(masked)
    if not tr:
        return text
    # 精确还原
    for ph, orig in ph_map.items():
        tr = tr.replace(ph, orig)

    # 模糊还原（处理百度改写过的形态）
    def _f(m):
        n = int(re.search(r"\d+", m.group(0)).group())
        return ph_map.get(f"XQZ{n}XQZ", m.group(0))

    tr = PH_FUZZY.sub(_f, tr)
    return tr


def fix_value(zh_val, en_val, path, report):
    """若英文值含残留占位符，用中文源重翻。"""
    if not isinstance(en_val, str):
        return en_val
    if not PH_STRICT.search(en_val):
        return en_val
    if not isinstance(zh_val, str) or not CJK.search(zh_val):
        return en_val
    print(f"  修复 {path}: {en_val[:60]!r}")
    new = translate_fixed(zh_val)
    report.append((path, en_val[:70], new[:70]))
    return new


def walk(zh, en, prefix, report):
    """递归遍历中英对照结构，修复含残留的叶子节点。"""
    if isinstance(en, dict) and isinstance(zh, dict):
        for k, v in en.items():
            if k in zh:
                en[k] = walk(zh[k], v, f"{prefix}.{k}", report)
    elif isinstance(en, list) and isinstance(zh, list):
        for i, v in enumerate(en):
            if i < len(zh):
                en[i] = walk(zh[i], v, f"{prefix}[{i}]", report)
    else:
        en = fix_value(zh, en, prefix, report)
    return en


def main():
    report = []
    for name in ("sml-quizzes", "sml-lessons", "sml-challenges"):
        zh_p = os.path.join(DATA, f"{name}.json")
        en_p = os.path.join(DATA, f"{name}_en.json")
        if not (os.path.exists(zh_p) and os.path.exists(en_p)):
            continue
        zh = json.load(open(zh_p, encoding="utf-8"))
        en = json.load(open(en_p, encoding="utf-8"))
        print(f"\n=== {name} ===")
        en = walk(zh, en, name, report)
        with open(en_p, "w", encoding="utf-8") as f:
            json.dump(en, f, ensure_ascii=False, indent=2)

    # 复查
    print("\n=== 复查 ===")
    for name in ("sml-quizzes", "sml-lessons", "sml-challenges"):
        p = os.path.join(DATA, f"{name}_en.json")
        if not os.path.exists(p):
            continue
        t = open(p, encoding="utf-8").read()
        ph = len(PH_STRICT.findall(t))
        cjk = CJK.findall(t)
        print(f"{name:20} 占位符残留={ph:3} 中文残留={len(cjk):4}")
        if cjk:
            # 打印含中文的片段
            d = json.load(open(p, encoding="utf-8"))
            ctx = [s for s in re.findall(r'"[^"]*[\u4e00-\u9fff][^"]*"', t)]
            print("   含中文片段:", ctx[:5])

    print(f"\n共修复 {len(report)} 处")


if __name__ == "__main__":
    main()
