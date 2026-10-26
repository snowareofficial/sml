// SPDX-License-Identifier: MulanPSL-2.0
//! SML 错误码本地化信息查找表（由生成器产出，请勿手改）。
//!
//! 中文（默认/源语言）来自 `site/static/errors.json`；英文来自 `errors/msg.en.json`。
//! 某条码/字段缺英文时回退中文，永不返回空。
//!
//! 与 `js/sml-errors.mjs` 逐字对齐（五端同向量同结果）。

/// 一条错误码的完整本地化信息。
pub struct ErrorInfo {
    pub id: &'static str,
    pub severity: &'static str,
    pub domain: &'static str,
    pub title: String,
    pub msg: String,
}

struct Entry {
    id: &'static str,
    severity: &'static str,
    domain: &'static str,
    zh_title: &'static str,
    zh_msg: &'static str,
    en_title: &'static str, // 缺英文为 ""
    en_msg: &'static str,
}

static ENTRIES: &[Entry] = &[
    Entry { id: "E-CLI-001", severity: "error", domain: "CLI", zh_title: "输入格式未知", zh_msg: "输入格式取值未知", en_title: "Unknown input format", en_msg: "The input format value is unknown" },
    Entry { id: "E-CLI-002", severity: "error", domain: "CLI", zh_title: "输出格式未知", zh_msg: "输出格式取值未知", en_title: "Unknown output format", en_msg: "The output format value is unknown" },
    Entry { id: "E-CLI-003", severity: "error", domain: "CLI", zh_title: "参数组合非法", zh_msg: "命令行参数组合非法：互斥项同时给出，或依赖项缺失", en_title: "Invalid argument combination", en_msg: "Invalid command-line argument combination: mutually exclusive options given together, or a required dependency is missing" },
    Entry { id: "E-CLI-004", severity: "error", domain: "CLI", zh_title: "lint 只适用于 SML 文档", zh_msg: "lint 只能检查 SML 文档", en_title: "lint applies to SML documents only", en_msg: "lint can only check SML documents" },
    Entry { id: "E-CLI-005", severity: "error", domain: "CLI", zh_title: "目录模式必须指定输出目录", zh_msg: "目录批量模式必须指定输出目录", en_title: "Directory mode requires an output directory", en_msg: "Directory batch mode requires an output directory" },
    Entry { id: "E-CLI-006", severity: "error", domain: "CLI", zh_title: "规则文档解析或构建失败", zh_msg: "自定义生成规则文档解析或构建失败", en_title: "Rule document parse or build failed", en_msg: "Failed to parse or build the custom generation rule document" },
    Entry { id: "E-CLI-007", severity: "error", domain: "CLI", zh_title: "输出后端报错", zh_msg: "输出后端报错（内层原因见原始错误）", en_title: "Output backend error", en_msg: "Output backend error (see the original error for the inner cause)" },
    Entry { id: "E-CLI-008", severity: "error", domain: "CLI", zh_title: "命令行用法错误", zh_msg: "命令行用法错误：未知参数、缺少取值或取值非法", en_title: "Command-line usage error", en_msg: "Command-line usage error: unknown argument, missing value, or invalid value" },
    Entry { id: "E-CONTRACT-001", severity: "error", domain: "CONTRACT", zh_title: "未定义的契约", zh_msg: "引用了未定义的契约", en_title: "Undefined contract", en_msg: "Referenced an undefined contract" },
    Entry { id: "E-CONTRACT-002", severity: "error", domain: "CONTRACT", zh_title: "字段类型不符", zh_msg: "字段类型应为期望类型，实际为其它类型", en_title: "Field type mismatch", en_msg: "Field type should be the expected type, but is another type" },
    Entry { id: "E-CONTRACT-003", severity: "error", domain: "CONTRACT", zh_title: "必填字段缺失", zh_msg: "字段必填但缺失", en_title: "Required field missing", en_msg: "The field is required but missing" },
    Entry { id: "E-CONTRACT-004", severity: "error", domain: "CONTRACT", zh_title: "未声明字段（严格模式）", zh_msg: "字段未在契约中声明；确需放宽请在契约名后写 loose", en_title: "Undeclared field (strict mode)", en_msg: "Field is not declared in the contract; to allow extra fields, write loose after the contract name" },
    Entry { id: "E-CONTRACT-005", severity: "error", domain: "CONTRACT", zh_title: "数值越界", zh_msg: "字段值小于下界或大于上界", en_title: "Value out of range", en_msg: "Field value is below the lower bound or above the upper bound" },
    Entry { id: "E-CONTRACT-006", severity: "error", domain: "CONTRACT", zh_title: "枚举取值非法", zh_msg: "取值不在枚举列表内", en_title: "Invalid enum value", en_msg: "The value is not in the enum list" },
    Entry { id: "E-CONTRACT-007", severity: "error", domain: "CONTRACT", zh_title: "外置类型校验失败", zh_msg: "字段不符合扩展类型的要求", en_title: "External type check failed", en_msg: "The field does not satisfy the extension type's requirements" },
    Entry { id: "E-CONTRACT-008", severity: "error", domain: "CONTRACT", zh_title: "组合字段应为块", zh_msg: "字段应为块并按该契约校验，实际不是块", en_title: "Composite field should be a block", en_msg: "The field should be a block validated against that contract, but is not a block" },
    Entry { id: "E-CONTRACT-009", severity: "error", domain: "CONTRACT", zh_title: "自定义类型格式不符", zh_msg: "字段的值不符合该类型的格式要求", en_title: "Custom type format mismatch", en_msg: "The field value does not match the type's format requirements" },
    Entry { id: "E-CONTRACT-010", severity: "error", domain: "CONTRACT", zh_title: "数值约束取值为非有限数", zh_msg: "字段的值为非有限数，不能作为数值约束的取值", en_title: "Numeric constraint value is non-finite", en_msg: "The field value is non-finite and cannot be used as a numeric constraint" },
    Entry { id: "E-CONTRACT-011", severity: "error", domain: "CONTRACT", zh_title: "外置修饰符校验失败", zh_msg: "字段不符合扩展修饰符的要求", en_title: "External modifier check failed", en_msg: "The field does not satisfy the extension modifier's requirements" },
    Entry { id: "E-CONTRACT-012", severity: "error", domain: "CONTRACT", zh_title: "未知数组元素类型", zh_msg: "数组元素类型名未知，且不是已注册的扩展类型", en_title: "Unknown array element type", en_msg: "The array element type name is unknown and is not a registered extension type" },
    Entry { id: "E-CONTRACT-013", severity: "error", domain: "CONTRACT", zh_title: "模式定义非法", zh_msg: "模式定义非法：未知字符类、未知量词、不支持的元素，或无法识别的元素", en_title: "Invalid pattern definition", en_msg: "Invalid pattern definition: unknown character class, unknown quantifier, unsupported element, or unrecognizable element" },
    Entry { id: "E-CONTRACT-014", severity: "error", domain: "CONTRACT", zh_title: "模式规则引用非法", zh_msg: "模式规则引用非法：引用了未定义的规则，或规则循环引用", en_title: "Invalid pattern rule reference", en_msg: "Invalid pattern rule reference: references an undefined rule, or the rule references itself cyclically" },
    Entry { id: "E-CONTRACT-015", severity: "error", domain: "CONTRACT", zh_title: "量词取值非法", zh_msg: "量词的取值非法：不是整数、为负数，或上界小于下界", en_title: "Invalid quantifier value", en_msg: "Invalid quantifier value: not an integer, negative, or the upper bound is less than the lower bound" },
    Entry { id: "E-CONTRACT-016", severity: "error", domain: "CONTRACT", zh_title: "契约叠加冲突", zh_msg: "同一字段在参与叠加的两份契约里类型、默认值或区间不一致", en_title: "Contract mixin conflict", en_msg: "The same field has inconsistent type, default, or range across the two mixed-in contracts" },
    Entry { id: "E-CONTRACT-017", severity: "error", domain: "CONTRACT", zh_title: "契约叠加未实现", zh_msg: "该实现暂不支持一个块应用多份契约；请拆分成单契约或等待", en_title: "Contract mixin not implemented", en_msg: "This implementation does not support applying several contracts to one block; split into single contracts or wait" },
    Entry { id: "E-CONTRACT-018", severity: "error", domain: "CONTRACT", zh_title: "契约 @requires 无法满足", zh_msg: "契约声明的 @requires 特性无法满足，契约无法定义或应用", en_title: "", en_msg: "" },
    Entry { id: "E-DERIVE-001", severity: "error", domain: "DERIVE", zh_title: "值与目标类型不符", zh_msg: "SML 值的类型与目标宿主类型不符", en_title: "Value/target type mismatch", en_msg: "The SML value's type does not match the target host type" },
    Entry { id: "E-DERIVE-002", severity: "error", domain: "DERIVE", zh_title: "数值超出目标类型范围", zh_msg: "数值超出目标宿主类型的取值范围", en_title: "Number out of target type range", en_msg: "The number is out of the target host type's range" },
    Entry { id: "E-DERIVE-003", severity: "error", domain: "DERIVE", zh_title: "未知的枚举值或变体", zh_msg: "未知的枚举值或枚举变体", en_title: "Unknown enum value or variant", en_msg: "Unknown enum value or enum variant" },
    Entry { id: "E-DERIVE-004", severity: "error", domain: "DERIVE", zh_title: "枚举变体形态不符", zh_msg: "枚举变体的形态与目标定义不符", en_title: "Enum variant shape mismatch", en_msg: "The enum variant's shape does not match the target definition" },
    Entry { id: "E-DERIVE-005", severity: "error", domain: "DERIVE", zh_title: "序列化时键不是字符串", zh_msg: "序列化时对象的键必须是字符串", en_title: "Non-string key during serialization", en_msg: "Object keys must be strings during serialization" },
    Entry { id: "E-DERIVE-006", severity: "error", domain: "DERIVE", zh_title: "宿主特性未启用", zh_msg: "该能力需要启用对应的宿主构建特性", en_title: "Host feature not enabled", en_msg: "This capability requires enabling the corresponding host build feature" },
    Entry { id: "E-DERIVE-007", severity: "error", domain: "DERIVE", zh_title: "派生属性或类型形状非法", zh_msg: "派生宏的属性或类型形状非法（编译期报错）", en_title: "Invalid derive attribute or type shape", en_msg: "Invalid derive-macro attribute or type shape (compile-time error)" },
    Entry { id: "E-DERIVE-008", severity: "error", domain: "DERIVE", zh_title: "派生入口的解析失败", zh_msg: "派生反序列化时 SML 解析失败（内层原因见原始错误）", en_title: "Derive entry parse failed", en_msg: "SML parsing failed during derive deserialization (see the original error for the inner cause)" },
    Entry { id: "E-EDITOR-001", severity: "error", domain: "EDITOR", zh_title: "解析器加载失败", zh_msg: "编辑器侧的解析器加载失败，补全与诊断不可用", en_title: "Parser load failed", en_msg: "The editor-side parser failed to load; completion and diagnostics are unavailable" },
    Entry { id: "E-EDITOR-002", severity: "error", domain: "EDITOR", zh_title: "高亮配置加载失败", zh_msg: "高亮配置加载或解析失败", en_title: "Highlight config load failed", en_msg: "Failed to load or parse the highlight configuration" },
    Entry { id: "E-EDITOR-003", severity: "error", domain: "EDITOR", zh_title: "无法格式化", zh_msg: "无法格式化：文档存在解析错误", en_title: "Cannot format", en_msg: "Cannot format: the document has parse errors" },
    Entry { id: "E-EDITOR-004", severity: "error", domain: "EDITOR", zh_title: "未打开工作区", zh_msg: "请先打开一个工作区文件夹", en_title: "No workspace open", en_msg: "Please open a workspace folder first" },
    Entry { id: "E-EXT-001", severity: "error", domain: "EXT", zh_title: "未注册的指令、类型或修饰符", zh_msg: "该指令、类型或修饰符未在本环境注册", en_title: "Unregistered directive, type, or modifier", en_msg: "The directive, type, or modifier is not registered in this environment" },
    Entry { id: "E-EXT-002", severity: "error", domain: "EXT", zh_title: "扩展注册名冲突", zh_msg: "不可注册与内置同名的扩展；同名重复注册同样报错", en_title: "Extension registration name conflict", en_msg: "Cannot register an extension with a built-in name; duplicate registration also errors" },
    Entry { id: "E-EXT-003", severity: "error", domain: "EXT", zh_title: "外置指令缺少取值", zh_msg: "外置指令的参数名后缺少取值", en_title: "External directive missing value", en_msg: "The external directive's parameter name is missing a value" },
    Entry { id: "E-EXT-004", severity: "error", domain: "EXT", zh_title: "外置指令返回非对象", zh_msg: "外置指令的合并结果须为对象", en_title: "External directive returned non-object", en_msg: "The external directive's merge result must be an object" },
    Entry { id: "E-EXT-005", severity: "error", domain: "EXT", zh_title: "外置修饰符缺少取值", zh_msg: "外置修饰符期望取值", en_title: "External modifier missing value", en_msg: "The external modifier expects a value" },
    Entry { id: "E-EXT-006", severity: "error", domain: "EXT", zh_title: "custom 规则文档非法", zh_msg: "自定义生成规则文档非法：缺少 rules、rules 为空，或某条规则缺少模板", en_title: "Invalid custom rule document", en_msg: "Invalid custom generation rule document: missing rules, empty rules, or a rule missing its template" },
    Entry { id: "E-EXT-007", severity: "error", domain: "EXT", zh_title: "编辑器定制文档非法", zh_msg: "编辑器定制文档非法：作用域名、规则锚点、颜色取值不符规范", en_title: "Invalid editor customization document", en_msg: "Invalid editor customization document: scope name, rule anchor, or color value does not follow the spec" },
    Entry { id: "E-EXT-008", severity: "error", domain: "EXT", zh_title: "外置指令执行失败", zh_msg: "外置指令处理该输入时失败", en_title: "External directive execution failed", en_msg: "The external directive failed while processing this input" },
    Entry { id: "E-FEATURE-001", severity: "error", domain: "FEATURE", zh_title: "特性未启用", zh_msg: "该语法需要相应特性，请先启用该特性", en_title: "Feature not enabled", en_msg: "This syntax requires the corresponding feature; enable it first" },
    Entry { id: "E-FEATURE-002", severity: "error", domain: "FEATURE", zh_title: "环境变量被禁用", zh_msg: "当前特性集禁用了环境变量内联，裸词或字符串无法解析", en_title: "Environment variables disabled", en_msg: "The current feature set disables environment-variable inlining; bare words or strings cannot be parsed" },
    Entry { id: "E-FEATURE-003", severity: "error", domain: "FEATURE", zh_title: "未知特性名", zh_msg: "未知特性名", en_title: "Unknown feature name", en_msg: "Unknown feature name" },
    Entry { id: "E-FEATURE-004", severity: "error", domain: "FEATURE", zh_title: "版本声明非法", zh_msg: "不支持的版本声明，或版本声明互相冲突；当前支持 v1..v4", en_title: "Invalid version declaration", en_msg: "Unsupported version declaration, or conflicting version declarations" },
    Entry { id: "E-FEATURE-005", severity: "error", domain: "FEATURE", zh_title: "裸词必须加引号", zh_msg: "字符串必须加引号：当前特性集禁用了裸词字符串", en_title: "Bare word must be quoted", en_msg: "Strings must be quoted: the current feature set disables bare-word strings" },
    Entry { id: "E-FEATURE-006", severity: "error", domain: "FEATURE", zh_title: "feature 子命令未知", zh_msg: "未知的 `@feature` 子命令", en_title: "Unknown feature subcommand", en_msg: "Unknown `@feature` subcommand" },
    Entry { id: "E-FEATURE-007", severity: "error", domain: "FEATURE", zh_title: "feature 指令缺少参数", zh_msg: "`@feature` 指令缺少参数", en_title: "feature directive missing argument", en_msg: "The `@feature` directive is missing an argument" },
    Entry { id: "E-FEATURE-008", severity: "error", domain: "FEATURE", zh_title: "feature mode 参数非法", zh_msg: "`@feature mode` 的参数须为白名单或黑名单之一", en_title: "Invalid feature mode argument", en_msg: "The `@feature mode` argument must be either whitelist or blacklist" },
    Entry { id: "E-FEATURE-009", severity: "error", domain: "FEATURE", zh_title: "请求特性与允许集无交集", zh_msg: "文档请求的特性与调用方允许的特性没有交集", en_title: "No overlap with allowed feature set", en_msg: "The document's requested features have no overlap with the caller's allowed set" },
    Entry { id: "E-INCLUDE-001", severity: "error", domain: "INCLUDE", zh_title: "include 文件缺失或读取失败", zh_msg: "include 无法定位或读取目标文件", en_title: "include file missing or read failed", en_msg: "include cannot locate or read the target file" },
    Entry { id: "E-INCLUDE-002", severity: "error", domain: "INCLUDE", zh_title: "include 循环引用", zh_msg: "include 循环引用", en_title: "include circular reference", en_msg: "include circular reference" },
    Entry { id: "E-INCLUDE-003", severity: "error", domain: "INCLUDE", zh_title: "include 越界拒绝", zh_msg: "include 目标不在基准目录内，已拒绝", en_title: "include out-of-bounds rejected", en_msg: "The include target is outside the base directory; rejected" },
    Entry { id: "E-INCLUDE-004", severity: "error", domain: "INCLUDE", zh_title: "include 嵌套超过上限", zh_msg: "include 嵌套超过上限层数", en_title: "include nesting exceeds limit", en_msg: "include nesting exceeds the maximum depth" },
    Entry { id: "E-INCLUDE-005", severity: "error", domain: "INCLUDE", zh_title: "键列表语法非法", zh_msg: "键列表语法非法：期望键列表、或键列表为空、或缺少闭合", en_title: "Invalid key-list syntax", en_msg: "Invalid key-list syntax: expected a key list, or the list is empty, or missing closing" },
    Entry { id: "E-INCLUDE-006", severity: "error", domain: "INCLUDE", zh_title: "未定义的片段引用", zh_msg: "未定义的片段引用", en_title: "Undefined fragment reference", en_msg: "Undefined fragment reference" },
    Entry { id: "E-INCLUDE-007", severity: "error", domain: "INCLUDE", zh_title: "片段展开结果不是对象", zh_msg: "片段展开结果不是对象，无法与所在块合并", en_title: "Fragment expansion is not an object", en_msg: "The fragment expansion result is not an object and cannot be merged into the block" },
    Entry { id: "E-INCLUDE-008", severity: "error", domain: "INCLUDE", zh_title: "部分引用缺少目标文件", zh_msg: "部分引用的写法必须接上目标文件", en_title: "Partial reference missing target file", en_msg: "A partial reference must be followed by a target file" },
    Entry { id: "E-INCLUDE-009", severity: "error", domain: "INCLUDE", zh_title: "部分引用不能配通配", zh_msg: "部分引用不能配合 glob 或 regex 通配，请指定单个文件", en_title: "Partial reference cannot use wildcard", en_msg: "A partial reference cannot be combined with glob or regex wildcards; specify a single file" },
    Entry { id: "E-INCLUDE-010", severity: "error", domain: "INCLUDE", zh_title: "基准目录不可解析", zh_msg: "include 基准目录不可解析，无法做越界校验，已拒绝继续", en_title: "Base directory unresolvable", en_msg: "The include base directory cannot be resolved; the out-of-bounds check is impossible, so parsing stops" },
    Entry { id: "E-INCLUDE-011", severity: "error", domain: "INCLUDE", zh_title: "include 预处理词法失败", zh_msg: "include 预处理阶段的词法失败", en_title: "include preprocessing lex failed", en_msg: "Lexing failed during include preprocessing" },
    Entry { id: "E-INCLUDE-012", severity: "error", domain: "INCLUDE", zh_title: "include 路径写法非法", zh_msg: "include 路径写法非法（未加引号或含非法字符）", en_title: "Invalid include path syntax", en_msg: "Invalid include path syntax (unquoted or containing illegal characters)" },
    Entry { id: "E-INTERNAL-001", severity: "error", domain: "INTERNAL", zh_title: "内部错误", zh_msg: "内部错误：走到了不应到达的分支", en_title: "Internal error", en_msg: "Internal error: reached a branch that should be unreachable" },
    Entry { id: "E-INTERNAL-002", severity: "error", domain: "INTERNAL", zh_title: "内置资源损坏", zh_msg: "内置资源损坏（打包或构建事故）", en_title: "Built-in resource corrupted", en_msg: "Built-in resource corrupted (packaging or build accident)" },
    Entry { id: "E-IO-001", severity: "error", domain: "IO", zh_title: "读取失败", zh_msg: "读取文件失败", en_title: "Read failed", en_msg: "Failed to read the file" },
    Entry { id: "E-IO-002", severity: "error", domain: "IO", zh_title: "输入为空", zh_msg: "输入为空", en_title: "Empty input", en_msg: "Input is empty" },
    Entry { id: "E-IO-003", severity: "error", domain: "IO", zh_title: "写入或建目录失败", zh_msg: "写入文件或创建目录失败", en_title: "Write or mkdir failed", en_msg: "Failed to write the file or create the directory" },
    Entry { id: "E-IO-004", severity: "error", domain: "IO", zh_title: "读取目录失败", zh_msg: "读取目录失败", en_title: "Directory read failed", en_msg: "Failed to read the directory" },
    Entry { id: "E-IO-005", severity: "error", domain: "IO", zh_title: "未检测到输入", zh_msg: "未检测到输入：既没有指定输入文件，也没有可读的管道输入", en_title: "No input detected", en_msg: "No input detected: neither an input file nor readable pipe input was given" },
    Entry { id: "E-IO-006", severity: "error", domain: "IO", zh_title: "标准输入读取失败", zh_msg: "读取标准输入失败或通道中断", en_title: "Standard input read failed", en_msg: "Failed to read standard input, or the channel broke" },
    Entry { id: "E-IO-007", severity: "error", domain: "IO", zh_title: "外部工具不可用或失败", zh_msg: "外部工具不可用（未安装或无法启动），或其构建失败", en_title: "External tool unavailable or failed", en_msg: "The external tool is unavailable (not installed or cannot start), or its build failed" },
    Entry { id: "E-LEX-001", severity: "error", domain: "LEX", zh_title: "字符串未闭合", zh_msg: "字符串未闭合（缺少结束引号）", en_title: "Unterminated string", en_msg: "Unterminated string (missing closing quote)" },
    Entry { id: "E-LEX-002", severity: "error", domain: "LEX", zh_title: "未闭合块注释（斜杠星号）", zh_msg: "未闭合的块注释，遇到文件结尾", en_title: "Unterminated block comment (slash-star)", en_msg: "Unterminated block comment; reached end of file" },
    Entry { id: "E-LEX-003", severity: "error", domain: "LEX", zh_title: "未闭合块注释（下划线星号）", zh_msg: "未闭合的块注释，遇到文件结尾", en_title: "Unterminated block comment (underscore-star)", en_msg: "Unterminated block comment; reached end of file" },
    Entry { id: "E-LEX-004", severity: "error", domain: "LEX", zh_title: "字符串含未知转义符", zh_msg: "字符串含未知转义符，转义集见规范", en_title: "Unknown escape in string", en_msg: "Unknown escape sequence in string; see the spec for the escape set" },
    Entry { id: "E-LEX-005", severity: "error", domain: "LEX", zh_title: "Unicode 转义非法", zh_msg: "Unicode 转义非法（位数不足、非十六进制、或非法码点）", en_title: "Invalid Unicode escape", en_msg: "Invalid Unicode escape (too few digits, non-hexadecimal, or illegal code point)" },
    Entry { id: "E-LEX-006", severity: "error", domain: "LEX", zh_title: "转义符后遇文件结束", zh_msg: "字符串中的转义符后遇到文件结束", en_title: "Escape before end of file", en_msg: "An escape sequence is followed by end of file inside the string" },
    Entry { id: "E-LIMIT-001", severity: "error", domain: "LIMIT", zh_title: "嵌套过深", zh_msg: "嵌套过深，超过本实现的上限层数，疑似递归或恶意输入", en_title: "Nesting too deep", en_msg: "Nesting exceeds this implementation's depth limit; suspected recursion or malicious input" },
    Entry { id: "E-LIMIT-002", severity: "error", domain: "LIMIT", zh_title: "模式匹配超步数预算", zh_msg: "模式匹配超出步数预算，疑似病态规则或超长输入", en_title: "Pattern match over step budget", en_msg: "Pattern matching exceeded the step budget; suspected pathological rule or overlong input" },
    Entry { id: "E-LIMIT-003", severity: "error", domain: "LIMIT", zh_title: "include 展开次数超限", zh_msg: "include 展开次数超过上限，疑似指数膨胀", en_title: "include expansion over limit", en_msg: "include expansions exceeded the limit; suspected exponential blow-up" },
    Entry { id: "E-LIMIT-004", severity: "error", domain: "LIMIT", zh_title: "输出递归深度超过上限", zh_msg: "递归深度超过上限（翻译后端）", en_title: "Output recursion depth over limit", en_msg: "Recursion depth exceeds the limit (translation backend)" },
    Entry { id: "E-LIMIT-005", severity: "error", domain: "LIMIT", zh_title: "custom 输出长度超上限", zh_msg: "custom 生成器输出超过长度上限（模板存在放大）", en_title: "custom output length over limit", en_msg: "custom generator output exceeds the length limit (the template amplifies)" },
    Entry { id: "E-LIMIT-006", severity: "error", domain: "LIMIT", zh_title: "custom 数组循环次数超上限", zh_msg: "custom 生成器数组超过循环上限", en_title: "custom array loop over limit", en_msg: "custom generator array exceeds the loop limit" },
    Entry { id: "E-LIMIT-007", severity: "error", domain: "LIMIT", zh_title: "模式源码长度超上限", zh_msg: "模式（正则）源码超过长度上限，拒绝编译", en_title: "Pattern source length over limit", en_msg: "The pattern (regex) source exceeds the length limit; compilation refused" },
    Entry { id: "E-LIMIT-008", severity: "error", domain: "LIMIT", zh_title: "待校验值长度超上限", zh_msg: "待校验字符串超过长度上限，拒绝校验", en_title: "Value-to-check length over limit", en_msg: "The string to check exceeds the length limit; validation refused" },
    Entry { id: "E-LIMIT-009", severity: "error", domain: "LIMIT", zh_title: "量词取值超上限", zh_msg: "量词的重复次数超过上限", en_title: "Quantifier value over limit", en_msg: "The quantifier's repeat count exceeds the limit" },
    Entry { id: "E-LIMIT-010", severity: "error", domain: "LIMIT", zh_title: "内存分配失败", zh_msg: "内存分配失败", en_title: "Memory allocation failed", en_msg: "Memory allocation failed" },
    Entry { id: "E-LINT-001", severity: "error", domain: "LINT", zh_title: "缩进里出现制表符", zh_msg: "缩进里出现制表符：SML 缩进敏感，请统一用空格", en_title: "Tab in indentation", en_msg: "A tab appears in indentation: SML is indentation-sensitive, use spaces consistently" },
    Entry { id: "E-MIGRATE-001", severity: "error", domain: "MIGRATE", zh_title: "顶层出现文本内容", zh_msg: "迁入文档在顶层出现了文本内容（只允许空白）", en_title: "Text content at top level", en_msg: "The imported document has text content at the top level (only whitespace is allowed)" },
    Entry { id: "E-MIGRATE-002", severity: "error", domain: "MIGRATE", zh_title: "标签未闭合", zh_msg: "标签未闭合：文件在标签内提前结束", en_title: "Unclosed tag", en_msg: "Unclosed tag: the file ends inside a tag" },
    Entry { id: "E-MIGRATE-003", severity: "error", domain: "MIGRATE", zh_title: "标签名为空", zh_msg: "标签名为空", en_title: "Empty tag name", en_msg: "The tag name is empty" },
    Entry { id: "E-MIGRATE-004", severity: "error", domain: "MIGRATE", zh_title: "结束标签不匹配", zh_msg: "结束标签与开始标签不匹配，或结束标签缺少闭合符号", en_title: "Mismatched end tag", en_msg: "The end tag does not match the start tag, or the end tag is missing its closing symbol" },
    Entry { id: "E-MIGRATE-005", severity: "error", domain: "MIGRATE", zh_title: "多余的结束标签", zh_msg: "多余的结束标签：没有与之匹配的开始标签", en_title: "Extra end tag", en_msg: "Extra end tag: no matching start tag" },
    Entry { id: "E-MIGRATE-006", severity: "error", domain: "MIGRATE", zh_title: "属性区语法非法", zh_msg: "属性区语法非法：出现非法字符、缺少等号、取值未加引号或引号未闭合", en_title: "Invalid attribute-area syntax", en_msg: "Invalid attribute-area syntax: illegal character, missing equals, unquoted value, or unclosed quote" },
    Entry { id: "E-MIGRATE-007", severity: "error", domain: "MIGRATE", zh_title: "注释或声明段未闭合", zh_msg: "注释、处理指令、文档类型声明或 CDATA 段未闭合", en_title: "Unclosed comment or declaration", en_msg: "Unclosed comment, processing instruction, DOCTYPE, or CDATA section" },
    Entry { id: "E-MIGRATE-008", severity: "error", domain: "MIGRATE", zh_title: "不支持的声明或处理指令", zh_msg: "此处不支持该声明或处理指令", en_title: "Unsupported declaration or PI", en_msg: "This declaration or processing instruction is not supported here" },
    Entry { id: "E-MIGRATE-009", severity: "error", domain: "MIGRATE", zh_title: "实体未闭合或未知", zh_msg: "实体未闭合，或使用了未支持的实体名", en_title: "Unterminated or unknown entity", en_msg: "Unterminated entity, or an unsupported entity name was used" },
    Entry { id: "E-MIGRATE-010", severity: "error", domain: "MIGRATE", zh_title: "实体数字非法", zh_msg: "实体的数字部分非法（非十六进制，或不是合法码点）", en_title: "Illegal entity number", en_msg: "The entity's numeric part is illegal (non-hexadecimal, or not a valid code point)" },
    Entry { id: "E-MIGRATE-011", severity: "error", domain: "MIGRATE", zh_title: "TOML 语法非法", zh_msg: "TOML 语法非法：不是键值形式、键名为空、缺少取值，或内联表里缺少等号", en_title: "Invalid TOML syntax", en_msg: "Invalid TOML syntax: not key-value form, empty key, missing value, or missing equals in an inline table" },
    Entry { id: "E-MIGRATE-012", severity: "error", domain: "MIGRATE", zh_title: "TOML 表定义冲突", zh_msg: "TOML 表定义冲突：同名已存在且不是表，或不是表数组", en_title: "TOML table definition conflict", en_msg: "TOML table definition conflict: the name already exists and is not a table, or is not a table array" },
    Entry { id: "E-MIGRATE-013", severity: "error", domain: "MIGRATE", zh_title: "YAML 结构非法", zh_msg: "YAML 结构非法：出现意外内容、不是键值形式、缩进过深，或流式结构后有多余字符", en_title: "Invalid YAML structure", en_msg: "Invalid YAML structure: unexpected content, not key-value form, indentation too deep, or extra characters after a flow structure" },
    Entry { id: "E-MIGRATE-014", severity: "error", domain: "MIGRATE", zh_title: "YAML 流式结构未闭合", zh_msg: "YAML 流式结构未闭合", en_title: "Unclosed YAML flow structure", en_msg: "Unclosed YAML flow structure" },
    Entry { id: "E-MIGRATE-015", severity: "error", domain: "MIGRATE", zh_title: "YAML 别名未定义", zh_msg: "YAML 别名引用了未定义的锚点", en_title: "Undefined YAML alias", en_msg: "The YAML alias references an undefined anchor" },
    Entry { id: "E-MIGRATE-016", severity: "error", domain: "MIGRATE", zh_title: "YAML 非法 UTF-8", zh_msg: "YAML 流式结构里出现非法 UTF-8", en_title: "Invalid UTF-8 in YAML", en_msg: "Invalid UTF-8 inside a YAML flow structure" },
    Entry { id: "E-MIGRATE-017", severity: "error", domain: "MIGRATE", zh_title: "不是合法 JSON", zh_msg: "迁入文本不是合法 JSON（或嵌套过深）", en_title: "Not valid JSON", en_msg: "The imported text is not valid JSON (or nested too deeply)" },
    Entry { id: "E-MIGRATE-018", severity: "error", domain: "MIGRATE", zh_title: "YAML 未知转义", zh_msg: "YAML 双引号串里出现未知转义序列", en_title: "Unknown YAML escape", en_msg: "An unknown escape sequence appears in a YAML double-quoted string" },
    Entry { id: "E-PARSE-001", severity: "error", domain: "PARSE", zh_title: "未闭合的块或数组", zh_msg: "未闭合的块或数组，遇到文件结尾", en_title: "Unclosed block or array", en_msg: "Unclosed block or array; reached end of file" },
    Entry { id: "E-PARSE-002", severity: "error", domain: "PARSE", zh_title: "闭合符错配", zh_msg: "块或数组未正确闭合：期望一个符号，却遇到另一个", en_title: "Closing symbol mismatch", en_msg: "Block or array not closed properly: expected one symbol but got another" },
    Entry { id: "E-PARSE-003", severity: "error", domain: "PARSE", zh_title: "多余的结束符号", zh_msg: "多余的结束符号，没有与之匹配的开始符号", en_title: "Extra closing symbol", en_msg: "Extra closing symbol with no matching opening symbol" },
    Entry { id: "E-PARSE-004", severity: "error", domain: "PARSE", zh_title: "孤立的 at 符号", zh_msg: "孤立的 at 符号不是合法指令（符号与名字之间不可有空白）", en_title: "Stray at sign", en_msg: "A stray at sign is not a valid directive (no whitespace allowed between the sign and the name)" },
    Entry { id: "E-PARSE-005", severity: "error", domain: "PARSE", zh_title: "不是合法指令且缺少片段体", zh_msg: "该指令名不是合法指令且缺少片段体", en_title: "Not a valid directive and missing fragment body", en_msg: "The directive name is not a valid directive and is missing a fragment body" },
    Entry { id: "E-PARSE-006", severity: "error", domain: "PARSE", zh_title: "期望键或标识符", zh_msg: "期望键或标识符，得其它记号", en_title: "Expected key or identifier", en_msg: "Expected a key or identifier, but got another token" },
    Entry { id: "E-PARSE-007", severity: "error", domain: "PARSE", zh_title: "裸词中含逗号", zh_msg: "非预期的逗号：裸词中不可含逗号，请改用数组或引号包裹", en_title: "Comma inside a bare word", en_msg: "Unexpected comma: a bare word cannot contain a comma; use an array or wrap it in quotes" },
    Entry { id: "E-PARSE-008", severity: "error", domain: "PARSE", zh_title: "顶层标量不可往返", zh_msg: "顶层须为容器（键值块、对象块或数组），单独的标量无法往返", en_title: "Top-level scalar not round-trippable", en_msg: "The top level must be a container (key-value block, object block, or array); a lone scalar cannot round-trip" },
    Entry { id: "E-PARSE-009", severity: "error", domain: "PARSE", zh_title: "命名空间段非法", zh_msg: "命名空间段不可使用该名字", en_title: "Illegal namespace segment", en_msg: "The namespace segment cannot use this name" },
    Entry { id: "E-PARSE-010", severity: "error", domain: "PARSE", zh_title: "键名非法", zh_msg: "键名不可使用该名字", en_title: "Illegal key name", en_msg: "The key name cannot use this name" },
    Entry { id: "E-PARSE-011", severity: "error", domain: "PARSE", zh_title: "at 符号后缺名字", zh_msg: "at 符号之后须为名字（指令名或片段名）", en_title: "Missing name after at sign", en_msg: "A name must follow the at sign (directive name or fragment name)" },
    Entry { id: "E-PARSE-012", severity: "error", domain: "PARSE", zh_title: "语法错误（兜底）", zh_msg: "语法错误：未能给出更具体的原因", en_title: "Syntax error (fallback)", en_msg: "Syntax error: no more specific reason could be given" },
    Entry { id: "E-PARSE-013", severity: "error", domain: "PARSE", zh_title: "键名位置不支持插值", zh_msg: "键名位置不支持 `$` 插值：循环变量只可用于值位置", en_title: "Interpolation not allowed in key position", en_msg: "$ interpolation is not allowed in key position: loop variables may only be used in value position" },
    Entry { id: "E-PARSE-014", severity: "error", domain: "PARSE", zh_title: "when 条件非法", zh_msg: "`@when` 的条件非法：缺少条件、左侧不是环境变量，或环境变量名缺失", en_title: "Invalid when condition", en_msg: "Invalid @when condition: missing condition, the left side is not an environment variable, or the variable name is missing" },
    Entry { id: "E-PARSE-015", severity: "error", domain: "PARSE", zh_title: "when 缺少比较值", zh_msg: "`@when` 的比较运算符后缺少比较值", en_title: "when missing comparison value", en_msg: "The @when comparison operator is missing its comparison value" },
    Entry { id: "E-PARSE-016", severity: "error", domain: "PARSE", zh_title: "when 连续出现", zh_msg: "`@when` 连续出现：它只作用于紧邻的下一个字段或块", en_title: "Consecutive when", en_msg: "@when appears consecutively: it only applies to the immediately following field or block" },
    Entry { id: "E-PARSE-017", severity: "error", domain: "PARSE", zh_title: "when 后无字段或块", zh_msg: "`@when` 后未跟随任何字段或块", en_title: "when not followed by field or block", en_msg: "@when is not followed by any field or block" },
    Entry { id: "E-PARSE-018", severity: "error", domain: "PARSE", zh_title: "for 语法非法", zh_msg: "`@for` 语法非法：缺少循环变量、缺少 `in`、缺少枚举项，或缺少循环体", en_title: "Invalid for syntax", en_msg: "Invalid @for syntax: missing loop variable, missing in, missing enum items, or missing loop body" },
    Entry { id: "E-PARSE-019", severity: "error", domain: "PARSE", zh_title: "指令头语法非法", zh_msg: "指令头语法非法：`@contract` / `@is` / `@type` 后缺少契约名、类型名或模式体", en_title: "Invalid directive header", en_msg: "Invalid directive header: @contract / @is / @type is missing a contract name, type name, or pattern body" },
    Entry { id: "E-PARSE-020", severity: "error", domain: "PARSE", zh_title: "片段参数语法非法", zh_msg: "片段参数非法：`type` 或 `name` 参数后缺少取值，或同一参数重复", en_title: "Invalid fragment parameter", en_msg: "Invalid fragment parameter: type or name is missing a value, or the same parameter is repeated" },
    Entry { id: "E-PARSE-021", severity: "error", domain: "PARSE", zh_title: "default 缺取值", zh_msg: "`default` 修饰符后缺少取值", en_title: "default missing value", en_msg: "The default modifier is missing a value" },
    Entry { id: "E-PARSE-022", severity: "error", domain: "PARSE", zh_title: "数值边界取值非数字", zh_msg: "`min` 或 `max` 的边界取值不是数字", en_title: "Non-numeric numeric bound", en_msg: "The min or max bound value is not a number" },
    Entry { id: "E-PARSE-023", severity: "error", domain: "PARSE", zh_title: "enum 后不是数组", zh_msg: "`enum` 后须为数组", en_title: "enum not followed by array", en_msg: "enum must be followed by an array" },
    Entry { id: "E-PARSE-024", severity: "error", domain: "PARSE", zh_title: "契约定义内字段规格语法非法", zh_msg: "契约定义里的字段规格语法非法：类型为空、数组或枚举缺少闭合、字段名为空", en_title: "Invalid field spec in contract", en_msg: "Invalid field spec inside a contract definition: empty type, unclosed array or enum, or empty field name" },
    Entry { id: "E-PARSE-025", severity: "error", domain: "PARSE", zh_title: "受限正则模式非法", zh_msg: "受限正则模式语法非法：量词之前没有可重复的原子、字符类未闭合，或以反斜杠结尾", en_title: "Invalid restricted regex", en_msg: "Invalid restricted-regex syntax: a quantifier with no repeatable atom before it, an unclosed character class, or a trailing backslash" },
    Entry { id: "I-FEATURE-001", severity: "info", domain: "FEATURE", zh_title: "命令行特性与解析器版本不一致", zh_msg: "解析器按其固定版本模式工作，命令行声明的特性版本仅作提示", en_title: "CLI feature/parser version mismatch", en_msg: "The parser works in its fixed version mode; the CLI's declared feature version is advisory only" },
    Entry { id: "W-EDITOR-001", severity: "warning", domain: "EDITOR", zh_title: "自定义词与官方关键字冲突", zh_msg: "自定义词与官方关键字冲突，已忽略", en_title: "Custom word conflicts with official keyword", en_msg: "A custom word conflicts with an official keyword; ignored" },
    Entry { id: "W-FEATURE-001", severity: "warning", domain: "FEATURE", zh_title: "位置参数写法已废弃", zh_msg: "位置参数写法已废弃，推荐带参数名的写法", en_title: "Positional-argument form deprecated", en_msg: "The positional-argument form is deprecated; prefer the named-parameter form" },
    Entry { id: "W-LINT-001", severity: "warning", domain: "LINT", zh_title: "片段定义从未被引用", zh_msg: "片段定义后从未被引用", en_title: "Fragment never referenced", en_msg: "A fragment is defined but never referenced" },
    Entry { id: "W-LINT-002", severity: "warning", domain: "LINT", zh_title: "契约定义从未被应用", zh_msg: "契约定义后从未被应用", en_title: "Contract never applied", en_msg: "A contract is defined but never applied" },
    Entry { id: "W-LINT-003", severity: "warning", domain: "LINT", zh_title: "同一块内键名重复", zh_msg: "键在同一块内重复，后者会覆盖前者", en_title: "Duplicate key in the same block", en_msg: "A key is duplicated within the same block; the later one overrides the former" },
    Entry { id: "W-LINT-004", severity: "warning", domain: "LINT", zh_title: "字段是空值", zh_msg: "字段是空值", en_title: "Field is null", en_msg: "The field is null" },
    Entry { id: "W-LINT-005", severity: "warning", domain: "LINT", zh_title: "嵌套超过建议阈值", zh_msg: "嵌套深度已超过建议阈值", en_title: "Nesting exceeds suggested threshold", en_msg: "Nesting depth has exceeded the suggested threshold" },
];

