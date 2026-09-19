# TASK-hy3-w19：编辑期能力**手工验收**（VSIX / 图标主题 / 字段悬浮 / Zed 同步）

> **这份文档是给「不需要判断力」的执行者写的。** 每一条都给了：**敲什么命令**、**点什么按钮**、
> **期望看到什么**、**不符怎么办**。**不要凭猜改动任何文件** —— 本轮的功能已经写完并提交，
> 这一份是**验收单**，不是开发单。
>
> 目标一句话：**确认「装完 VSIX 后，编辑器里那 6 件事真的按文档工作；并且没有把用户的图标弄没」。**
>
> 上一份是 `TASK-hy3.md`（W16 收口）与 `TASK-hy3-w8-w9.md`（Zed + CI 门禁）。

---

## 0. 铁律（违反就是失败，不需要讨论）

1. **只读不写**：本任务**不改代码**。发现不符 ⇒ 停下、贴证据（命令 + 完整输出）问人。
2. **不许 `git add -A` / `git add .`**：真要提交证据文件，只 `git add <你明确创建的那个>`。
3. **不许 `git push`**：推送由用户本人做。
4. **临时探针文件一律以 `_` 开头**（`.gitignore` 的 `**/_*` 会挡住它，不污染仓库）。
5. **不许动**：`Desktop\sml_secret\`（私有资产库）、`.codebuddy/`、`.gitignore` 里那段安全护栏。
6. **一次只做一条**，做完一条贴证据，再进下一条。
7. **命令别用管道**：`xxx | findstr ...` 会把退出码吃掉（下面 W19.5 有现场复现），
   要过滤就 `> "%TEMP%\out.txt" 2>&1` 落盘再读。
8. Windows + PowerShell 在本机偶发 **AMSI 崩溃**（`AccessViolationException`，输出乱码后中断）：
   这不是你的错，**别把多行 python 写进 `-c`**（`\n` 会被吃掉），改成写 `%TEMP%\xxx.py` 再跑。

---

## 1. 开工前检查（三条，必须全绿）

```bash
cd C:\Users\sakeen\Desktop\sml
git status --short          # 期望：**空**（干净）。不空 ⇒ 停下问人
git --no-pager log --oneline -3
```

期望看到最近三条里包含（顺序可能不同，**subject 对得上就行**）：

```
75e3a7f feat(vscode)+docs: 字段级悬浮/跳转（契约字段 ↔ 数据键）+ 教科书补「字段说明与 i18n」
451e5b5 feat(vscode)+docs: 块名悬浮 / 嵌套契约实例 / 应用特殊颜色；教科书·首页·llms·AI 推荐语全面更新
a7943b9 fix(vscode): 致命 —— 顶层读了 VS Code 1.138 已移除的 vscode.InsertTextFormat，扩展从未激活过
```

再跑三关闸门（**逐条跑，别串成一条**），期望最后一行都是 `ALL PASS`：

```bash
cd editors\vscode
node scripts\_prepublish.mjs        # 期望：6 个 ok + "PREPUBLISH ALL PASS —— 允许打包"
node scripts\_verify_ext.mjs        # 期望："EXT VERIFY ALL PASS"（约 60 条 ✓）
node scripts\_verify_activate.mjs   # 期望："ACTIVATE SMOKE ALL PASS"
```

**不符怎么办**：把**整条命令 + 完整输出**贴出来问人。**不要**自己去改脚本让它过。

---

## 2. W19.0 现状（已实测，别重复劳动）

| 项 | 实测值（2026-09-19） |
|---|---|
| VSIX | `editors/vscode/sml-lang-0.4.2.vsix`，**20 项 / 约 149.8 KB** |
| 站点托管副本 | `site/static/dl/sml-lang-0.4.2.vsix`（与上面同哈希） |
| 版本号 | **保持 0.4.2**（未上架市场；升版本要同步 4 处，漏一处就制造漂移） |
| 包内解析器指纹 | `src/vendor/sml.mjs` = **69404 B**，sha256 前缀 **70f1ee47** |
| 悬浮覆盖 | 契约名 · 块名 · **字段（声明处 + 数据区）** · 关键字 |
| 图标主题 | `sml-icons-seti`（SML + Seti，**保留其他文件图标**）/ `sml-icons`（仅 .sml，⚠️ 别自动切它） |

---

## 3. W19.1 装 VSIX 并核对「包是不是新的」

```bash
code --install-extension C:\Users\sakeen\Desktop\sml\editors\vscode\sml-lang-0.4.2.vsix --force
```
期望：VS Code 右下角提示 `Extension 'sml-lang-0.4.2.vsix' was successfully installed.`

然后 `Ctrl+Shift+P` → `Developer: Reload Window`（**必须重载**，否则跑的还是旧代码）。

**核对包内文件**（这是本仓库的铁律：判据落在「**包内 vs 工作区**」，不是「同步脚本跑过」）：

```bash
cd C:\Users\sakeen\Desktop\sml
python "%TEMP%\check_installed.py"
```
若该脚本不在（它是临时件，不入库），用这条替代：

```bash
python - <<'PY'   # PowerShell 下请写成文件再跑，见铁律 8
import hashlib, os
for rel in ["src/extension.js", "src/sml-parse.mjs", "src/vendor/sml.mjs"]:
    a = os.path.join(os.path.expanduser(r"~\.vscode\extensions\snoware.sml-lang-0.4.2"), rel)
    b = os.path.join(r"C:\Users\sakeen\Desktop\sml\editors\vscode", rel)
    ha = hashlib.sha256(open(a,"rb").read()).hexdigest()[:16]
    hb = hashlib.sha256(open(b,"rb").read()).hexdigest()[:16]
    print(("一致 ✓ " if ha==hb else "**不一致** ✗ "), rel, os.path.getsize(a), ha)
