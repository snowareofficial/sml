# 生成「Seti + SML」文件图标主题
#
# 背景：VSCode 的**文件图标主题不支持 extends**（那是颜色主题的字段），
# 内置的所有图标主题也都没用它。所以想让 .sml 用青色 {*} 且其他文件沿用
# Seti，只能把 Seti 的图标资源真正复制进来，再覆盖 .sml 映射。
#
# Seti 图标来自 seti-ui（MIT），故一并复制 VSCode 的 ThirdPartyNotices.txt。

import json
import os
import shutil
import sys

sys.stdout.reconfigure(encoding="utf-8")

SETI = r"D:\Microsoft VS Code\a44adf7f53\resources\app\extensions\theme-seti"
ICONS = os.path.join(SETI, "icons")
DST = os.path.join("fileicons", "seti")

os.makedirs(DST, exist_ok=True)

# 1) 字体（Seti 的图标是字体字形，非独立图片）
shutil.copy2(os.path.join(ICONS, "seti.woff"), os.path.join(DST, "seti.woff"))
print("copied seti.woff")

# 2) MIT 许可声明（seti-ui）
notice = os.path.join(SETI, "ThirdPartyNotices.txt")
if os.path.isfile(notice):
    shutil.copy2(notice, os.path.join(DST, "THIRD-PARTY-NOTICES.txt"))
    print("copied THIRD-PARTY-NOTICES.txt")

# 3) 注入 SML 图标
theme = json.load(open(os.path.join(ICONS, "vs-seti-icon-theme.json"), encoding="utf-8"))
before_fx = len(theme.get("fileExtensions", {}))
theme.setdefault("iconDefinitions", {})["_sml_file"] = {"iconPath": "../sml-file.svg"}
theme.setdefault("fileExtensions", {})["sml"] = "_sml_file"
theme.setdefault("languageIds", {})["sml"] = "_sml_file"

out = os.path.join(DST, "sml-seti-icon-theme.json")
with open(out, "w", encoding="utf-8", newline="\n") as f:
    json.dump(theme, f, ensure_ascii=False, separators=(",", ":"))
    f.write("\n")

print(f"generated {out}")
print(f"  fileExtensions: {before_fx} -> {len(theme['fileExtensions'])} (注入 sml)")
print(f"  iconDefinitions: {len(theme['iconDefinitions'])}")
print(f"  size: {os.path.getsize(out)} bytes")
