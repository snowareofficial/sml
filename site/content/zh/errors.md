---
title: "错误码总表"
translationKey: "errors"
---

# 错误码总表

SML 的每类错误都有**稳定的机器可读码**，形如 `E-CONTRACT-002`：

- 第 1 段是**级别**：`E` 错误 / `W` 告警 / `I` 提示
- 第 2 段是**领域**，共 14 个，按层分：语言层（词法、语法、契约、include、上限、特性、扩展点）、
  宿主绑定层（IO、内部、派生桥）、工具层（迁入格式、命令行、lint）、编辑器层（编辑器宿主）
- 第 3 段是**序号**，三位、只增不改；删掉的码留空位，不回收

**码是稳定契约，文案不是。** 各语言实现的报错措辞可以不同（甚至不同语言），只要码相同就是同一件事：
用码去判断、去检索、去写文档；不要用文案去匹配。

> 权威数据源是仓库里的 [`errors/codes.sml`](https://gitee.com/snoware/sml/blob/master/errors/codes.sml)，
> 本页的表格由 `python errors/gen_json.py` 从它生成 —— 也就是说**这一页与代码引用的是同一份事实**，
> 不会出现「文档里有的码代码里没有」。

<div id="errors-app">
  <p>正在加载错误码表……（若长时间无内容，说明 <code>errors.json</code> 未生成：
  先跑 <code>python errors/gen_json.py</code>）</p>
</div>

## 怎么用

```text
E-CONTRACT-002   字段类型不符
E-PARSE-008      顶层标量不可往返
E-LIMIT-001      嵌套过深
```

- 写脚本判断错误类别时，匹配**前两段**往往比匹配整码更合适（如所有契约错误都是 `E-CONTRACT-*`）
- 报 bug 时请带上码 + 最小复现，比贴一大段文案有用得多
- 想加新码？改 `errors/codes.sml`，序号只增；文案可以随便改，码不行
