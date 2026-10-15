---
title: "Download"
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

# Download SML { ❄ }

All downloads are released under the [MulanPSL-2.0](/LICENSE) license; source is on
[the repo](https://gitee.com/snoware/sml). Every language binding calls the Rust cdylib `sml`/`libsml`
through the C-ABI (`c/sml_rs.h` v3); the read path traverses the value tree directly, with no JSON
round-trip; source packages ship with `LICENSE` and `README`, ready to embed into your project.

## Download

<table class="dl-table">
<thead><tr><th>Item</th><th>Version</th><th>Notes</th><th>Download</th></tr></thead>
<tbody>
<tr>
<td><b>VS Code Extension</b></td><td><code>0.4.3</code></td>
<td>Editor plugin: highlighting / diagnostics / completion / jump / formatting + SML-specific editing</td>
<td><a href="/dl/sml-lang-0.4.3.vsix" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>Python</b></td><td><code>0.1.0</code></td>
<td>ctypes binding to the cdylib, zero third-party deps; value-tree traversal</td>
<td><a href="/dl/sml-python-0.1.0.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>Ruby</b></td><td><code>0.1.0</code></td>
<td>fiddle binding to the cdylib, zero gem deps; value-tree traversal</td>
<td><a href="/dl/sml-ruby-0.1.0.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>Java</b></td><td><code>0.1.0</code></td>
<td>JDK Panama FFM binding to the cdylib; value-tree traversal</td>
<td><a href="/dl/sml-java-0.1.0.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>Go</b></td><td><code>0.1.0</code></td>
<td>cgo binding to the cdylib; value-tree traversal</td>
<td><a href="/dl/sml-go-0.1.0.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>C#</b></td><td><code>0.1.0</code></td>
<td>P/Invoke binding to the cdylib (not verified locally)</td>
<td><a href="/dl/sml-csharp-0.1.0.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>C</b></td><td><code>0.6.3</code></td>
<td>Pure C99 self-contained impl, or the Rust bridge header — pick one</td>
<td><a href="/dl/sml-c-0.6.3.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>C++</b></td><td><code>0.6.3</code></td>
<td>C++17 zero-dependency impl, or the Rust bridge header — pick one</td>
<td><a href="/dl/sml-cpp-0.6.3.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>JavaScript</b></td><td><code>0.6.3</code></td>
<td>Zero-dependency ESM, same parser as the playground</td>
<td><a href="/dl/sml-javascript-0.6.3.zip" class="dl-btn">Download</a></td>
</tr>
<tr>
<td><b>Lua</b></td><td><code>0.6.3</code></td>
<td>Lua binding, shipped as an in-repo reference implementation</td>
<td><a href="/dl/sml-lua-0.6.3.zip" class="dl-btn">Download</a></td>
</tr>
</tbody>
</table>

## Debian / Ubuntu packages (libswsml · amd64)

System packages for the Rust cdylib backend (`swsml` 0.6.3, SONAME `libswsml.so.0`). Install and go: the shared library is registered with `ldconfig`, headers land in `/usr/include/sml`, and `pkg-config` is wired up. Split by backend into four packages:

<table class="dl-table">
<thead><tr><th>Package</th><th>Version</th><th>Notes</th><th>Download</th></tr></thead>
<tbody>
<tr><td><b>libswsml0</b></td><td><code>0.6.3</code></td><td>Runtime: Rust cdylib C-ABI (<code>libswsml.so.0</code>), full v1..v4 feature set</td><td><a href="/dl/libswsml0_0.6.3_amd64.deb" class="dl-btn">Download</a></td></tr>
<tr><td><b>libswsml-dev</b></td><td><code>0.6.3</code></td><td>C development: <code>sml_rs.h</code> + <code>libswsml.so</code> + <code>pkg-config swsml</code></td><td><a href="/dl/libswsml-dev_0.6.3_amd64.deb" class="dl-btn">Download</a></td></tr>
<tr><td><b>libswsml-cpp0</b></td><td><code>0.6.3</code></td><td>C++ runtime: RAII wrapper (<code>libswsml_cpp.so.0</code>)</td><td><a href="/dl/libswsml-cpp0_0.6.3_amd64.deb" class="dl-btn">Download</a></td></tr>
<tr><td><b>libswsml-cpp-dev</b></td><td><code>0.6.3</code></td><td>C++ development: <code>sml_rs.hpp</code> + <code>pkg-config swsml_cpp</code></td><td><a href="/dl/libswsml-cpp-dev_0.6.3_amd64.deb" class="dl-btn">Download</a></td></tr>
</tbody>
</table>

```sh
# runtime
sudo dpkg -i libswsml0_0.6.3_amd64.deb libswsml-cpp0_0.6.3_amd64.deb
# development (headers + pkg-config)
sudo dpkg -i libswsml-dev_0.6.3_amd64.deb libswsml-cpp-dev_0.6.3_amd64.deb
# then: gcc app.c $(pkg-config --cflags --libs swsml) -o app
#       g++ -std=c++17 app.cpp $(pkg-config --cflags --libs swsml_cpp) -o app
```

> **amd64 only** for now; build other architectures with the in-repo `packaging/build_deb.sh --arch <arch>`.
> The name deliberately uses `libswsml` to avoid colliding with Debian's official `libSML` / `libsml-dev` (smart-meter ecosystem).
> Depends: `libc6`, `libgcc-s1` (C++ packages also need `libstdc++6`).

## Checksums (SHA256)

Verify after downloading. The block below is `sha256sum -c`-compatible — save it as `SHA256SUMS` next to the downloaded files and run `sha256sum -c SHA256SUMS` to check them all at once:

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

## Implementation Notes (click to expand)

<details class="feature"><summary>VS Code Extension · 0.4.3</summary>

Provides VSCode with SML syntax highlighting, diagnostics, completion, hover, jump and formatting, plus
edit-time capabilities "that only make sense in SML":

- **Hover**: a contract name (`@is Server`) shows the contract declaration + the **actual structure with
  defaults filled by the parser**; a block name (`primary {`) shows its **path** (`database.primary`),
  the contract it applies and the filled structure
- **Go to definition**: `@is Server` → `@contract Server`, `&base` → `@base { }`
- **include / namespace navigation**: F12 on a path jumps to the included file, hover shows "top-level
  keys of that file"; a namespace alias (`as db`) hover shows "which keys this namespace actually has",
  F12 jumps to the included file; a reference (`db.host`) hover tells you which file/definition it comes
  from and F12 jumps there
- **Special colors**: select a word → right-click "apply special color", written into the workspace
  `HL-cfg.sml` (travels with the repo, hand-editable); by default it only affects **syntax units**
  (contract / fragment / type / key / directive), not same-named text inside comments or strings; it also
  honors **namespace prefixes** (`@is ui.base.Service` references are colored too)
- **Spotlight**: light up a word across the **whole workspace** (status bar shows N places / M files)
- **Self-check**: right-click "SML: Self-check" to expand "why nothing happens" line by line into
  "Output → SML" (extension version, **in-package parser fingerprint**, command registration,
  language mode, document validation result, whether jump works)

- Install: VSCode → Extensions → `...` → Install from VSIX, pick the downloaded file
- Note: the extension folder contains `README.md` (bilingual) and `README.en.md` (English)

> **This build is a repackage**: it fixes a fatal bug where the extension **failed to activate at all**
> on VS Code 1.13x. If you installed an earlier 0.4.2, please **reinstall this file**. After install,
> use right-click "SML: Self-check" to verify the in-package parser fingerprint (should be
> `96110 B / c7a5054d…`).
>
> The extension is not on the Marketplace; only manual VSIX install is provided (Zed users, see
> `editors/zed/README.md` in the repo). Contracts (`@contract` / `@is`) are supported on Rust, JS, C,
> C++ and Lua, so **semantic errors inside the editor are also reported**.

</details>

<details class="feature"><summary>Host Bindings · 0.1.0 (Python / Ruby / Java / Go / C#)</summary>

{{% featurelist %}}

> The five host bindings call the Rust cdylib through the C-ABI; the read path traverses the value tree
> directly (type-preserving, rich errors in hand), with no JSON round-trip; only `dumps` goes through the
> official bridge `object → JSON → sml_dump`. C# is **not verified locally** (no .NET SDK on this machine);
> its logic is strictly aligned with the other four and can be verified via `dotnet run --project bindings/csharp`
> once an SDK is installed.

</details>

<details class="feature"><summary>Reference Implementations · 0.6.3 (C / C++ / JavaScript / Lua)</summary>

{{% featurelist %}}

> C / C++ are zero-dependency self-contained implementations in the repo (or pick the Rust bridge header
> `sml_rs.*`); JavaScript is a zero-dependency ESM, same parser as the playground; Lua is an in-repo
> reference binding. All four share the same capability surface as the host bindings.

</details>

<details class="feature"><summary>Rust Library & CLI (swsml 0.6.3 · smltools 0.2.2)</summary>

- **Rust library**: [`swsml` 0.6.3](https://crates.io/crates/swsml) —— `cargo add swsml`
- **CLI**: [`smltools` 0.2.2](https://crates.io/crates/smltools) —— `cargo install smltools`
  (version independent of the library; `smltools` was renamed from `smlconv`, which is no longer updated)

> **0.6.3 highlights**: contract superposition (one block applies multiple contracts, three equivalent
> spellings); contract-mapping API; contract inference `smltools --infer-contract`; errors carry location
> `[line N, col M]`; five fixes including pattern-quantifier / compile-scale bounds. Full list in
> [CHANGELOG.md](https://gitee.com/snoware/sml/blob/main/CHANGELOG.md)
> ([English](https://gitee.com/snoware/sml/blob/main/CHANGELOG.en.md)).

</details>