PY
```
期望：三行都是 `一致 ✓`，且 `src/vendor/sml.mjs` 是 **69404**、前缀 `70f1ee47`。

**不符怎么办**：先确认装的是不是 `editors/vscode/sml-lang-0.4.2.vsix`（别装成 `site/static/dl/` 里的旧副本）；
仍不一致 ⇒ 停下贴证据。

---

## 4. W19.2 用「SML: 自检」确认扩展真的激活了（**第一步永远先做这个**）

打开任意 `.sml`（例如 `showcase_contract.sml`），确保右下角语言模式显示 **SML**
（不是 Plain Text —— 不是就是 W19.2 的失败，见下）。

然后 **右键 → `SML: 自检`**（或 `Ctrl+Shift+P` 搜 `SML: 自检`），看「输出 → SML」面板。期望**至少**有：

```
扩展激活（<当前时间>）—— 有「没反应」的地方，执行命令 `SML: 自检`
解析器加载成功 ✓（补全 / 悬浮 / 跳转 / 诊断可用）
SML 语言已注册：✓
桥接层 sml-parse.mjs：已加载 ✓
包内解析器 vendor/sml.mjs：69404 B / sha256 70f1ee476ab684fc
命令注册：sml.specialHighlight ✓  sml.clearSpecialHighlight ✓  sml.selfCheck ✓
语言模式：languageId = sml ✓
```

**不符怎么办（照这个顺序判断）**：

| 面板里看到 | 含义 | 怎么办 |
|---|---|---|
| 完全没有该面板 / 一行都没有 | 扩展**没被加载**（被禁用 / 受限模式 / 没装） | 扩展面板里确认 `snoware.sml-lang` 已启用；再看是否「限制模式」 |
| `桥接层 … 未加载 ✗` | 解析器文件缺失 | 重装 VSIX；贴证据问人 |
| `SML 语言已注册：✗` | 扩展没被加载 | 同上（此时语言模式也改不回 SML） |
| `语言模式：languageId = plaintext` | 文件没被当成 SML | 点面板外那条警告的「设为 SML」；仍不行贴证据 |

---

## 5. W19.3 逐项手工验收（6 件事，一件一条，做完贴证据）

> 统一用 `showcase_contract.sml`（它同时含：契约声明、嵌套块、行内块、片段引用、`&base`）。

### 5.1 悬浮：契约名 —— 期望两段
把光标停在**第 34 行** `@contract Server` 的 `Server` 上。期望悬浮里有两段：
`**契约 \`Server\`**（\`@contract\` 声明）` + `**填入默认值后的结构 —— 来自块 \`database.primary\`（第 52 行）**`。
**只看到第一段** ⇒ 看「输出 → SML」里那句 `悬停「Server」：只显示契约声明（取不到实例）` 后面写的**原因**，
把它贴出来。

