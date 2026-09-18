# TASK-hy3：W16 收尾（Lua 11 处 → C++ 6 条 → 统一收口）

> **这份文档是给「不需要判断力」的执行者写的。** 每一条都给了：**改哪个文件的哪一行**、
> **改成什么**、**照抄哪段已验证的实现**、**跑什么命令**、**期望看到什么**、**不符怎么办**。
> 文档里的**行号是 2026-09-18 提交 `f8ebcd5` 时的实测值**；如果你读到的行对不上，
> **立刻停下**（说明文件已被人改过），不要凭猜。
>
> 目标一句话：**SML 有五个实现（Rust / C / C++ / JS / Lua），同一个坏输入必须在五端报同一个码。**
> Rust、JS、C 已经做完（C 是最完整的可照抄样板），这个任务把 **Lua** 和 **C++** 补上。

---

## 0. 铁律（违反就是失败，不需要讨论）

1. **不许自己发挥**：不重构、不「顺便优化」、不改与本任务无关的行、不调整格式。
2. **不许 `git add -A` / `git add .`**：只 `git add <你明确改过的文件>`。临时探针文件（`_` 开头）**不要提交**。
3. **不许 `git push`**：提交完就停，推送由用户本人做。
4. **不许动**：`.codebuddy/`、`rust/qsm/**`、`site/`、`editors/`、`Desktop\sml_secret\`（私有资产库）。
5. **不许删** `.gitignore` 里「私有报送件 / 内部报告」那一段（那是安全护栏）。
6. **一次只做一条**，做完一条就跑验证、贴证据，再进下一条。**不要攒着一起改**。
7. 任何一步的**实际输出与本文档不符** ⇒ 停下、把命令+输出贴出来问人，**不要猜着往下做**。
8. 临时探针文件一律以 `_` 开头（`.gitignore` 的 `**/_*` 会挡住它们，不会污染仓库）。

---

## 1. 环境与开工前检查（照抄）

```bash
cd c:\Users\sakeen\Desktop\sml

# 0) 工作区必须干净（若列出一堆改动，停下问人）
git status --short

# 1) 私有资产守卫必须 OK（说明没把敏感件弄进主库）
python tools/check_private_assets.py

# 2) 三个基线必须先全绿，否则说明环境坏了，先修环境
python c/build_check.py --run                    # 期望末行 === ALL PASSED ===
python lua/run_check.py                          # 期望 ALL LUA CHECKS PASSED
python cpp/build_verify.py                       # 期望六个 target 全 rc=0
```

工具路径（这台机器）：

- Lua 解释器：`C:\msys64\ucrt64\bin\luajit.EXE`
- Rust 构建**必须**指定目标目录（C 盘只剩 ~1.6 GB，会写满）：
  `set CARGO_TARGET_DIR=E:\snoware-target`（PowerShell：`$env:CARGO_TARGET_DIR="E:\snoware-target"`）

**黄金参考（「正确的码是什么」只能从这里抄，不许自己发明）**：

| 用途 | 位置 |
|---|---|
| 期望码 + 触发条件（Rust 是基准） | `rust/tests/error_codes.rs` |
| JS 端的同码用例 | `js/probe-error-codes.mjs` |
| **C 端完整实现**（本任务主要照抄对象） | `c/sml.c`（行号见 §2.2） |
| 码表（141 条，含每条的含义与备注） | `errors/codes.sml`（唯一事实来源，改码表要重跑生成器） |

---

## 2. 任务 A：Lua 11 处（A1–A11；`lua/lib/sml.soup`，1308 行）

### 2.0 先跑探针，留下「改动前」的证据（**必须做**）

把下面这个文件**原样**存成 `lua/_w16_probe.lua`（注意：`_` 开头，不会被提交）：

```lua
-- lua/_w16_probe.lua —— W16 行为矩阵自检探针（任务文档 TASK-hy3.md §2.0 用的就是这一份）
package.path = "./lua/?.soup;./lua/?/init.soup;" .. package.path
local Sml = require("lib.sml")

