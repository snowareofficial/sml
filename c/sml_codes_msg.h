/* SPDX-License-Identifier: MulanPSL-2.0 */
/* 本文件由 errors/gen_js_errors.py 生成，请勿手改。
 * 中文（默认/源语言）来自 site/static/errors.json；英文来自 errors/msg.en.json。
 * 某条码/字段缺英文时回退中文，永不返回空。
 *
 * 用法：
 *   #include "sml_codes_msg.h"
 *   const char *t = sml_error_title("E-PARSE-006", "en");  // "Expected key or identifier"
 *   const char *m = sml_error_msg("E-PARSE-006", NULL);   // 中文 msg（locale 为 NULL/"" = 中文）
 *
 * 未知码 / NULL 入参 -> 返回 NULL。locale 以 "en" 开头 -> 英文，否则中文。
 * C++ 直接 include 本头即可（header-only，不另造）。
 */
#ifndef SML_CODES_MSG_H
#define SML_CODES_MSG_H

#include <string.h>

struct SmlErrorMsg {
    const char *code;
    const char *severity;
    const char *domain;
    const char *zh_title;
    const char *zh_msg;
    const char *en_title; /* 缺英文为 NULL */
    const char *en_msg;
};

static const struct SmlErrorMsg SML_ERROR_MSGS[] = {
    { "E-CLI-001", "error", "CLI", "输入格式未知", "输入格式取值未知", "Unknown input format", "The input format value is unknown" },
    { "E-CLI-002", "error", "CLI", "输出格式未知", "输出格式取值未知", "Unknown output format", "The output format value is unknown" },
    { "E-CLI-003", "error", "CLI", "参数组合非法", "命令行参数组合非法：互斥项同时给出，或依赖项缺失", "Invalid argument combination", "Invalid command-line argument combination: mutually exclusive options given together, or a required dependency is missing" },
    { "E-CLI-004", "error", "CLI", "lint 只适用于 SML 文档", "lint 只能检查 SML 文档", "lint applies to SML documents only", "lint can only check SML documents" },
    { "E-CLI-005", "error", "CLI", "目录模式必须指定输出目录", "目录批量模式必须指定输出目录", "Directory mode requires an output directory", "Directory batch mode requires an output directory" },
    { "E-CLI-006", "error", "CLI", "规则文档解析或构建失败", "自定义生成规则文档解析或构建失败", "Rule document parse or build failed", "Failed to parse or build the custom generation rule document" },
    { "E-CLI-007", "error", "CLI", "输出后端报错", "输出后端报错（内层原因见原始错误）", "Output backend error", "Output backend error (see the original error for the inner cause)" },
    { "E-CLI-008", "error", "CLI", "命令行用法错误", "命令行用法错误：未知参数、缺少取值或取值非法", "Command-line usage error", "Command-line usage error: unknown argument, missing value, or invalid value" },
    { "E-CONTRACT-001", "error", "CONTRACT", "未定义的契约", "引用了未定义的契约", "Undefined contract", "Referenced an undefined contract" },
    { "E-CONTRACT-002", "error", "CONTRACT", "字段类型不符", "字段类型应为期望类型，实际为其它类型", "Field type mismatch", "Field type should be the expected type, but is another type" },
    { "E-CONTRACT-003", "error", "CONTRACT", "必填字段缺失", "字段必填但缺失", "Required field missing", "The field is required but missing" },
    { "E-CONTRACT-004", "error", "CONTRACT", "未声明字段（严格模式）", "字段未在契约中声明；确需放宽请在契约名后写 loose", "Undeclared field (strict mode)", "Field is not declared in the contract; to allow extra fields, write loose after the contract name" },
    { "E-CONTRACT-005", "error", "CONTRACT", "数值越界", "字段值小于下界或大于上界", "Value out of range", "Field value is below the lower bound or above the upper bound" },
    { "E-CONTRACT-006", "error", "CONTRACT", "枚举取值非法", "取值不在枚举列表内", "Invalid enum value", "The value is not in the enum list" },
    { "E-CONTRACT-007", "error", "CONTRACT", "外置类型校验失败", "字段不符合扩展类型的要求", "External type check failed", "The field does not satisfy the extension type's requirements" },
    { "E-CONTRACT-008", "error", "CONTRACT", "组合字段应为块", "字段应为块并按该契约校验，实际不是块", "Composite field should be a block", "The field should be a block validated against that contract, but is not a block" },
    { "E-CONTRACT-009", "error", "CONTRACT", "自定义类型格式不符", "字段的值不符合该类型的格式要求", "Custom type format mismatch", "The field value does not match the type's format requirements" },
    { "E-CONTRACT-010", "error", "CONTRACT", "数值约束取值为非有限数", "字段的值为非有限数，不能作为数值约束的取值", "Numeric constraint value is non-finite", "The field value is non-finite and cannot be used as a numeric constraint" },
    { "E-CONTRACT-011", "error", "CONTRACT", "外置修饰符校验失败", "字段不符合扩展修饰符的要求", "External modifier check failed", "The field does not satisfy the extension modifier's requirements" },
    { "E-CONTRACT-012", "error", "CONTRACT", "未知数组元素类型", "数组元素类型名未知，且不是已注册的扩展类型", "Unknown array element type", "The array element type name is unknown and is not a registered extension type" },
    { "E-CONTRACT-013", "error", "CONTRACT", "模式定义非法", "模式定义非法：未知字符类、未知量词、不支持的元素，或无法识别的元素", "Invalid pattern definition", "Invalid pattern definition: unknown character class, unknown quantifier, unsupported element, or unrecognizable element" },
    { "E-CONTRACT-014", "error", "CONTRACT", "模式规则引用非法", "模式规则引用非法：引用了未定义的规则，或规则循环引用", "Invalid pattern rule reference", "Invalid pattern rule reference: references an undefined rule, or the rule references itself cyclically" },
    { "E-CONTRACT-015", "error", "CONTRACT", "量词取值非法", "量词的取值非法：不是整数、为负数，或上界小于下界", "Invalid quantifier value", "Invalid quantifier value: not an integer, negative, or the upper bound is less than the lower bound" },
    { "E-CONTRACT-016", "error", "CONTRACT", "契约叠加冲突", "同一字段在参与叠加的两份契约里类型、默认值或区间不一致", "Contract mixin conflict", "The same field has inconsistent type, default, or range across the two mixed-in contracts" },
    { "E-CONTRACT-017", "error", "CONTRACT", "契约叠加未实现", "该实现暂不支持一个块应用多份契约；请拆分成单契约或等待", "Contract mixin not implemented", "This implementation does not support applying several contracts to one block; split into single contracts or wait" },
    { "E-CONTRACT-018", "error", "CONTRACT", "契约 @requires 无法满足", "契约声明的 @requires 特性无法满足，契约无法定义或应用", NULL, NULL },
    { "E-DERIVE-001", "error", "DERIVE", "值与目标类型不符", "SML 值的类型与目标宿主类型不符", "Value/target type mismatch", "The SML value's type does not match the target host type" },
    { "E-DERIVE-002", "error", "DERIVE", "数值超出目标类型范围", "数值超出目标宿主类型的取值范围", "Number out of target type range", "The number is out of the target host type's range" },
    { "E-DERIVE-003", "error", "DERIVE", "未知的枚举值或变体", "未知的枚举值或枚举变体", "Unknown enum value or variant", "Unknown enum value or enum variant" },
    { "E-DERIVE-004", "error", "DERIVE", "枚举变体形态不符", "枚举变体的形态与目标定义不符", "Enum variant shape mismatch", "The enum variant's shape does not match the target definition" },
    { "E-DERIVE-005", "error", "DERIVE", "序列化时键不是字符串", "序列化时对象的键必须是字符串", "Non-string key during serialization", "Object keys must be strings during serialization" },
    { "E-DERIVE-006", "error", "DERIVE", "宿主特性未启用", "该能力需要启用对应的宿主构建特性", "Host feature not enabled", "This capability requires enabling the corresponding host build feature" },
    { "E-DERIVE-007", "error", "DERIVE", "派生属性或类型形状非法", "派生宏的属性或类型形状非法（编译期报错）", "Invalid derive attribute or type shape", "Invalid derive-macro attribute or type shape (compile-time error)" },
    { "E-DERIVE-008", "error", "DERIVE", "派生入口的解析失败", "派生反序列化时 SML 解析失败（内层原因见原始错误）", "Derive entry parse failed", "SML parsing failed during derive deserialization (see the original error for the inner cause)" },
    { "E-EDITOR-001", "error", "EDITOR", "解析器加载失败", "编辑器侧的解析器加载失败，补全与诊断不可用", "Parser load failed", "The editor-side parser failed to load; completion and diagnostics are unavailable" },
    { "E-EDITOR-002", "error", "EDITOR", "高亮配置加载失败", "高亮配置加载或解析失败", "Highlight config load failed", "Failed to load or parse the highlight configuration" },
    { "E-EDITOR-003", "error", "EDITOR", "无法格式化", "无法格式化：文档存在解析错误", "Cannot format", "Cannot format: the document has parse errors" },
    { "E-EDITOR-004", "error", "EDITOR", "未打开工作区", "请先打开一个工作区文件夹", "No workspace open", "Please open a workspace folder first" },
    { "E-EXT-001", "error", "EXT", "未注册的指令、类型或修饰符", "该指令、类型或修饰符未在本环境注册", "Unregistered directive, type, or modifier", "The directive, type, or modifier is not registered in this environment" },
    { "E-EXT-002", "error", "EXT", "扩展注册名冲突", "不可注册与内置同名的扩展；同名重复注册同样报错", "Extension registration name conflict", "Cannot register an extension with a built-in name; duplicate registration also errors" },
    { "E-EXT-003", "error", "EXT", "外置指令缺少取值", "外置指令的参数名后缺少取值", "External directive missing value", "The external directive's parameter name is missing a value" },
    { "E-EXT-004", "error", "EXT", "外置指令返回非对象", "外置指令的合并结果须为对象", "External directive returned non-object", "The external directive's merge result must be an object" },
    { "E-EXT-005", "error", "EXT", "外置修饰符缺少取值", "外置修饰符期望取值", "External modifier missing value", "The external modifier expects a value" },
    { "E-EXT-006", "error", "EXT", "custom 规则文档非法", "自定义生成规则文档非法：缺少 rules、rules 为空，或某条规则缺少模板", "Invalid custom rule document", "Invalid custom generation rule document: missing rules, empty rules, or a rule missing its template" },
    { "E-EXT-007", "error", "EXT", "编辑器定制文档非法", "编辑器定制文档非法：作用域名、规则锚点、颜色取值不符规范", "Invalid editor customization document", "Invalid editor customization document: scope name, rule anchor, or color value does not follow the spec" },
    { "E-EXT-008", "error", "EXT", "外置指令执行失败", "外置指令处理该输入时失败", "External directive execution failed", "The external directive failed while processing this input" },
    { "E-FEATURE-001", "error", "FEATURE", "特性未启用", "该语法需要相应特性，请先启用该特性", "Feature not enabled", "This syntax requires the corresponding feature; enable it first" },
    { "E-FEATURE-002", "error", "FEATURE", "环境变量被禁用", "当前特性集禁用了环境变量内联，裸词或字符串无法解析", "Environment variables disabled", "The current feature set disables environment-variable inlining; bare words or strings cannot be parsed" },
    { "E-FEATURE-003", "error", "FEATURE", "未知特性名", "未知特性名", "Unknown feature name", "Unknown feature name" },
    { "E-FEATURE-004", "error", "FEATURE", "版本声明非法", "不支持的版本声明，或版本声明互相冲突；当前支持 v1..v4", "Invalid version declaration", "Unsupported version declaration, or conflicting version declarations" },
    { "E-FEATURE-005", "error", "FEATURE", "裸词必须加引号", "字符串必须加引号：当前特性集禁用了裸词字符串", "Bare word must be quoted", "Strings must be quoted: the current feature set disables bare-word strings" },
    { "E-FEATURE-006", "error", "FEATURE", "feature 子命令未知", "未知的 `@feature` 子命令", "Unknown feature subcommand", "Unknown `@feature` subcommand" },
    { "E-FEATURE-007", "error", "FEATURE", "feature 指令缺少参数", "`@feature` 指令缺少参数", "feature directive missing argument", "The `@feature` directive is missing an argument" },
    { "E-FEATURE-008", "error", "FEATURE", "feature mode 参数非法", "`@feature mode` 的参数须为白名单或黑名单之一", "Invalid feature mode argument", "The `@feature mode` argument must be either whitelist or blacklist" },
    { "E-FEATURE-009", "error", "FEATURE", "请求特性与允许集无交集", "文档请求的特性与调用方允许的特性没有交集", "No overlap with allowed feature set", "The document's requested features have no overlap with the caller's allowed set" },
    { "E-INCLUDE-001", "error", "INCLUDE", "include 文件缺失或读取失败", "include 无法定位或读取目标文件", "include file missing or read failed", "include cannot locate or read the target file" },
    { "E-INCLUDE-002", "error", "INCLUDE", "include 循环引用", "include 循环引用", "include circular reference", "include circular reference" },
    { "E-INCLUDE-003", "error", "INCLUDE", "include 越界拒绝", "include 目标不在基准目录内，已拒绝", "include out-of-bounds rejected", "The include target is outside the base directory; rejected" },
    { "E-INCLUDE-004", "error", "INCLUDE", "include 嵌套超过上限", "include 嵌套超过上限层数", "include nesting exceeds limit", "include nesting exceeds the maximum depth" },
    { "E-INCLUDE-005", "error", "INCLUDE", "键列表语法非法", "键列表语法非法：期望键列表、或键列表为空、或缺少闭合", "Invalid key-list syntax", "Invalid key-list syntax: expected a key list, or the list is empty, or missing closing" },
    { "E-INCLUDE-006", "error", "INCLUDE", "未定义的片段引用", "未定义的片段引用", "Undefined fragment reference", "Undefined fragment reference" },
    { "E-INCLUDE-007", "error", "INCLUDE", "片段展开结果不是对象", "片段展开结果不是对象，无法与所在块合并", "Fragment expansion is not an object", "The fragment expansion result is not an object and cannot be merged into the block" },
    { "E-INCLUDE-008", "error", "INCLUDE", "部分引用缺少目标文件", "部分引用的写法必须接上目标文件", "Partial reference missing target file", "A partial reference must be followed by a target file" },
    { "E-INCLUDE-009", "error", "INCLUDE", "部分引用不能配通配", "部分引用不能配合 glob 或 regex 通配，请指定单个文件", "Partial reference cannot use wildcard", "A partial reference cannot be combined with glob or regex wildcards; specify a single file" },
    { "E-INCLUDE-010", "error", "INCLUDE", "基准目录不可解析", "include 基准目录不可解析，无法做越界校验，已拒绝继续", "Base directory unresolvable", "The include base directory cannot be resolved; the out-of-bounds check is impossible, so parsing stops" },
    { "E-INCLUDE-011", "error", "INCLUDE", "include 预处理词法失败", "include 预处理阶段的词法失败", "include preprocessing lex failed", "Lexing failed during include preprocessing" },
    { "E-INCLUDE-012", "error", "INCLUDE", "include 路径写法非法", "include 路径写法非法（未加引号或含非法字符）", "Invalid include path syntax", "Invalid include path syntax (unquoted or containing illegal characters)" },
    { "E-INTERNAL-001", "error", "INTERNAL", "内部错误", "内部错误：走到了不应到达的分支", "Internal error", "Internal error: reached a branch that should be unreachable" },
    { "E-INTERNAL-002", "error", "INTERNAL", "内置资源损坏", "内置资源损坏（打包或构建事故）", "Built-in resource corrupted", "Built-in resource corrupted (packaging or build accident)" },
    { "E-IO-001", "error", "IO", "读取失败", "读取文件失败", "Read failed", "Failed to read the file" },
    { "E-IO-002", "error", "IO", "输入为空", "输入为空", "Empty input", "Input is empty" },
    { "E-IO-003", "error", "IO", "写入或建目录失败", "写入文件或创建目录失败", "Write or mkdir failed", "Failed to write the file or create the directory" },
    { "E-IO-004", "error", "IO", "读取目录失败", "读取目录失败", "Directory read failed", "Failed to read the directory" },
    { "E-IO-005", "error", "IO", "未检测到输入", "未检测到输入：既没有指定输入文件，也没有可读的管道输入", "No input detected", "No input detected: neither an input file nor readable pipe input was given" },
    { "E-IO-006", "error", "IO", "标准输入读取失败", "读取标准输入失败或通道中断", "Standard input read failed", "Failed to read standard input, or the channel broke" },
    { "E-IO-007", "error", "IO", "外部工具不可用或失败", "外部工具不可用（未安装或无法启动），或其构建失败", "External tool unavailable or failed", "The external tool is unavailable (not installed or cannot start), or its build failed" },
    { "E-LEX-001", "error", "LEX", "字符串未闭合", "字符串未闭合（缺少结束引号）", "Unterminated string", "Unterminated string (missing closing quote)" },
    { "E-LEX-002", "error", "LEX", "未闭合块注释（斜杠星号）", "未闭合的块注释，遇到文件结尾", "Unterminated block comment (slash-star)", "Unterminated block comment; reached end of file" },
    { "E-LEX-003", "error", "LEX", "未闭合块注释（下划线星号）", "未闭合的块注释，遇到文件结尾", "Unterminated block comment (underscore-star)", "Unterminated block comment; reached end of file" },
    { "E-LEX-004", "error", "LEX", "字符串含未知转义符", "字符串含未知转义符，转义集见规范", "Unknown escape in string", "Unknown escape sequence in string; see the spec for the escape set" },
    { "E-LEX-005", "error", "LEX", "Unicode 转义非法", "Unicode 转义非法（位数不足、非十六进制、或非法码点）", "Invalid Unicode escape", "Invalid Unicode escape (too few digits, non-hexadecimal, or illegal code point)" },
    { "E-LEX-006", "error", "LEX", "转义符后遇文件结束", "字符串中的转义符后遇到文件结束", "Escape before end of file", "An escape sequence is followed by end of file inside the string" },
    { "E-LIMIT-001", "error", "LIMIT", "嵌套过深", "嵌套过深，超过本实现的上限层数，疑似递归或恶意输入", "Nesting too deep", "Nesting exceeds this implementation's depth limit; suspected recursion or malicious input" },
    { "E-LIMIT-002", "error", "LIMIT", "模式匹配超步数预算", "模式匹配超出步数预算，疑似病态规则或超长输入", "Pattern match over step budget", "Pattern matching exceeded the step budget; suspected pathological rule or overlong input" },
    { "E-LIMIT-003", "error", "LIMIT", "include 展开次数超限", "include 展开次数超过上限，疑似指数膨胀", "include expansion over limit", "include expansions exceeded the limit; suspected exponential blow-up" },
    { "E-LIMIT-004", "error", "LIMIT", "输出递归深度超过上限", "递归深度超过上限（翻译后端）", "Output recursion depth over limit", "Recursion depth exceeds the limit (translation backend)" },
    { "E-LIMIT-005", "error", "LIMIT", "custom 输出长度超上限", "custom 生成器输出超过长度上限（模板存在放大）", "custom output length over limit", "custom generator output exceeds the length limit (the template amplifies)" },
    { "E-LIMIT-006", "error", "LIMIT", "custom 数组循环次数超上限", "custom 生成器数组超过循环上限", "custom array loop over limit", "custom generator array exceeds the loop limit" },
    { "E-LIMIT-007", "error", "LIMIT", "模式源码长度超上限", "模式（正则）源码超过长度上限，拒绝编译", "Pattern source length over limit", "The pattern (regex) source exceeds the length limit; compilation refused" },
    { "E-LIMIT-008", "error", "LIMIT", "待校验值长度超上限", "待校验字符串超过长度上限，拒绝校验", "Value-to-check length over limit", "The string to check exceeds the length limit; validation refused" },
    { "E-LIMIT-009", "error", "LIMIT", "量词取值超上限", "量词的重复次数超过上限", "Quantifier value over limit", "The quantifier's repeat count exceeds the limit" },
    { "E-LIMIT-010", "error", "LIMIT", "内存分配失败", "内存分配失败", "Memory allocation failed", "Memory allocation failed" },
    { "E-LINT-001", "error", "LINT", "缩进里出现制表符", "缩进里出现制表符：SML 缩进敏感，请统一用空格", "Tab in indentation", "A tab appears in indentation: SML is indentation-sensitive, use spaces consistently" },
    { "E-MIGRATE-001", "error", "MIGRATE", "顶层出现文本内容", "迁入文档在顶层出现了文本内容（只允许空白）", "Text content at top level", "The imported document has text content at the top level (only whitespace is allowed)" },
    { "E-MIGRATE-002", "error", "MIGRATE", "标签未闭合", "标签未闭合：文件在标签内提前结束", "Unclosed tag", "Unclosed tag: the file ends inside a tag" },
    { "E-MIGRATE-003", "error", "MIGRATE", "标签名为空", "标签名为空", "Empty tag name", "The tag name is empty" },
    { "E-MIGRATE-004", "error", "MIGRATE", "结束标签不匹配", "结束标签与开始标签不匹配，或结束标签缺少闭合符号", "Mismatched end tag", "The end tag does not match the start tag, or the end tag is missing its closing symbol" },
    { "E-MIGRATE-005", "error", "MIGRATE", "多余的结束标签", "多余的结束标签：没有与之匹配的开始标签", "Extra end tag", "Extra end tag: no matching start tag" },
    { "E-MIGRATE-006", "error", "MIGRATE", "属性区语法非法", "属性区语法非法：出现非法字符、缺少等号、取值未加引号或引号未闭合", "Invalid attribute-area syntax", "Invalid attribute-area syntax: illegal character, missing equals, unquoted value, or unclosed quote" },
    { "E-MIGRATE-007", "error", "MIGRATE", "注释或声明段未闭合", "注释、处理指令、文档类型声明或 CDATA 段未闭合", "Unclosed comment or declaration", "Unclosed comment, processing instruction, DOCTYPE, or CDATA section" },
    { "E-MIGRATE-008", "error", "MIGRATE", "不支持的声明或处理指令", "此处不支持该声明或处理指令", "Unsupported declaration or PI", "This declaration or processing instruction is not supported here" },
    { "E-MIGRATE-009", "error", "MIGRATE", "实体未闭合或未知", "实体未闭合，或使用了未支持的实体名", "Unterminated or unknown entity", "Unterminated entity, or an unsupported entity name was used" },
    { "E-MIGRATE-010", "error", "MIGRATE", "实体数字非法", "实体的数字部分非法（非十六进制，或不是合法码点）", "Illegal entity number", "The entity's numeric part is illegal (non-hexadecimal, or not a valid code point)" },
    { "E-MIGRATE-011", "error", "MIGRATE", "TOML 语法非法", "TOML 语法非法：不是键值形式、键名为空、缺少取值，或内联表里缺少等号", "Invalid TOML syntax", "Invalid TOML syntax: not key-value form, empty key, missing value, or missing equals in an inline table" },
    { "E-MIGRATE-012", "error", "MIGRATE", "TOML 表定义冲突", "TOML 表定义冲突：同名已存在且不是表，或不是表数组", "TOML table definition conflict", "TOML table definition conflict: the name already exists and is not a table, or is not a table array" },
    { "E-MIGRATE-013", "error", "MIGRATE", "YAML 结构非法", "YAML 结构非法：出现意外内容、不是键值形式、缩进过深，或流式结构后有多余字符", "Invalid YAML structure", "Invalid YAML structure: unexpected content, not key-value form, indentation too deep, or extra characters after a flow structure" },
    { "E-MIGRATE-014", "error", "MIGRATE", "YAML 流式结构未闭合", "YAML 流式结构未闭合", "Unclosed YAML flow structure", "Unclosed YAML flow structure" },
    { "E-MIGRATE-015", "error", "MIGRATE", "YAML 别名未定义", "YAML 别名引用了未定义的锚点", "Undefined YAML alias", "The YAML alias references an undefined anchor" },
    { "E-MIGRATE-016", "error", "MIGRATE", "YAML 非法 UTF-8", "YAML 流式结构里出现非法 UTF-8", "Invalid UTF-8 in YAML", "Invalid UTF-8 inside a YAML flow structure" },
    { "E-MIGRATE-017", "error", "MIGRATE", "不是合法 JSON", "迁入文本不是合法 JSON（或嵌套过深）", "Not valid JSON", "The imported text is not valid JSON (or nested too deeply)" },
    { "E-MIGRATE-018", "error", "MIGRATE", "YAML 未知转义", "YAML 双引号串里出现未知转义序列", "Unknown YAML escape", "An unknown escape sequence appears in a YAML double-quoted string" },
    { "E-PARSE-001", "error", "PARSE", "未闭合的块或数组", "未闭合的块或数组，遇到文件结尾", "Unclosed block or array", "Unclosed block or array; reached end of file" },
    { "E-PARSE-002", "error", "PARSE", "闭合符错配", "块或数组未正确闭合：期望一个符号，却遇到另一个", "Closing symbol mismatch", "Block or array not closed properly: expected one symbol but got another" },
    { "E-PARSE-003", "error", "PARSE", "多余的结束符号", "多余的结束符号，没有与之匹配的开始符号", "Extra closing symbol", "Extra closing symbol with no matching opening symbol" },
    { "E-PARSE-004", "error", "PARSE", "孤立的 at 符号", "孤立的 at 符号不是合法指令（符号与名字之间不可有空白）", "Stray at sign", "A stray at sign is not a valid directive (no whitespace allowed between the sign and the name)" },
    { "E-PARSE-005", "error", "PARSE", "不是合法指令且缺少片段体", "该指令名不是合法指令且缺少片段体", "Not a valid directive and missing fragment body", "The directive name is not a valid directive and is missing a fragment body" },
    { "E-PARSE-006", "error", "PARSE", "期望键或标识符", "期望键或标识符，得其它记号", "Expected key or identifier", "Expected a key or identifier, but got another token" },
    { "E-PARSE-007", "error", "PARSE", "裸词中含逗号", "非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹", "Comma inside a bare word", "Unexpected comma: a bare word cannot contain a comma; use an array or wrap it in quotes" },
    { "E-PARSE-008", "error", "PARSE", "顶层标量不可往返", "顶层须为容器（键值块、对象块或数组），单独的标量无法往返", "Top-level scalar not round-trippable", "The top level must be a container (key-value block, object block, or array); a lone scalar cannot round-trip" },
    { "E-PARSE-009", "error", "PARSE", "命名空间段非法", "命名空间段不可使用该名字", "Illegal namespace segment", "The namespace segment cannot use this name" },
    { "E-PARSE-010", "error", "PARSE", "键名非法", "键名不可使用该名字", "Illegal key name", "The key name cannot use this name" },
    { "E-PARSE-011", "error", "PARSE", "at 符号后缺名字", "at 符号之后须为名字（指令名或片段名）", "Missing name after at sign", "A name must follow the at sign (directive name or fragment name)" },
    { "E-PARSE-012", "error", "PARSE", "语法错误（兜底）", "语法错误：未能给出更具体的原因", "Syntax error (fallback)", "Syntax error: no more specific reason could be given" },
    { "E-PARSE-013", "error", "PARSE", "键名位置不支持插值", "键名位置不支持 `$` 插值：循环变量只可用于值位置", "Interpolation not allowed in key position", "$ interpolation is not allowed in key position: loop variables may only be used in value position" },
    { "E-PARSE-014", "error", "PARSE", "when 条件非法", "`@when` 的条件非法：缺少条件、左侧不是环境变量，或环境变量名缺失", "Invalid when condition", "Invalid @when condition: missing condition, the left side is not an environment variable, or the variable name is missing" },
    { "E-PARSE-015", "error", "PARSE", "when 缺少比较值", "`@when` 的比较运算符后缺少比较值", "when missing comparison value", "The @when comparison operator is missing its comparison value" },
    { "E-PARSE-016", "error", "PARSE", "when 连续出现", "`@when` 连续出现：它只作用于紧邻的下一个字段或块", "Consecutive when", "@when appears consecutively: it only applies to the immediately following field or block" },
    { "E-PARSE-017", "error", "PARSE", "when 后无字段或块", "`@when` 后未跟随任何字段或块", "when not followed by field or block", "@when is not followed by any field or block" },
    { "E-PARSE-018", "error", "PARSE", "for 语法非法", "`@for` 语法非法：缺少循环变量、缺少 `in`、缺少枚举项，或缺少循环体", "Invalid for syntax", "Invalid @for syntax: missing loop variable, missing in, missing enum items, or missing loop body" },
    { "E-PARSE-019", "error", "PARSE", "指令头语法非法", "指令头语法非法：`@contract` / `@is` / `@type` 后缺少契约名、类型名或模式体", "Invalid directive header", "Invalid directive header: @contract / @is / @type is missing a contract name, type name, or pattern body" },
    { "E-PARSE-020", "error", "PARSE", "片段参数语法非法", "片段参数非法：`type` 或 `name` 参数后缺少取值，或同一参数重复", "Invalid fragment parameter", "Invalid fragment parameter: type or name is missing a value, or the same parameter is repeated" },
    { "E-PARSE-021", "error", "PARSE", "default 缺取值", "`default` 修饰符后缺少取值", "default missing value", "The default modifier is missing a value" },
    { "E-PARSE-022", "error", "PARSE", "数值边界取值非数字", "`min` 或 `max` 的边界取值不是数字", "Non-numeric numeric bound", "The min or max bound value is not a number" },
    { "E-PARSE-023", "error", "PARSE", "enum 后不是数组", "`enum` 后须为数组", "enum not followed by array", "enum must be followed by an array" },
    { "E-PARSE-024", "error", "PARSE", "契约定义内字段规格语法非法", "契约定义里的字段规格语法非法：类型为空、数组或枚举缺少闭合、字段名为空", "Invalid field spec in contract", "Invalid field spec inside a contract definition: empty type, unclosed array or enum, or empty field name" },
    { "E-PARSE-025", "error", "PARSE", "受限正则模式非法", "受限正则模式语法非法：量词之前没有可重复的原子、字符类未闭合，或以反斜杠结尾", "Invalid restricted regex", "Invalid restricted-regex syntax: a quantifier with no repeatable atom before it, an unclosed character class, or a trailing backslash" },
    { "I-FEATURE-001", "info", "FEATURE", "命令行特性与解析器版本不一致", "解析器按其固定版本模式工作，命令行声明的特性版本仅作提示", "CLI feature/parser version mismatch", "The parser works in its fixed version mode; the CLI's declared feature version is advisory only" },
    { "W-EDITOR-001", "warning", "EDITOR", "自定义词与官方关键字冲突", "自定义词与官方关键字冲突，已忽略", "Custom word conflicts with official keyword", "A custom word conflicts with an official keyword; ignored" },
    { "W-FEATURE-001", "warning", "FEATURE", "位置参数写法已废弃", "位置参数写法已废弃，推荐带参数名的写法", "Positional-argument form deprecated", "The positional-argument form is deprecated; prefer the named-parameter form" },
    { "W-LINT-001", "warning", "LINT", "片段定义从未被引用", "片段定义后从未被引用", "Fragment never referenced", "A fragment is defined but never referenced" },
    { "W-LINT-002", "warning", "LINT", "契约定义从未被应用", "契约定义后从未被应用", "Contract never applied", "A contract is defined but never applied" },
    { "W-LINT-003", "warning", "LINT", "同一块内键名重复", "键在同一块内重复，后者会覆盖前者", "Duplicate key in the same block", "A key is duplicated within the same block; the later one overrides the former" },
    { "W-LINT-004", "warning", "LINT", "字段是空值", "字段是空值", "Field is null", "The field is null" },
    { "W-LINT-005", "warning", "LINT", "嵌套超过建议阈值", "嵌套深度已超过建议阈值", "Nesting exceeds suggested threshold", "Nesting depth has exceeded the suggested threshold" },
    { NULL, NULL, NULL, NULL, NULL, NULL, NULL } /* 哨兵 */
};

