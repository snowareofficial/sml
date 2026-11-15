/* 以下 8 个模块按「前置/工具 → 词法 → 契约 → 特性 → 裸块/弱绑定 → 解析 → include/API → 序列化」
   顺序 include —— unity 构建，预处理输出与拆分前逐行相同。见 cpp/src/README.md。 */
#include "src/00_prelude_util.inc"
#include "src/01_lex.inc"
// 模式匹配器（`@type` 的类型校验用）：`TypeSpec::Pattern` 要引用它，故排在契约之前。
#include "src/02b_pattern.inc"
#include "src/02_coerce_type.inc"
#include "src/03_feature_scan.inc"
#include "src/04_weak_bare.inc"
#include "src/05_parse.inc"
#include "src/06_include_api.inc"
#include "src/07_dump.inc"
