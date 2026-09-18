# 解析样例（查 ERROR 用）

这里放的是**给人看的语法样例**，配合 `tree-sitter parse` 检查 grammar 是否会把
SML 切出 `ERROR` 节点：

```bash
cd editors/zed/grammars/sml
tree-sitter generate
tree-sitter parse test/parse/*.sml      # 输出里不该出现 ERROR
```

## 为什么不是标准的 `test/corpus/*.txt`

Tree-sitter 的 `tree-sitter test` 需要语料文件里**手写期望语法树**，那是「先跑一遍
拿真实输出，再粘回去」的活。本仓库当前环境**没有 tree-sitter CLI**（安装 tree-sitter-cli
需要联网下载），所以这里不提交任何手推的期望树 —— 免得提交一份看起来通过、其实对不上的
测试。首次跑到 `tree-sitter generate` 之后，用

```bash
tree-sitter parse test/parse/*.sml
```

的输出把 `test/corpus/` 补齐即可（这一步刻意留给有 CLI 的环境）。

## 样例清单

| 文件 | 覆盖点 |
|---|---|
| `basic.sml` | 键值、对象块、数组、注释、引号字符串、数字与布尔 |
| `advanced.sml` | 片段与 `&引用`、`$env.`、指令与契约、匿名与裸词、块冒号可省 |
