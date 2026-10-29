# SML 项目贡献指南

> 欢迎参与 SML（Structured Markup Language）多语言实现项目！
> 这是一个设计简洁、解析高效的数据交换格式，支持 Rust/C/C++/JS/Lua 五端实现。

## 🚀 快速上手（3 分钟）

### 什么是 SML？

SML 是一种**人类可读、机器可解析**的数据格式，语法类似 JSON/YAML/TOML，但更简洁：

```sml
@version v1

server {
  host: localhost
  port: 8080
  timeout: 30.5
    
  certificates [
    cert { path: "cert.pem" },
    cert { path: "key.pem" }
  ]
}
```

### 为什么不用 JSON/YAML/TOML？

| 特性 | SML | JSON | YAML | TOML |
|------|-----|------|------|------|
| **注释** | `# 单行` / `/* 多行 */` | ❌ | ✅ | ✅ |
| **字符串转义** | `\u{1F680}` 表情支持 | ✅ | ✅ | ✅ |
| **片段复用** | `&base` 引用定义 | ❌ | ❌ | ❌ |
| **条件编译** | `@when enable-tls` | ❌ | ❌ | ❌ |
| **契约约束** | `@is Server` 类型校验 | ❌ | ❌ | ❌ |
| **跨平台实现** | 五端一致性 | ✅ | ⚠️ | ⚠️ |

## 📋 从哪入手？

### 🐛 新手友好任务

这些标签标记的任务适合初次贡献：

- `good-first-issue` - 文档修复、简单测试用例
- `documentation` - 改进现有文档
- `test` - 补充测试覆盖率

### 🔧 进阶任务

- `help-wanted` - 需要额外上下文的功能开发
- `bug` - bug 修复
- `enhancement` - 新特性实现

### 💡 核心开发

- `research` - 架构讨论与方案设计
- `rfc` - 重大变更提案
- `performance` - 性能优化

## 🛠️ 环境设置

### 必需工具

- **Rust** (v1.70+) - 主实现
- **C99 编译器** (gcc/clang) - C 实现
- **C++17 编译器** - C++ 实现  
- **Node.js** (v18+) - JS 实现
- **LuaJIT** 或 Lua 5.1+ - Lua 实现
- **Python 3.8+** - 构建脚本

### 克隆仓库

```bash
git clone https://github.com/swsml/sml.git
cd sml
```

### 验证环境

```bash
# 运行所有检查
python tools/build_check.py --run

# 各端独立检查
python rust/run_check.py
python c/build_check.py
python cpp/build_verify.py
node js/probe-error-codes.mjs
luajit lua/run_check.lua
```

## 🤝 提交贡献

### 开发流程

1. **创建 Issue** - 讨论设计或报告 Bug
2. **Fork 仓库** - 在你的账号下 fork
3. **创建分支** - `git checkout -b feature/description`
4. **编写代码** - 遵循下方编码规范
5. **添加测试** - 覆盖新行为
6. **运行检查** - 确保五端全部通过
7. **提交 PR** - 描述改动与动机

### 分支命名规范

- `feature/description` - 新功能
- `fix/description` - bug 修复
- `docs/description` - 文档更新
- `test/description` - 测试相关
- `refactor/description` - 代码重构

### Commit 消息格式

```
<类型>: <简短描述>

[可选] 详细描述，包括：
- 问题背景
- 解决方案
- 影响范围

[可选] 关联 Issue: #123
```

**类型**：`feat`, `fix`, `docs`, `style`, `refactor`, `test`, `chore`

## 🧪 测试规范

### 必须包含

1. **单元测试** - 针对功能点
2. **跨实现一致性测试** - 五端统一行为
3. **回归测试** - 防止旧行为破坏

### 添加新测试

```rust
// Rust 示例
#[test]
fn test_feature_behavior() {
    let input = "...";
    let expected = ...;
    
    // 在 Rust 中验证
    assert_eq!(parse(input), expected);
    
    // 其他四端需有同等测试
    // (见 tests/conformance.{sml, json})
}
```

## 📖 编码规范

### Rust

- 遵循 [Rust 官方风格指南](https://doc.rust-lang.org/style-guide/)
- 零警告：`cargo clippy` 必须全绿
- 格式化：`cargo fmt`

### C/C++

- C99 / C++17 标准
- 无未初始化变量
- 边界检查完整

### JavaScript/TypeScript

- ES2020+
- 严格模式：`'use strict'`
- ESLint 规则通过

### Lua

- Lua 5.1 / LuaJIT 兼容
- 避免全局变量
- 错误处理完整

## 🔍 审查流程

1. **CI 检查** - 自动跑通所有测试
2. **代码审查** - 至少 1 位维护者批准
3. **讨论期** - 开放 24 小时反馈
4. **合并** - Squash Merge 到主分支

### 审查要点

- ✅ 功能正确性
- ✅ 五端一致性
- ✅ 测试覆盖率
- ✅ 文档完整性
- ✅ 无安全隐患

## 🆘 获取帮助

- **技术问题** - 查看 [`docs/`](docs/) 目录
- **即时交流** - GitHub Discussions
- **Bug 报告** - 提供最小复现示例

## 📄 许可证

本项目采用 MIT 许可证。提交贡献即表示你同意：

- 代码遵循 MIT 协议
- 拥有代码的知识产权或已获得授权
- 允许商业使用与分发

---

**感谢你的贡献！** 🎉

SML 项目的成长依赖社区力量。无论是小修正还是大功能，每一份努力都值得赞赏。

如果有任何不清楚的地方，先开一个 Discussion 聊聊，我们很乐意帮助你！