-- { 名称, 输入, 期望码（nil = 期望解析成功） }
local cases = {
  { "1 未闭合字符串",        'k: "abc\n',                "E-LEX-001" },
  { "2 未闭合块注释 /*",     "k: 1\n/* abc\n",           "E-LEX-002" },
  { "3 未闭合块注释 _*",     "k: 1\n_* abc\n",           "E-LEX-003" },
  { "4 未知转义",            'k: "a\\qb"\n',             "E-LEX-004" },
  { "5 \\u 位数不足",        'k: "\\u12"\n',             "E-LEX-005" },
  { "6 数组里多余的 }",      "m: [ } ]\n",               "E-PARSE-003" },
  { "7 闭合符错配 a { ] }",  "a { ] }\n",                "E-PARSE-002" },
  { "8 顶层多余的 }",        "k: 1\n}\n",                "E-PARSE-003" },
  { "9 顶层多余的 ]",        "k: 1\n]\n",                "E-PARSE-003" },
  { "10 未注册指令带体",     "@foo bar { x: 1 }\n",      "E-PARSE-005" },
  { "11 未注册指令无体",     "@foo bar\n",               "E-PARSE-005" },
  { "12 未定义片段引用",     "x: &nosuchfrag\n",         "E-INCLUDE-006" },
  { "13 顶层标量 42",        "42\n",                     "E-PARSE-008" },
  { "14 顶层引号串",         '"42"\n',                   "E-PARSE-008" },
  -- 15 是「现在报**错**码、改完要能成功」的特例（Lua 目前不支持片段显式参数）
  { "15 片段显式参数(应成功)", "@foo type: Server name: p { x: 1 }\n", nil },
  -- ↓↓↓ 正对照：**必须一直是「解析成功」**，谁把它们改成报错就是改错了 ↓↓↓
  { "P1 合法转义",           'k: "a\\nb\\t\\r\\0\\"c\\\\d"\n', nil },
  { "P2 \\u 定长四位",       'k: "\\u4e2d"\n',           nil },
  { "P3 \\u 花括号形式",     'k: "\\u{1F680}"\n',        nil },
  { "P4 块注释正常闭合",     "k: 1\n/* ok */\n",         nil },
  { "P5 _* 注释正常闭合",    "k: 1\n_* ok *_ \n",        nil },
  { "P6 片段定义（无参数）", "@foo { x: 1 }\n",          nil },
  { "P7 片段定义+引用",      "@foo { x: 1 }\ny: &foo\n", nil },
  { "P9 两 token 裸键对",    "hello world\n",            nil },
  { "P10 键值块",            "42: x\n",                  nil },
  { "P11 对象块",            "{ a: 1 }\n",               nil },
  { "P12 顶层数组",          "[1, 2]\n",                 nil },
  { "P13 嵌套数组",          "m: [ 1, [2, 3], 4 ]\n",    nil },
  { "P14 带指令的顶层标量",  "@version v1\n42\n",        nil },
  { "P15 空输入",            "",                         nil },
  { "P16 只有注释",          "# c\n",                    nil },
}

local pass, fail = 0, 0
for _, c in ipairs(cases) do
  local ok, res, err = pcall(Sml.load, c[2])
  local got
  if not ok then
    got = "抛异常"
  elseif res == nil then
    got = tostring(err):match("^(E%-[A-Z]+%-%d+)") or ("失败:" .. tostring(err))
  else
    got = nil                       -- 解析成功
  end
  local want = c[3]
  local good = (got == want)
  if good then pass = pass + 1 else fail = fail + 1 end
  print(string.format("%-24s 期望=%-14s 实得=%-14s %s",
        c[1], tostring(want), tostring(got), good and "PASS" or "**FAIL**"))