/// 全部错误码 id（按升序）。
pub const ALL: &[&str] = &[
    "E-CLI-001",
    "E-CLI-002",
    "E-CLI-003",
    "E-CLI-004",
    "E-CLI-005",
    "E-CLI-006",
    "E-CLI-007",
    "E-CLI-008",
    "E-CONTRACT-001",
    "E-CONTRACT-002",
    "E-CONTRACT-003",
    "E-CONTRACT-004",
    "E-CONTRACT-005",
    "E-CONTRACT-006",
    "E-CONTRACT-007",
    "E-CONTRACT-008",
    "E-CONTRACT-009",
    "E-CONTRACT-010",
    "E-CONTRACT-011",
    "E-CONTRACT-012",
    "E-CONTRACT-013",
    "E-CONTRACT-014",
    "E-CONTRACT-015",
    "E-CONTRACT-016",
    "E-CONTRACT-017",
    "E-CONTRACT-018",
    "E-DERIVE-001",
    "E-DERIVE-002",
    "E-DERIVE-003",
    "E-DERIVE-004",
    "E-DERIVE-005",
    "E-DERIVE-006",
    "E-DERIVE-007",
    "E-DERIVE-008",
    "E-EDITOR-001",
    "E-EDITOR-002",
    "E-EDITOR-003",
    "E-EDITOR-004",
    "E-EXT-001",
    "E-EXT-002",
    "E-EXT-003",
    "E-EXT-004",
    "E-EXT-005",
    "E-EXT-006",
    "E-EXT-007",
    "E-EXT-008",
    "E-FEATURE-001",
    "E-FEATURE-002",
    "E-FEATURE-003",
    "E-FEATURE-004",
    "E-FEATURE-005",
    "E-FEATURE-006",
    "E-FEATURE-007",
    "E-FEATURE-008",
    "E-FEATURE-009",
    "E-INCLUDE-001",
    "E-INCLUDE-002",
    "E-INCLUDE-003",
    "E-INCLUDE-004",
    "E-INCLUDE-005",
    "E-INCLUDE-006",
    "E-INCLUDE-007",
    "E-INCLUDE-008",
    "E-INCLUDE-009",
    "E-INCLUDE-010",
    "E-INCLUDE-011",
    "E-INCLUDE-012",
    "E-INTERNAL-001",
    "E-INTERNAL-002",
    "E-IO-001",
    "E-IO-002",
    "E-IO-003",
    "E-IO-004",
    "E-IO-005",
    "E-IO-006",
    "E-IO-007",
    "E-LEX-001",
    "E-LEX-002",
    "E-LEX-003",
    "E-LEX-004",
    "E-LEX-005",
    "E-LEX-006",
    "E-LIMIT-001",
    "E-LIMIT-002",
    "E-LIMIT-003",
    "E-LIMIT-004",
    "E-LIMIT-005",
    "E-LIMIT-006",
    "E-LIMIT-007",
    "E-LIMIT-008",
    "E-LIMIT-009",
    "E-LIMIT-010",
    "E-LINT-001",
    "E-MIGRATE-001",
    "E-MIGRATE-002",
    "E-MIGRATE-003",
    "E-MIGRATE-004",
    "E-MIGRATE-005",
    "E-MIGRATE-006",
    "E-MIGRATE-007",
    "E-MIGRATE-008",
    "E-MIGRATE-009",
    "E-MIGRATE-010",
    "E-MIGRATE-011",
    "E-MIGRATE-012",
    "E-MIGRATE-013",
    "E-MIGRATE-014",
    "E-MIGRATE-015",
    "E-MIGRATE-016",
    "E-MIGRATE-017",
    "E-MIGRATE-018",
    "E-PARSE-001",
    "E-PARSE-002",
    "E-PARSE-003",
    "E-PARSE-004",
    "E-PARSE-005",
    "E-PARSE-006",
    "E-PARSE-007",
    "E-PARSE-008",
    "E-PARSE-009",
    "E-PARSE-010",
    "E-PARSE-011",
    "E-PARSE-012",
    "E-PARSE-013",
    "E-PARSE-014",
    "E-PARSE-015",
    "E-PARSE-016",
    "E-PARSE-017",
    "E-PARSE-018",
    "E-PARSE-019",
    "E-PARSE-020",
    "E-PARSE-021",
    "E-PARSE-022",
    "E-PARSE-023",
    "E-PARSE-024",
    "E-PARSE-025",
    "I-FEATURE-001",
    "W-EDITOR-001",
    "W-FEATURE-001",
    "W-LINT-001",
    "W-LINT-002",
    "W-LINT-003",
    "W-LINT-004",
    "W-LINT-005",
];