### 5.2 悬浮：块名 —— 期望含路径
光标停在**第 52 行**的 `primary`。期望第一行是
`**块 \`primary\`**　（路径 \`database.primary\`）`，并有「应用契约 **\`Server\`**」与填充后的结构
（应含 `port: 5432` 与 `tls: false` —— 这两个是**契约填的**）。
**期望与下面不符就贴证据**（这一条曾整条失效：`primary` 嵌在 `database` 里，旧代码只认顶层块）。

### 5.3 悬浮：字段（两种位置都要试）
- **声明处**：光标停在**第 36 行**的 `port`（`port: int default 5432  # 缺失时填 5432`）。
  期望：`类型 \`int\`　·　默认 \`5432\`　·　可选` + 一行 `> 缺失时填 5432`。
- **数据区**：光标停在**第 64 行**的 `port: 5433`。期望在上面基础上多一行
  `当前值：\`5433\`（块里显式写的）`。
- 另试 **第 39 行** `status`：期望含 `枚举 \`active\` / \`standby\` / \`retired\``。

### 5.4 跳转（F12 或 Ctrl+点击）
| 起点 | 期望跳到 |
|---|---|
| 第 53 行 `@is Server` 的 `Server` | 第 34 行 `@contract Server` |
| `&base`（`showcase_contract.sml` 第 118 行附近） | `@base {` 定义行 |
| 第 64 行 `port`（数据区的键） | 第 36 行**契约里的 `port` 字段** |

### 5.5 特殊颜色（会改工作区文件，**做完记得删掉那一段**）
在 `showcase_contract.sml` 里选中 `Server` → 右键 **`SML: 应用特殊颜色`** → 选一个颜色。
期望：① 提示「已写入 HL-cfg.sml…」；② 工作区根出现/追加 `HL-cfg.sml`，含
`unit: contract` 与 `color: "#…"`；③ 颜色**只**染 `@contract Server` / `@is Server` 这类语法位置，
**不**染注释里同名的 `Server`。
**验收完请删掉 `HL-cfg.sml`**（除非你想留着），删掉后 `SML: 重载自定义高亮配置` 一次。

### 5.6 特别高亮 + 文件图标主题
- 选中一个词 → 右键 **`SML: 特别高亮选中词（当前工作区）`**：状态栏出现 `N 处 / M 文件`；
  点状态栏可清除。
- `Ctrl+Shift+P` → **`SML: 文件图标主题`**：期望三个选项（SML+Seti / 仅 .sml / 打开 VS Code 选择器）。
  ⚠️ **本条的回归点**：选「SML Icons + Seti」后，**非 SML 文件（.md/.json/.py）必须仍有图标**。
  若你发现自己正在用「SML Icons（仅 .sml）」而其他文件没图标 ⇒ 那正是本轮修掉的 bug 现场，
  用这条命令切回 `SML Icons + Seti`，并把现象贴出来（说明「修复提示」可能没弹出来）。

---

## 6. W19.4 Zed 侧核对（不装 Zed 也能做）

```bash
cd C:\Users\sakeen\Desktop\sml\editors\zed\grammars\sml
tree-sitter --version          # 期望 0.22.6（README 里钉的版本）
tree-sitter generate           # 期望：无输出（成功）
tree-sitter parse test\parse\basic.sml test\parse\advanced.sml
```
期望：`parse` 输出里 **没有 `ERROR` / `MISSING`**（样例树会很长，用 `findstr /C:"ERROR" /C:"MISSING"` 过滤看有没有命中为空）。

**没装 tree-sitter CLI**：跳过本节的命令，只在报告里写「CLI 未安装，未验证」——
**不要**去 `npm i -g tree-sitter-cli@latest`（README 钉的是 0.22.6，装成新版会产生假差异）。

**注意**：本轮的 VS Code 功能扩展**不影响** Zed 侧任何文件（grammar / highlights.scm /
`extension.toml` 的 version 与 grammar rev 都没动）。所以本节只验证「没被误伤」。

---

