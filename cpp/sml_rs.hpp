/*
 * sml_rs.hpp — SML v3 C++ 桥接封装 (Rust cdylib 后端)
 *
 * 与 sml.hpp (纯 C++ native 实现) 并存: 本封装不实现解析器, 而是桥接
 * rust crate `swsml` 的 cdylib, 获得完整 v3 能力
 * ($env / glob-include / @feature / @contract)。
 *
 * 链接 (Windows / msys64 ucrt64):
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp -L<E:/snoware-target>/release -lsml -o example_rs
 */
#ifndef SML_RS_HPP
#define SML_RS_HPP

#include <string>
#include <vector>
#include <memory>

namespace sml {

// 自动释放 Rust 返回的 C 字符串
struct CStrDeleter {
    void operator()(char *p) const;
};
using OwnedCStr = std::unique_ptr<char, CStrDeleter>;

// v3 解析选项
struct ParseOptions {
    std::vector<std::string> features;   // 额外启用的特性
    std::vector<std::string> env;        // "KEY=VALUE" 形式注入
    std::vector<std::string> allow;      // 允许的版本, 如 {"v1","v3"}

    std::string to_json() const;
};

// 解析文本 (走 Rust 后端, 完整 v3 能力)。失败返回空串。
std::string parse_ex(const std::string &text, const ParseOptions &opts = {});

// 从文件解析 (自动 include/glob/契约)。失败返回空串。
std::string parse_file(const std::string &path);

// 基础解析 (无 env/feature 上下文)。
std::string parse(const std::string &text);

// 序列化 JSON -> SML。
std::string dump(const std::string &json);

// 支持的特性名列表。
std::vector<std::string> features();

// 版本字符串, 如 "sml 0.4.0"。
std::string version();

} // namespace sml

#endif // SML_RS_HPP