static int sml_locale_is_en(const char *locale) {
    if (!locale || !*locale) return 0;
    char c0 = locale[0], c1 = locale[1];
    if (c0 >= 'A' && c0 <= 'Z') c0 = (char)(c0 + 32);
    if (c1 >= 'A' && c1 <= 'Z') c1 = (char)(c1 + 32);
    return (c0 == 'e' && c1 == 'n');
}

static const struct SmlErrorMsg *sml_find_error(const char *code) {
    if (!code || !*code) return NULL;
    for (int i = 0; SML_ERROR_MSGS[i].code != NULL; i++) {
        if (strcmp(SML_ERROR_MSGS[i].code, code) == 0) return &SML_ERROR_MSGS[i];
    }
    return NULL;
}

/* 取标题（本地化）；未知码 / NULL 入参 -> NULL */
static const char *sml_error_title(const char *code, const char *locale) {
    const struct SmlErrorMsg *e = sml_find_error(code);
    if (!e) return NULL;
    if (sml_locale_is_en(locale) && e->en_title && e->en_title[0]) return e->en_title;
    return e->zh_title;
}

/* 取文案（本地化）；未知码 / NULL 入参 -> NULL */
static const char *sml_error_msg(const char *code, const char *locale) {
    const struct SmlErrorMsg *e = sml_find_error(code);
    if (!e) return NULL;
    if (sml_locale_is_en(locale) && e->en_msg && e->en_msg[0]) return e->en_msg;
    return e->zh_msg;
}

#endif /* SML_CODES_MSG_H */