end
print(string.format("\nPASS %d / FAIL %d（共 %d 条）", pass, fail, #cases))
```

跑它并把输出存下来：

```bash
cd c:\Users\sakeen\Desktop\sml
C:\msys64\ucrt64\bin\luajit.EXE lua\_w16_probe.lua > %TEMP%\lua_before.txt
```

**改动前的实测基线（2026-09-18 用上面这份探针**真跑过**，不是估的）**：
**`PASS 17 / FAIL 13`（共 30 条）** —— FAIL 的 13 格是第 1–7、10–15 格（第 8、9 格本来就对；
第 15 格比较特殊：它现在报的是 `E-PARSE-006`，而期望是**解析成功**）。
**如果你的基线不是 17/13，停下问人**（说明 `lua/lib/sml.soup` 已被人改过）。
`PASS 17` 里除第 8/9 格外全是正对照，一条都不许被改坏。

---

### 2.1 ⚠️ 先改这一处（A0，不改它，后面所有词法错误都会被吃掉）

- **文件**：`lua/lib/sml.soup`
- **位置**：`Sml.load` 函数里，**第 1152–1158 行**
- **现状（已读代码确认）**：

  ```lua
  1152:  local ok, toks = pcall(tokenize, src)
  1153:  if not ok then
  1154:    -- tokenize 目前不会抛（词法层刻意宽松），留此分支只为不静默吞掉意外异常。
  1157:    return nil, "E-PARSE-012 sml: 词法阶段异常: " .. tostring(toks)
  1158:  end
  ```

  ⇒ **这里无条件把异常包装成 `E-PARSE-012`**。所以等你在词法层 `fail("E-LEX-001", …)` 之后，
  用户看到的会是 `E-PARSE-012`，**不是** `E-LEX-001` —— 第一格就会卡住。
- **改成**（照抄同文件 1144–1148 行与 1179–1185 行**已有的**写法：带码的原样透出）：

  ```lua
  local ok, toks = pcall(tokenize, src)
  if not ok then
    -- 词法层 `fail()` 抛出的错**带码**，必须原样透出（与下面解析阶段同一个规矩）；
    -- 不带码的才是「没能给出更具体原因」→ E-PARSE-012 兜底。
    local m = tostring(toks)
    if has_code(m) then return nil, m end
    return nil, "E-PARSE-012 sml: 词法阶段异常: " .. m
  end
  ```

- **验证**：改完先跑 §1 的 `python lua/run_check.py` —— **必须仍然 `ALL LUA CHECKS PASSED`**
  （这一步只改错误透传，不该动任何既有行为）。跑 §2.0 的探针，失败数**仍是 14**（还什么都没修）。

---

### 2.2 改动总表（A1–A11，**照抄 `c/sml.c` 的对应实现**）

> `lua/lib/sml.soup` 的行号是改前（`f8ebcd5`）的行号；改完一处，后面的行号会往上飘，
> **所以每条都给了「找什么文本」**，用文本定位，不要只信行号。

| # | 条件 | Lua 改哪里（找什么） | 期望码 | C 的照抄位置（已验证） |
|---|---|---|---|---|
| A1 | `/*` 未闭合到文件结尾 | `tokenize` 的 `elseif c == "/" and c2 == "*" then` 分支（125–133 行）：那个 `while i <= n do … break end` 循环**没有记录是否闭合** | `E-LEX-002` | `c/sml.c:377` |
| A2 | `_*` 未闭合到文件结尾 | 同上，`elseif c == "_" and c2 == "*" then`（134–142 行） | `E-LEX-003` | `c/sml.c:387` |
| A3 | 字符串未闭合到文件结尾 | `elseif c == '"' then` 分支（143–172 行）：`while j <= n` 里 `cc == '"' then break`，**没有闭标记** | `E-LEX-001` | `c/sml.c:460` |
| A4 | 字符串含未知转义 | 同分支 154–165 行的转义白名单：**第 165 行 `else qb[#qb+1] = e end`**（未知转义**静默丢反斜杠**：`"a\qb"` → `aqb`） | `E-LEX-004` | `c/sml.c:446` |
| A5 | `\u` 转义非法/位数不足 | 同分支：**现在完全没处理 `u`**（落到 165 行的 else，`"\u4e2d"` → `u4e2d`，**数据被改坏**） | `E-LEX-005` | `c/sml.c:436` |
| A6 | 数组里多余的 `}` | `parse_array`（786–808 行）：`tok == "}"` 落进最后那个 `else`，被 `coerce` 当元素**收进数组** | `E-PARSE-003` | `c/sml.c:1100`（或 1132） |
| A7 | 闭合符错配 `a { ] }` | `parse_block` 的 `elseif tok == "]" then`（649–654 行）：`closing == "}"` 时**错报 003** | **`E-PARSE-002`** | `c/sml.c:1141` |
| A8 | 未注册指令：位置参数形态 / 没有片段体 | `parse_block` 的 `@` 分支 else（707–726 行）：`ftype/fname_arg` 是**位置参数**读法；且 `tokens[i] ~= "{"` 时**什么都不做**（整条静默丢掉） | `E-PARSE-005` | `c/sml.c:1271` |
| A9 | 未定义片段引用 `&name` | `coerce` 的 `if string.sub(tok, 1, 1) == "&" then`（226–232 行）：**第 231 行 `return tok  -- 未定义引用原样保留`** | `E-INCLUDE-006` | `c/sml.c:670` |
| A10 | 顶层标量（`42` / `"42"`） | `Sml.load` 第 1171–1177 行（`local first = toks[1]` 那三个分支）之前 | `E-PARSE-008` | `c/sml.c:1412` |
| A11 | 片段**显式参数** `@foo type: X name: Y { … }`（**能力缺失**：现在报 `E-PARSE-006`，探针第 15 格） | 同 A8 那一段（707–726 行）：它只认**位置参数** —— `type` 被当位置参数吃掉后，紧跟的 `:` 掉进键位分支（732 行 `fail("E-PARSE-006"…)`） | 应**解析成功**（无码），且参数要进 `__type` / `__name`（供 `&` 引用还原） | `c/sml.c:1271`（同一处的 `is_param` 判据 + 显式参数读法） |

**A1–A3 的通用改法**（三处一模一样，只是码不同）：给循环加一个「是否闭合」的标记，
循环结束后**没闭合就 `fail(...)`**：

```lua
-- 例：A1（`/*`）。A2 把码换成 E-LEX-003、结束符换成 `*_`；A3 见下面单独说明
elseif c == "/" and c2 == "*" then
  i = i + 2
  local closed = false                      -- ← 新增
  while i <= n do
    local a = string.sub(text, i, i)
    local b = (i < n) and string.sub(text, i + 1, i + 1) or ""
    if a == "*" and b == "/" then i = i + 2; closed = true; break end   -- ← 记下闭合
    i = i + 1
  end
  if not closed then fail("E-LEX-002", "未闭合的块注释 /* ... */（遇到文件结尾）") end  -- ← 新增
```

**A3（未闭合字符串）**：在 `local qb = {}` 旁边加 `local closed = false`，
在 `if cc == '"' then break end` 改成 `if cc == '"' then closed = true; break end`，
循环结束后（172 行 `i = j + 1` 之前或之后）：

```lua
if not closed then fail("E-LEX-001", "字符串未闭合（缺少结束引号）") end
```

⚠️ **只在这个循环里判**：`while j <= n` 正常走完才说明遇到文件结尾。
（C 那边是多加了一个「缓冲区满就 break」的保护分支，Lua 这个循环没有该分支，不用管。）

**A4（未知转义）改法 —— 注意这里有个「顺带的口径变化」**：

Lua 现在的白名单（154–165 行）含 **Lua 特有的** `a` `b` `f` `v` `'`，
而 Rust / C / JS / C++ 的合法转义集**只有** `n` `t` `r` `0` `"` `\` 和 `\u`。
本任务要把 Lua 对齐到**同一集合**：

```lua
-- 只保留：n t r 0 " \ u ；其余（含 a b f v '）一律报 E-LEX-004
if e == "n" then qb[#qb+1] = "\n"
elseif e == "t" then qb[#qb+1] = "\t"
elseif e == "r" then qb[#qb+1] = "\r"
elseif e == "0" then qb[#qb+1] = "\0"
elseif e == "\\" then qb[#qb+1] = "\\"
elseif e == '"' then qb[#qb+1] = '"'
elseif e == "u" then
  -- A5：\uXXXX（定长四位）或 \u{...}，非法 ⇒ E-LEX-005（照 c/sml.c:436 的逻辑）
  -- …（下面 A5 单独讲）
else
  fail("E-LEX-004", "字符串含未知转义符 \\" .. e .. "（仅支持 \\n \\t \\r \\0 \\\" \\\\ \\uXXXX）")
end
```

⚠️ **这是一处行为变更**（此前 `\a`、`\v` 能用，往后会报错）。所以 **A4 做完必须做 §2.4 的全仓扫描**：
如果仓库里有 `.sml` 文档正在用 `\a` `\b` `\f` `\v` `\'`，**停下报告**，不要自己决定怎么办。

**A5（`\u`）**：照着 `c/sml.c:436` 附近那段写（定长四位必须**恰好 4 位**；
`\u{...}` 花括号形式；非十六进制 / 花括号未闭合 / 空 / 码点 > `0x10FFFF` /
落在代理区 `D800–DFFF` ⇒ 全部 `E-LEX-005`；合法则按 UTF-8 编进结果）。
Lua 端可以用一个小助手把码点转 UTF-8（`string.char` + 位运算，或 `utf8.char`——
**LuaJIT 是 5.1，可能没有 `utf8` 库，先用 `if utf8 then … end` 试，没有就手写**；
写完**必须**用探针 P2/P3 两条正对照证明 `\u4e2d` 与 `\u{1F680}` 能正确还原成 `中` 和 `🚀`。

**A7（错配 vs 多余）的判据（必须分清，别一刀切）**：

| 情形 | `closing` 的值 | 现在 | 应改为 |
|---|---|---|---|
| `a { ] }`：块内遇到 `]` | `"}"` | `E-PARSE-003` ❌ | **`E-PARSE-002`**（闭合符错配） |
| `k: 1` + `]`：顶层遇到 `]` | `nil` | `E-PARSE-003` ✅ | 保持 `E-PARSE-003` |
| `k: 1` + `}`：顶层遇到 `}` | `nil` | `E-PARSE-003` ✅ | 保持 `E-PARSE-003` |

（`closing == nil` 就是**顶层**：见 `Sml.load` 第 1177 行 `parse_block(toks, 1, nil, …)`。）

**A8（未注册指令）判据（照 `c/sml.c:1271` 的 `is_param`）**：

1. 只有**紧跟着冒号**的 `type` / `name` 才算参数（`@foo type: Server { … }`）；
2. 参数读完后，**下一个 token 必须是 `{`**；不是 ⇒ `E-PARSE-005`；
3. 出现了**位置参数**（即没有冒号的裸词，如 `@foo bar { … }`）⇒ `E-PARSE-005`；
4. **没有片段体**（`@foo bar`、`@foo`）⇒ `E-PARSE-005`。
5. **显式参数形式必须支持**（A11）：`@foo type: Server name: prod { x: 1 }` 要**成功解析**，
   不能也当成位置参数报 005 —— 探针第 15 格就是钉这个的。

⚠️ **正对照 P6 / P7 必须仍然通过**（原 P8「显式参数」已挪到第 15 格，见 A11）：
`@foo { x: 1 }`（无参数、带体）是**合法片段定义**。
**绝对不要**写成「`@` 开头一律报 005」——那样会把合法文档打死。

**A10（顶层标量）判据**：`#toks == 1 且 toks[1] 不是 "{" 也不是 "["` ⇒ `E-PARSE-008`。
（`hello world` 是 2 个 token ⇒ 不报；`@version v1` + `42` 是 4 个 token ⇒ 不报；
空输入 0 个 token ⇒ 不报。三条都在正对照 P9/P12/P14/P15/P16 里。）

**关于 `fail()` 的机制**（不需要你额外做短路）：
`fail(code, msg)`（第 60 行）会**抛异常**，`Sml.load` 的 `pcall`（1163、1179 行）
见到**带码**的消息就**原样透出**（1184 行）⇒ 词法错误抛出来后**不会再被语法错误覆盖**，
这一点比 C 省事（C 需要额外的 `failed` 标记）。

---

### 2.3 A1–A11 全部改完后，探针必须全绿

```bash
cd c:\Users\sakeen\Desktop\sml
C:\msys64\ucrt64\bin\luajit.EXE lua\_w16_probe.lua > %TEMP%\lua_after.txt
type %TEMP%\lua_after.txt
```

**期望**：末行 `PASS 30 / FAIL 0（共 30 条）`。
**只要有一条 FAIL，就不要往下做**，把那一行的用例输入与实得输出贴出来。

### 2.4 全仓扫描（**必做**，A4 的口径变化必须靠它兜底）

1. 新建 `lua/_w16_scan.lua`（`_` 开头 ⇒ 不会被提交），骨架：

   ```lua
   -- 扫全仓 *.sml，打印「码 或 OK — 路径」；用法: luajit lua/_w16_scan.lua > 输出文件
   package.path = "./lua/?.soup;./lua/?/init.soup;" .. package.path
   local Sml = require("lib.sml")
   local function walk(dir, out)
     local p = io.popen('dir /b /s "' .. dir .. '\\*.sml"')   -- Windows
     for f in p:lines() do out[#out+1] = f end
     p:close()
   end
   local files = {}
   walk(".", files)
   table.sort(files)
   for _, f in ipairs(files) do
     local h = io.open(f, "rb"); local t = h:read("*a"); h:close()
     local res, err = Sml.load(t, nil)                 -- base=nil：include 关闭（与改动前同口径）
     local tag = res and "OK" or (tostring(err):match("^(E%-[A-Z]+%-%d+)" ) or "FAIL")
     print(string.format("%-14s %s", tag, f))
   end
   ```

2. **改动前**先跑一遍留基线：`git stash` 不行（你还没提交）——用下面的办法：
   先把当前 `lua/lib/sml.soup` 备份成 `lua/_sml_before.soup`，再
   `git show HEAD:lua/lib/sml.soup > lua/lib/sml.soup` 跑扫描 → 存成 `%TEMP%\scan_before.txt`，
   然后**把备份还原回去**（还原后必须 `git status --short` 只显示你正在改的那个文件）。
3. 改动后再跑一遍 → `%TEMP%\scan_after.txt`。
4. **逐条比对差集**：凡是「改动前 OK / 改动后报错」的文件，**一条一条**在报告里写清
   「文件 → 改后码 → 改动前那棵树长什么样」。判据：**如果改动前那棵树是错的（或缺内容），
   那就是「静默给错树 → 现在响亮拒绝」，可以接受**；**如果改动前那棵树是对的，那是误伤，
   必须停下报告**（最可能是 A4 的转义口径变化引起的）。

### 2.5 补测试用例（`lua/test_codes.lua`）

照 `c/test_codes.c` 里 **W16 那两组**的用例清单往 `lua/test_codes.lua` 里逐条加
（输入 + 期望码**逐字照抄**，不要自己编用例）。用例清单就是 §2.0 探针里的 `cases`
（**14 条要报错的 + 16 条正对照** —— 16 里含第 15 格那种「原本报错、改后必须成功」的），
**一条都不能少**。
加完跑 `python lua/run_check.py`，期望 `ALL CODE TESTS PASSED`，且打印的通过条数 = 原有 + 30。

### 2.6 判别实验（**这是验收的核心，必须做**）

目的：证明「改动前确实静默/报错码不对」。

```bash
cd c:\Users\sakeen\Desktop\sml
python -c "import hashlib;print(hashlib.sha256(open('lua/lib/sml.soup','rb').read()).hexdigest())"  # 记下当前 sha
git show HEAD:lua/lib/sml.soup > lua/_sml_head.soup
copy lua/lib/sml.soup lua/_sml_new.soup
copy lua/_sml_head.soup lua/lib/sml.soup
C:\msys64\ucrt64\bin\luajit.EXE lua\_w16_probe.lua > %TEMP%\lua_head_probe.txt   # 期望：FAIL 很多（≈14+）
copy lua\_sml_new.soup lua\lib\sml.soup                                          # ★ 必须还原
```

**期望**：HEAD 版探针 **FAIL 13**（新实现 0）；`lua/run_check.py` 在 HEAD 版下**必须红**。
还原后**再对一次 sha256**（与开头记的相同），并跑 `python lua/run_check.py` 确认又绿了。
**还原失败（sha 不一致）立即停手报告** —— 别把 HEAD 版当成品提交。

---

## 3. 任务 B：C++ 6 条（`cpp/sml.cpp`）

> **第一步不是改代码，是实测。** 上一轮的经验：判定表上写的和实际跑出来的**不一样**
> （Lua 判定表记 6 条，实测 9 处）。所以：**先量，再改，并把量到的结果写进报告。**

1. **量基线**：`python cpp/build_verify.py` ⇒ 六个 target 全 `rc=0`（记下输出）。若不是，先停下。
2. **写探针** `cpp/_w16_probe.cpp`（`_` 开头 ⇒ 不提交）：把 §2.0 那 30 个用例**照抄**成 C++ 数组，
   对每个跑 `sml::parse`，打印 `名称 | 期望码 | 实得码 | PASS/FAIL`。
   编译命令参考 `cpp/` 里现有的构建脚本（`cpp/build_verify.py` 里有它用的编译参数，照抄）。
3. **逐格与黄金表比对**：期望值取 `rust/tests/error_codes.rs`。
   把「实测静默通过」的每一格列成表 → **这张表就是 C++ 的任务清单**。
4. **逐条修**：每修一条就重跑探针，确认那一格由 FAIL 变 PASS，且**没有把别的格从 PASS 弄成 FAIL**。
5. **补 C++ 的 CODES 套件**：把这 30 条加进 `cpp/` 的 CODES target（80 条 → 110 条），
   `python cpp/build_verify.py` 必须 rc=0。
6. **判别实验**：`git show HEAD:cpp/sml.cpp` 换进去跑同一份探针 ⇒ **必须红**；换回来对 sha256。
7. **全仓扫描**：做法同 §2.4（C++ 有 `include_dir` 概念，注意 base 传 nil 以关闭 include）。

⚠️ C++ 与 C 的**已知差异**（别去「顺手统一」，那是另一个任务）：C++ 的深度守卫、
include 展开位置、错误码取值都有各自的历史原因；**本任务只加 W16 那些静默点的报错**。

---

## 4. 任务 C：统一收口（A、B 都绿了才做）

| 步骤 | 做什么 | 具体位置 |
|---|---|---|
| C1 | 码表 `impls` 回填：`E-LEX-001/002/003/004/005`、`E-PARSE-002/003/005/008`、`E-INCLUDE-006` 加上 `lua`（C++ 同理），并写清「此前静默」的备注 | `errors/codes.sml` |
| C2 | 重跑两个生成器（码表是唯一事实来源） | `python errors/gen_json.py` + `python errors/gen_codes.py`，然后 `git status --short` 看生成物是否真的变了 |
| C3 | README 静默清单改写：把「C++ / Lua（未做）」那一段改成实际状态 | `errors/README.md` |
| C4 | CHANGELOG：在 `## [未发布]` 的 `### 变更` 下加一段（表格：输入 / 改前 / 改后码），并在 `### 修复` 下写查出来的附带问题 | `CHANGELOG.md` |
| C5 | 进度表补行（含判别实验的红条数、全仓扫描的差集判定） | `errors/silence-decisions.md` §4 |
| C6 | TODO 的 W16 行更新（剩什么就写剩什么；**W3 若被 Lua/C++ 那格顺带完成，也要标**） | `TODO.md` |
| C7 | 往 `HANDOFF.md` 追加一节（编号顺延），写清：改了什么、判别实验结果、全仓扫描判定、**遗留与教训** | `HANDOFF.md` |

---

## 5. 验收清单（逐项打勾，缺一不可）

- [ ] §1 三项基线命令全绿（C / Lua / C++）
- [ ] `git status --short` 在开工前是空的
- [ ] Lua：探针 `PASS 30 / FAIL 0`
- [ ] Lua：`python lua/run_check.py` → `ALL LUA CHECKS PASSED`（条数 = 原 120 + 30）
- [ ] Lua：判别实验 HEAD 版 **FAIL 13**、还原后 sha256 与开工前一致
- [ ] Lua：全仓扫描 before/after 差集**逐条判定**并写进报告（无误伤）
- [ ] C++：先有实测矩阵，再改；探针 `PASS 30 / FAIL 0`
- [ ] C++：`python cpp/build_verify.py` 六 target rc=0，CODES 条数 = 原 80 + 30
- [ ] C++：判别实验 HEAD 版必须红 + 还原 sha256 一致
- [ ] 码表 `impls` 回填 + 两个生成器重跑（生成物有 diff）
- [ ] CHANGELOG / README / silence-decisions / TODO / HANDOFF 五处都改了
- [ ] `python tools/check_private_assets.py` 仍 OK
- [ ] 提交**按主题分笔**（Lua 一笔、C++ 一笔、收口一笔），每笔都**逐文件** `git add`
- [ ] **没有**做 `git push`

---

## 6. 提交规范（照抄现有习惯）

```bash
git add lua/lib/sml.soup lua/test_codes.lua      # 只加你改过的；不带 -A
git commit -m "fix(lua): W16 的 Lua 批 —— <一句话>

<正文：改前静默 / 改后码 的表格；判别实验（HEAD 版 N 条红）；全仓扫描差集判定；
 顺带查出的问题；码表与文档收口>"
```

提交信息末尾**必须**带证据（判别实验的条数、扫描的判定结论）。**不许** `--no-verify`、
**不许** `-A`、**不许** push。

---

## 7. 什么时候必须停手问人

1. 本文档任何一条的**实测结果与文档不符**（行号对不上、期望码不符、条数不符）。
2. 全仓扫描出现「**改动前是正确文档 / 改动后报错**」——**这是误伤，绝对不许自己糊过去**。
3. 某个用例修了 **3 次**还是 FAIL。
4. 需要改**公开 API**（如 C 的 `sml_parse_json` 加 err 参数）——那要用户拍板。
5. 发现要动 `rust/qsm/**`、`site/**`、`editors/**`、`sml_secret` 才能继续。
6. 磁盘空间不足、构建要写 `C:\`（Rust 记得 `CARGO_TARGET_DIR=E:\snoware-target`）。

停下来时报告格式：**「我在做 A几 / B几 + 命令 + 完整输出 + 我卡住的判断点」**，不要只写「失败了」。

---

## 8. 附：为什么是这些码（一句话一码，供你判断合理性）

| 码 | 含义 |
|---|---|
| `E-LEX-001` | 字符串没闭合（读到文件结尾都没等到结束引号） |
| `E-LEX-002` | `/* … */` 块注释没闭合 |
| `E-LEX-003` | `_* … *_` 块注释没闭合 |
| `E-LEX-004` | 字符串里有不认识的转义（只允许 `n t r 0 " \ u`） |
| `E-LEX-005` | `\u` 转义非法（位数不对、非十六进制、码点越界或落代理区） |
| `E-PARSE-002` | 闭合符错配（期望 `}` 却遇到 `]`，或反之） |
| `E-PARSE-003` | 多余的结束符号（顶层没有与之匹配的开始符号 / 数组里冒出 `}`） |
| `E-PARSE-005` | 不是合法指令，且缺少片段体 `{ … }` |
| `E-PARSE-008` | 顶层只能是容器（键值块 / 对象块 / 数组），单个标量无法往返 |
| `E-INCLUDE-006` | 片段引用 `&name` 指向一个不存在的片段 |

**共同原则**：这些输入以前**不报错**（有的还会静默给出一棵**错的树**，比如 `m: [ } ]` 得到
`{"m":[]}`），现在改成**响亮拒绝**。**代价**是少数以前「能解析」的文档会开始报错 ——
所以每一处都配**正对照**，并且必须做全仓扫描来证明「没有把正确的文档打死」。