/// 默认 locale（中文/源语言）。
pub const DEFAULT_LOCALE: &str = "zh";

fn norm_locale(locale: &str) -> &str {
    if locale.is_empty() { return DEFAULT_LOCALE; }
    let l = locale.to_lowercase();
    if l.starts_with("en") { "en" } else { "zh" }
}

/// 取一条错误码的完整本地化信息；未知码返回 `None`。
///
/// - `code`：错误码字符串（空/非字符串按未知码处理）。
/// - `locale`：可选；缺省 / 空 = `zh`；以 `en` 开头 = 英文；其它回落中文。
/// - 缺英文：该字段回退中文，永不返回空串。
pub fn error_info(code: &str, locale: &str) -> Option<ErrorInfo> {
    if code.is_empty() { return None; }
    let e = ENTRIES.iter().find(|e| e.id == code)?;
    let mut title = e.zh_title.to_string();
    let mut msg = e.zh_msg.to_string();
    if norm_locale(locale) == "en" {
        if !e.en_title.is_empty() { title = e.en_title.to_string(); }
        if !e.en_msg.is_empty() { msg = e.en_msg.to_string(); }
    }
    Some(ErrorInfo {
        id: e.id,
        severity: e.severity,
        domain: e.domain,
        title,
        msg,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zh_default_title() {
        let i = error_info("E-PARSE-006", "").unwrap();
        assert_eq!(i.title, "期望键或标识符");
        assert_eq!(i.domain, "PARSE");
        assert_eq!(i.severity, "error");
    }

    #[test]
    fn en_title() {
        let i = error_info("E-PARSE-006", "en").unwrap();
        assert_eq!(i.title, "Expected key or identifier");
    }

    #[test]
    fn en_us_normalize() {
        let i = error_info("E-LEX-001", "en-US").unwrap();
        assert_eq!(i.title, "Unterminated string");
    }

    #[test]
    fn locale_fallback_zh() {
        assert_eq!(error_info("E-LEX-001", "zh-CN").unwrap().title, "字符串未闭合");
        assert_eq!(error_info("E-LEX-001", "fr").unwrap().title, "字符串未闭合");
    }

    #[test]
    fn unknown_is_none() {
        assert!(error_info("E-NOPE-999", "zh").is_none());
        assert!(error_info("E-NOPE-999", "").is_none());
        assert!(error_info("", "zh").is_none());
    }

    #[test]
    fn all_nonempty_and_parse_count() {
        let mut parse = 0usize;
        let mut empty = 0usize;
        for code in ALL {
            let i = error_info(code, "zh").expect("known code");
            if i.title.is_empty() || i.msg.is_empty() { empty += 1; }
            if i.domain == "PARSE" { parse += 1; }
        }
        assert_eq!(empty, 0, "所有码 title/msg 必须非空");
        assert_eq!(parse, 25, "PARSE 领域应为 25 条");
    }
}
