---
title: "下载"
---

<style>
.dl-btn, details.feature > summary {
  display: inline-block; cursor: pointer; text-decoration: none;
  border: 1px solid #4a90d9; color: #4a90d9; border-radius: 6px;
  padding: 3px 12px; font-size: .92em; user-select: none; margin: 2px 4px 2px 0;
}
.dl-btn:hover, details.feature > summary:hover { background: #4a90d9; color: #fff; }
details.feature > summary { list-style: none; }
details.feature > summary::-webkit-details-marker { display: none; }
details.feature[open] > summary { background: #4a90d9; color: #fff; }
details.feature { margin: 8px 0; }
.feature-note { color: #c0392b; font-size: .9em; }
.dl-table { border-collapse: collapse; width: 100%; margin: 12px 0 24px; }
.dl-table th, .dl-table td { border: 1px solid var(--border); padding: 7px 12px; text-align: left; vertical-align: top; }
.dl-table th { background: var(--th); color: var(--accent-strong); font-weight: 600; }
.dl-table td b { font-size: 1.02em; }
.dl-table .meta { color: var(--fg-dim); font-size: .82em; display: block; margin-top: 2px; }
</style>

# 下载 SML { ❄ }

所有下载项均基于 [MulanPSL-2.0](/LICENSE) 许可发布，源码见 [仓库](https://gitee.com/snoware/sml)。
各语言绑定统一经 C-ABI（`c/sml_rs.h` v3）调 Rust cdylib `sml`/`libsml`；读路径直接遍历值树、不经 JSON；
源码包内含 `LICENSE` 与 `README`，可直接嵌入你的工程。

## 下载

<table class="dl-table">
<thead><tr><th>项目</th><th>版本</th><th>说明</th><th>下载</th></tr></thead>
<tbody>
<tr>
<td><b>VS Code 扩展</b></td><td><code>0.4.3</code></td>
<td>编辑器插件：高亮 / 诊断 / 补全 / 跳转 / 格式化 + SML 专属编辑能力</td>
<td><a href="/dl/sml-lang-0.4.3.vsix" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>Python</b></td><td><code>0.1.0</code></td>
<td>ctypes 调 cdylib，零三方依赖；值树直访</td>
<td><a href="/dl/sml-python-0.1.0.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>Ruby</b></td><td><code>0.1.0</code></td>
<td>fiddle 调 cdylib，零 gem 依赖；值树直访</td>
<td><a href="/dl/sml-ruby-0.1.0.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>Java</b></td><td><code>0.1.0</code></td>
<td>JDK Panama FFM 调 cdylib；值树直访</td>
<td><a href="/dl/sml-java-0.1.0.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>Go</b></td><td><code>0.1.0</code></td>
<td>cgo 调 cdylib；值树直访</td>
<td><a href="/dl/sml-go-0.1.0.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>C#</b></td><td><code>0.1.0</code></td>
<td>P/Invoke 调 cdylib（本机未验证）</td>
<td><a href="/dl/sml-csharp-0.1.0.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>C</b></td><td><code>0.6.3</code></td>
<td>纯 C99 自包含实现 或 Rust 桥接头 二选一</td>
<td><a href="/dl/sml-c-0.6.3.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>C++</b></td><td><code>0.6.3</code></td>
<td>C++17 零依赖实现 或 Rust 桥接头 二选一</td>
<td><a href="/dl/sml-cpp-0.6.3.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>JavaScript</b></td><td><code>0.6.3</code></td>
<td>零依赖 ESM，与 playground 同解析器</td>
<td><a href="/dl/sml-javascript-0.6.3.zip" class="dl-btn">下载</a></td>
</tr>
<tr>
<td><b>Lua</b></td><td><code>0.6.3</code></td>
<td>Lua 绑定，随仓库参考实现</td>
<td><a href="/dl/sml-lua-0.6.3.zip" class="dl-btn">下载</a></td>
</tr>
</tbody>
</table>

## Debian / Ubuntu 包（libswsml · amd64）

Rust cdylib 后端的系统集成包（`swsml` 0.6.3，SONAME `libswsml.so.0`）。装好即用：共享库进 `ldconfig`，头文件进 `/usr/include/sml`，`pkg-config` 一并就位。按后端拆成四个包，按需取用：

<table class="dl-table">
<thead><tr><th>包</th><th>版本</th><th>说明</th><th>下载</th></tr></thead>
<tbody>
<tr><td><b>libswsml0</b></td><td><code>0.6.3</code></td><td>运行时：Rust cdylib C-ABI（<code>libswsml.so.0</code>），v1..v4 全集能力</td><td><a href="/dl/libswsml0_0.6.3_amd64.deb" class="dl-btn">下载</a></td></tr>
<tr><td><b>libswsml-dev</b></td><td><code>0.6.3</code></td><td>C 开发：<code>sml_rs.h</code> + <code>libswsml.so</code> + <code>pkg-config swsml</code></td><td><a href="/dl/libswsml-dev_0.6.3_amd64.deb" class="dl-btn">下载</a></td></tr>
<tr><td><b>libswsml-cpp0</b></td><td><code>0.6.3</code></td><td>C++ 运行时：RAII 封装（<code>libswsml_cpp.so.0</code>）</td><td><a href="/dl/libswsml-cpp0_0.6.3_amd64.deb" class="dl-btn">下载</a></td></tr>
<tr><td><b>libswsml-cpp-dev</b></td><td><code>0.6.3</code></td><td>C++ 开发：<code>sml_rs.hpp</code> + <code>pkg-config swsml_cpp</code></td><td><a href="/dl/libswsml-cpp-dev_0.6.3_amd64.deb" class="dl-btn">下载</a></td></tr>
</tbody>
</table>

```sh
# 运行时
sudo dpkg -i libswsml0_0.6.3_amd64.deb libswsml-cpp0_0.6.3_amd64.deb
# 开发（含头文件 + pkg-config）
sudo dpkg -i libswsml-dev_0.6.3_amd64.deb libswsml-cpp-dev_0.6.3_amd64.deb
# 之后即可：gcc app.c $(pkg-config --cflags --libs swsml) -o app
#           g++ -std=c++17 app.cpp $(pkg-config --cflags --libs swsml_cpp) -o app
```

> 目前仅 **amd64**；其他架构可用仓库内 `packaging/build_deb.sh --arch <arch>` 自建。
> 命名刻意用 `libswsml`，避开 Debian 官方 `libSML` / `libsml-dev`（智能电表生态）冲突。
> 依赖：`libc6`、`libgcc-s1`（C++ 包另需 `libstdc++6`）。

## 校验和（SHA256）

下载后请核对哈希。下面即为 `sha256sum -c` 兼容格式——整段存为 `SHA256SUMS`，与下载文件放同一目录后运行 `sha256sum -c SHA256SUMS` 即可一次校验全部：

```text
4a325ac778f6c666d47430fd394ad390f74b722440c2563f811ab240ce27375d  sml-lang-0.4.3.vsix
928c37067c834865afd88ee06149587ac1bcdbdab552775ca1c15cbf97ed9af6  sml-python-0.1.0.zip
85250c426e5724ed26371ed7f8690cb7695c40e7e157379d1ab62827ad23a81c  sml-ruby-0.1.0.zip
12a224512fbdb2e60b4fd82ae6912f801990eec50108ae14aad21e7d50ecf1a8  sml-java-0.1.0.zip
4099808da508f1fd8cf4e15948c54aec84690b035255a12a131fe451287a70b2  sml-go-0.1.0.zip
1e9a7b1ca0e87ba7d6554cfee9f709c6acb3723c203cb088f53b0f3921fc02dd  sml-csharp-0.1.0.zip
09ee5772007c106a684cd7b97ac01821863ff8aa371f8a01e71bda38c6621cd1  sml-c-0.6.3.zip
02ef4a95542a6fd6af9443cef58a494cdf3695441ab5a553b9ce916ff109e774  sml-cpp-0.6.3.zip
584d05e7be2958333e09cb20d21f25ebc0e7fecf8ad3da8d733161e4ebb6dd30  sml-javascript-0.6.3.zip
9e6688d1e4326cb3bf63bbf62347f49607b62a0ca7b35ffa0d1ed55ad37c27e9  sml-lua-0.6.3.zip
c5ded19ba858252c4b06ef1d783dba3f70c4d67451c89a98c5063623170f4c74  libswsml0_0.6.3_amd64.deb
06cbb1ed4f255ad8c23b1aba62792dc29b01e345cf74a3a401a4eada9d8c79ba  libswsml-dev_0.6.3_amd64.deb
b51d0fa9e738c3206d32c84745a1a8687f861b74debcd6be0557ffcb7bb3277e  libswsml-cpp0_0.6.3_amd64.deb
e174fa652aff0755d19ad99779ce1b3a1b516d8c6c62422f96f1e7e41aa6dd1b  libswsml-cpp-dev_0.6.3_amd64.deb
```

## 各实现说明（点击展开）

<details class="feature"><summary>VS Code 扩展 · 0.4.3</summary>

为 VSCode 提供 SML 的语法高亮、错误提示、补全、悬浮说明、跳转与格式化，以及一批
「只在 SML 里说得通」的编辑期能力：

- **悬浮**：契约名（`@is Server`）显示契约声明 + **解析器按契约填充默认值后的实际结构**；
  块名（`primary {`）显示**路径**（`database.primary`）、它应用的契约与填充后的结构
- **跳转到定义**：`@is Server` → `@contract Server`，`&base` → `@base { }`
- **include / 命名空间导航**：路径上 F12 跳到被包含文件、悬停给「该文件的顶层键」；
  命名空间别名（`as db`）悬停给「这个命名空间里**实际有哪些键**」，F12 跳到被包含文件；
  引用（`db.host`）悬停告诉你它来自哪个文件的什么定义，F12 直接跳过去
- **特殊颜色**：选中一个词 → 右键「应用特殊颜色」，写进工作区的 `HL-cfg.sml`
  （随仓库走、可直接手写编辑）；默认**只对语法单元生效**（契约 / 片段 / 类型 / 键 / 指令），
  不会把注释与字符串里的同名文字一起染掉；**也认命名空间前缀**（`@is ui.base.Service`
  这类引用处照样着色）。**写 DSL / 做分析时这套东西的用法**见扩展 README 的同名小节
- **特别高亮**：把某个词在**整个工作区**里点亮（状态栏显示 N 处 / M 文件）
- **自检**：右键「SML: 自检」把「为什么没反应」逐条摊开写进「输出 → SML」
  （扩展版本、**包内解析器指纹**、命令注册、语言模式、文档校验结果、能不能跳转）

- 安装：VSCode → 扩展 → `...` → 从 VSIX 安装，选择下载的文件
- 说明：扩展目录内含 `README.md`（中英双语）与 `README.en.md`（英文）

> **本版为重新打包**：修掉了「在 VS Code 1.13x 上扩展**根本无法激活**」的致命问题。若你装过
> 更早的 0.4.2，请**重新安装本文件**。装完可用右键「SML: 自检」核对包内解析器指纹
> （应为 `96110 B / c7a5054d…`）。
>
> 扩展未上架 Marketplace，仅提供 VSIX 手动安装（Zed 用户请看仓库里的 `editors/zed/README.md`）。
> 契约（`@contract` / `@is`）在 Rust、JS、C、C++、Lua 上都已支持，因此**编辑器内的语义错误也会报**。

</details>

<details class="feature"><summary>多语言宿主绑定 · 0.1.0（Python / Ruby / Java / Go / C#）</summary>

{{% featurelist %}}

> 五门宿主绑定经 C-ABI 调 Rust cdylib；读路径直接遍历值树（类型保真、富错误直取），不经 JSON；
> 仅 `dumps` 走官方桥 `对象 → JSON → sml_dump`。C# 本机无 .NET SDK **尚未实地验证**，逻辑与其余四门
> 严格对齐，待装 SDK 后 `dotnet run --project bindings/csharp` 即可验证。

</details>

<details class="feature"><summary>参考实现 · 0.6.3（C / C++ / JavaScript / Lua）</summary>

{{% featurelist %}}

> C / C++ 为仓库内零依赖自包含实现（亦可选 Rust 桥接头 `sml_rs.*`）；JavaScript 为零依赖 ESM，
> 与官网 playground 同解析器；Lua 为随仓库提供的参考绑定。四者与宿主绑定共享同一套能力面。

</details>

<details class="feature"><summary>Rust 库与命令行工具（swsml 0.6.3 · smltools 0.2.2）</summary>

- **Rust 库**：[`swsml` 0.6.3](https://crates.io/crates/swsml) —— `cargo add swsml`
- **命令行工具**：[`smltools` 0.2.2](https://crates.io/crates/smltools) —— `cargo install smltools`
  （版本号与库**互相独立**；`smltools` 由 `smlconv` 改名而来，旧的 `smlconv` 不再更新）

> **0.6.3 的要点**：契约叠加（一个块应用多份契约，三写法等价）；契约映射 API；契约推断
> `smltools --infer-contract`；错误带位置 `[第 N 行第 M 列]`；修复模式量词与编译规模无上限等五处。
> 完整清单见 [CHANGELOG.md](https://gitee.com/snoware/sml/blob/main/CHANGELOG.md)
> （[English](https://gitee.com/snoware/sml/blob/main/CHANGELOG.en.md)）。

</details>
