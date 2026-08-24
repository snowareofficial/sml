---
title: "BamZap — 包管理器"
---

# BamZap { ❄ }

Soup 伴生孵化项目：**内容寻址对象池 + 后量子签名（soupq）+ HTTPS 分发 + 声明式部署（HetuFile）**。

半独立于 Soup：一个 `bamzap.sar` 即完整客户端，仅需 `soupx`（+ `soup55b.dll`）运行，soupmake 构建。

## 快速开始

```bash
soupx bamzap.sar keygen myorg
soupx bamzap.sar trust add <fp> "myorg release key"
export SOUPKG_SIGN_ID=myorg
soupx bamzap.sar pool register myapp ./myapp/
soupx bamzap.sar verify myapp
soupx bamzap.sar bare run myapp Alice
```

## 能力

- 内容寻址对象池（sha256，跨版本去重）
- content-defined chunking（平均 ~1MiB，断点下载）
- 后量子签名 soupq（SPHINCS+ 风格，底层 SM3）
- deny-by-default 信任库
- HTTPS 对象空间分发
- HetuFile.sml 声明式部署 + LanTuFile.sml 构建配置

## SML 关联

- **HetuFile.sml**：声明式部署文件用 SML 书写
- **LanTuFile.sml**：soupmake 构建配置可用 SML 书写（与 Soupfile 等价）
