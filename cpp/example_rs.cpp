/*
 * example_rs.cpp — 演示通过 Rust cdylib 后端使用 SML v3 能力 (C++ 封装)。
 * 编译:
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp -L<E:/snoware-target>/release -lsml -o example_rs
 */
#include "sml_rs.hpp"
#include <iostream>

int main() {
    const std::string doc =
        "server {\n"
        "  host: web.example\n"
        "  port: 8080\n"
        "  env: $env.APP_ENV\n"
        "}\n";

    std::cout << "=== 基础 parse ===\n" << sml::parse(doc) << "\n\n";

    sml::ParseOptions opt;
    opt.env = {"APP_ENV=production"};
    opt.allow = {"v1", "v2", "v3"};
    std::cout << "=== v3 parse_ex (env 注入) ===\n"
              << sml::parse_ex(doc, opt) << "\n\n";

    std::cout << "=== features ===\n";
    for (const auto &f : sml::features()) std::cout << "  " << f << "\n";
    std::cout << "\nversion: " << sml::version() << "\n";
    return 0;
}