## 7. W19.5 两条历史坑的现场复核（可选，但很有价值）

**① 闸门用管道会丢退出码**（曾导致「闸门报 FAIL，打包照样执行」）：

```bash
cd C:\Users\sakeen\Desktop\sml\editors\vscode
node scripts\_prepublish.mjs | findstr /C:"PASS"      # ← 这一条的退出码恒为 0（findstr 的）
echo %errorlevel%
node scripts\_prepublish.mjs > "%TEMP%\pp.txt" 2>&1    # ← 正确姿势
echo %errorlevel%                                       # ← 这个才反映闸门
```
期望：第二条 `%errorlevel%` 为 **0**（全绿时）；若闸门真有失败，**只有**重定向那条会给非 0。

**② CRLF 下 `.` 不匹配 `\r`**（曾让契约字段一个都解析不出来）：

```bash
cd C:\Users\sakeen\Desktop\sml\editors\vscode
node scripts\_verify_ext.mjs > "%TEMP%\ve.txt" 2>&1
findstr /C:"CRLF 下契约字段" "%TEMP%\ve.txt"
```
期望：`✓ CRLF 下契约字段全部解析出来  解析出 7 个`。

---

## 8. W19.6 文档同步检查（改功能时**必须**同步的 5 处）

本轮已同步，**你的任务是核对它们彼此一致**（不用改）：

| 文件 | 该写什么 |
|---|---|
| `editors/vscode/README.md`（中英双语） | 功能表：悬浮（契约/块/字段）、跳转、特殊颜色、特别高亮、自检、图标主题 |
| `editors/vscode/README.en.md` | 同上（英文） |
| `site/content/zh/downloads.md` 与 `en/downloads.md` | 能力清单 + 「0.4.2 是重新打包」提醒 + 装完怎么核对指纹 |
| `site/content/zh/_index.md` 与 `en/_index.md` | 编辑器支持表（**别再写「走 LSP」** —— 本扩展不启 LSP） |
| `site/content/{zh,en}/book/ch12-smltools.md` §12.9 + `ch05-contract.md` §5.1.1 | 编辑器能力表 + 字段说明与 i18n |
| `llms.txt`（+ `site/static/llms.txt`、`site/public/llms.txt` 三份同哈希） | Editor support 一节 + 给 AI 的话术 |

改了 `site/content/**` 之后**必须**重生成搜索索引：

```bash
python site\tools\gen_search_index.py     # 期望：输出 "44 页 / 约 230 KB"
```

---

## 9. 验收清单（逐条打勾，全绿才算完成）

- [ ] `git status --short` 空
- [ ] `_prepublish.mjs` → `PREPUBLISH ALL PASS`（6 步）
- [ ] `_verify_ext.mjs` → `EXT VERIFY ALL PASS`
- [ ] `_verify_activate.mjs` → `ACTIVATE SMOKE ALL PASS`
- [ ] VSIX 装好 + 包内三文件与工作区一致（`vendor/sml.mjs` = 69404 B / `70f1ee47`）
- [ ] 「SML: 自检」面板里 `SML 语言已注册：✓`、`桥接层 … 已加载 ✓`、`语言模式：languageId = sml ✓`
- [ ] W19.3 的 6 件事逐项符合
- [ ] 非 SML 文件（.md/.json）**仍有图标**（图标主题回归点）
- [ ] Zed 侧：`tree-sitter parse` 无 `ERROR`（或注明「CLI 未安装，未验证」）
- [ ] `site/static/search-index.json` 与 `site/content/**` 同步（改了正文才需要）

## 10. 证据记录（贴命令与输出，别写「我试过了」）

| 项 | 命令 | 结果（贴关键行） |
|---|---|---|
| 闸门 1 | `node scripts/_prepublish.mjs` | |
| 闸门 2 | `node scripts/_verify_ext.mjs` | |
| 闸门 3 | `node scripts/_verify_activate.mjs` | |
| 包内一致 | 见 §3 | |
| 自检面板 | 右键 `SML: 自检` | |
| 字段悬浮 | 光标停在 `port`（第 36 行） | |
| 图标回归 | `SML: 文件图标主题` → SML+Seti | |

**任何一条不符 ⇒ 停下、贴证据、问人。不要猜，不要顺手改。**
