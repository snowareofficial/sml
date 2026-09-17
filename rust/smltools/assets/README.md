# 资产目录

## `baseline.tmLanguage.json`

编辑器高亮的**原版基线**，由 `src/highlight.rs` 用 `include_str!` 编译期内嵌。

- **唯一权威来源**：`editors/vscode/syntaxes/sml.tmLanguage.json`
- **同步方式**：改完权威来源后，把它复制到这里（命令见下）
- **为什么留副本而不是直接读文件**：`smltools` 是独立发布的二进制，运行时未必能在
  用户机器上找到仓库路径；内嵌才能「一个可执行文件自带基线」。
- **为什么不抄成 Rust 常量**：抄写会得到一份编译器不会检查的影子副本，原版更新后
  会静默漂移。内嵌真实 JSON 至少保证「复制那一刻是逐字一致的」，且锚点名是从基线
  **动态读取**的（见 `highlight.rs`），基线增删规则时可用锚点自动跟着变。

```bash
# 同步基线（在仓库根执行）
cp editors/vscode/syntaxes/sml.tmLanguage.json rust/smltools/assets/baseline.tmLanguage.json
```

用 SML 定制高亮的用法：

```bash
smltools --from sml --to tmlanguage -i my-highlight.sml -o syntaxes/sml.tmLanguage.json
```
