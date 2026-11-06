# 分发现状与选项（`bindings/` + `.ambr` 绑定）

> 本文件只写**现状与选项**，不含任何发布动作。发布流程是 `release.yml`（TODO T35，未建）。
> This file records **status and options only** — it performs no release. The release
> workflow is T35 and does not exist yet.

## 现在有什么

| 端 | 包管理器 | 元数据 | native 依赖 | 状态 |
|---|---|---|---|---|
| Rust `swsml` | crates.io | ✅ `Cargo.toml` | 无 | 已发多版（`0.6.9-dev.0`） |
| Rust `amberice` | crates.io | ✅ | 无 | workspace 版本，未单独发 |
| Rust `amberice-ffi` | — | `publish = false` | — | 只作构建产物，不入库不发 |
| Rust `swml` | crates.io | ✅ | 无 | `publish = false`，未发 |
| Python（SML） | PyPI | ❌ 无 `pyproject.toml` | 需要 `libsml` | 源码可用 |
| Python（`.ambr`） | PyPI | ❌ 无 `pyproject.toml` | 需要 `libamberice_ffi` | 源码可用 |
| Ruby | RubyGems | ❌ 无 `.gemspec` | 需要 `so`/`dll` | 源码可用 |
| Java（SML） | Maven | ❌ 无 `pom.xml` | Panama + `libsml` | 源码可用 |
| Java（`.ambr`） | Maven | ❌ 无 `pom.xml` | `Ambr` 纯 Java；`AmbrWrite` 需 `libamberice_ffi` | 源码可用 |
| Go | Go modules | ✅ `go.mod`（`module sml`） | cgo：`so`/`dll` + import lib | 源码可用 |
| C# | NuGet | ❌ `csproj` 无 `PackageId` | 需要 `dll` | 源码可用 |
| Lua | LuaRocks | ❌ 无 `.rockspec` | 读纯 Lua；写需 `libamberice_ffi` | 源码可用 |
| JS | npm | ❌ 无 `package.json` | 读纯 JS；写为 wasm | 源码可用 |
| C / C++ | vcpkg / Conan | ❌ | 读纯实现；写需 cdylib | 源码可用 |

「源码可用」= 把目录拷进项目、按各 README 编译即可；**不是**「一条命令安装」。

## 三条路，代价不同

### A. 每端 prebuild

每个包管理器都出多平台产物（wheel 矩阵 / NuGet `runtimes/<rid>/native` / gem precompiled）。

- 优点：`pip install` / `dotnet add package` 即用
- 代价：CI 矩阵翻倍；每个平台一份二进制；版本要对齐
- 适合：Python / C# / Java 这三个「装包即用」生态

### B. 只发 Rust crate，绑定留源码

- 优点：零维护，没有平台矩阵
- 代价：其它语言用户必须自己编 cdylib
- 适合：现在这个阶段

### C. 混合

有原生通道的端出包（C#/Java/Python/Ruby），纯实现端留源码（JS 浏览器 / Lua / C/C++）。

- 优点：成本集中在「装包即用」的生态
- 代价：两套支持口径，文档要分别写
- 这是我倾向的，但没有你的决定不会动

## 每个端缺的最小集合

dry-run 检查：`python3 tools/check/check_packaging.py`（只报告，不发布）

| 端 | 缺的文件 | 补它需要 |
|---|---|---|
| Python | `pyproject.toml` + `cibuildwheel` 配置 | 决定是「源码编译」还是「多平台 wheel」 |
| Ruby | `*.gemspec` + `extconf.rb` 或 precompiled | 同上 |
| Java | `pom.xml`（坐标 + 是否打 native jar） | 决定 native 走「同包」还是「首次运行下载」 |
| C# | `csproj` 的 `PackageId` / `runtimes/<rid>/native/` | 决定 native 打进包还是单独包 |
| Lua | `*.rockspec` | 决定是否声明 `libamberice_ffi` 为外部依赖 |
| JS | `package.json` + `files` / `optionalDependencies` | 决定浏览器走 wasm、Node 走 prebuild |
| Go | 不需要新文件；需要**交叉编译说明** | cgo 交叉工具链，或无解则只支持宿主平台 |
| C/C++ | vcpkg port / Conan recipe | 可选，源码集成本来就简单 |

## 两个硬事实（不是选择，是约束）

1. **Go 的 cgo 是静态链接**：只读 cdylib 会让包在**构建期**失败，做不到运行期查能力位。要「同一二进制对接两种构建」，只能放弃 cgo 改纯实现。
2. **Windows + MSVC rustc** 产出 `amberice_ffi.dll.lib`，GNU ld 不认；MinGW 用户必须先 `gendef` + `dlltool` 生成 `libamberice_ffi.dll.a`（见 `go/amberice/README.md`）。

## 未决

分发策略本身是决策点，`TODO` 里没有条目。在它被裁决前，所有端保持「源码可用」。
