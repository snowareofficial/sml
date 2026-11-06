// SPDX-License-Identifier: MulanPSL-2.0
// test_limits.cpp - 深度守卫的回归用例（审计项）
//
// 背景：parse_block 里的子块原先**直接**递归 parse_block，绕过了 parse_value 上的
// 深度守卫 —— 于是 depth 不增长，`a{a{a{...}}}` 这种纯块嵌套可以一路递归打穿栈。
// 栈溢出在 C++ 里不可捕获（进程直接死，catch 不住），所以只能用「进程活着」当断言：
// 用例跑得出结论，就说明没崩。
//
// 两个方向都要钉：
//   · 远超上限的输入必须**报错返回**，而不是崩；
//   · 正常深度（远低于上限）必须**照常解析** —— 守卫不能因为修 bug 变得过敏感。
#include "sml.hpp"

#include <iostream>
#include <string>

using namespace sml;

static int failures = 0;
#define CHECK(c, m) do { if (!(c)) { std::cerr << "FAIL: " << m << "\n"; failures++; } } while (0)

// a { a { ... } }
static std::string nest_blocks(int n) {
    std::string s;
    for (int i = 0; i < n; i++) s += "a { ";
    for (int i = 0; i < n; i++) s += "} ";
    return s;
}

// a: [[[...]]]
static std::string nest_arrays(int n) {
    std::string s = "a: ";
    s.append((std::size_t)n, '[');
    s.append((std::size_t)n, ']');
    return s;
}

// a { x: [ a { x: [ ... ] } ] }：块与数组交替，两边计数要一起起作用
static std::string nest_block_array(int n) {
    std::string s;
    for (int i = 0; i < n; i++) s += "a { x: [ ";
    for (int i = 0; i < n; i++) s += "] } ";
    return s;
}

static void over_depth(const std::string& text, const char* what) {
    std::string err;
    ValuePtr v = Parser::parse(text, &err);
    if (v) {
        std::cerr << "FAIL: " << what << " 应当报错，却解析成功了\n";
        failures++;
        return;
    }
    if (err.find("嵌套过深") == std::string::npos) {
        std::cerr << "FAIL: " << what << " 的错误文案不含「嵌套过深」：" << err << "\n";
        failures++;
    }
}

int main() {
    // 1) 正常深度：守卫不能误伤
    std::cout << "  case: 100 层块嵌套（应当成功）" << std::endl;
    {
        std::string err;
        ValuePtr v = Parser::parse(nest_blocks(100), &err);
        CHECK(v != nullptr, "100 层块嵌套应当解析成功");
        if (!v) std::cerr << "      err: " << err << "\n";
    }
    {
        std::string err;
        ValuePtr v = Parser::parse(nest_arrays(100), &err);
        CHECK(v != nullptr, "100 层数组嵌套应当解析成功");
        if (!v) std::cerr << "      err: " << err << "\n";
    }

    // 2) 远超上限：必须报错返回（修之前这三条都会打穿栈）
    //    崩了就被系统直接收走、缓冲区里的输出全丢 —— 所以每条前先打标记并 flush，
    //    看最后一行就知道死在哪个 case。
    std::cout << "  case: 10 万层块嵌套" << std::endl;
    over_depth(nest_blocks(100000), "10 万层块嵌套");
    std::cout << "  case: 10 万层数组嵌套" << std::endl;
    over_depth(nest_arrays(100000), "10 万层数组嵌套");
    std::cout << "  case: 块与数组交替 10 万层" << std::endl;
    over_depth(nest_block_array(100000), "块与数组交替 10 万层");

    // 3) 边界要**逐格**钉住：128 层放行、第 129 层报此码。
    //    只测 100 与 100000 是测不出「差一格」的 —— 这两个数在任何一种差一格里结果都一样。
    //    口径来源：五端闭合嵌套实测（改前 Rust 127 / C 127 / C++ 128 / JS 128 / Lua 128，
    //    现统一为 128/129）。本文件两条入口都要钉：守卫历史上各自绕过过。
    std::cout << "  case: 深度边界（128 放行 / 129 报）" << std::endl;
    {
        std::string err;
        ValuePtr v = Parser::parse(nest_blocks(128), &err);
        CHECK(v != nullptr, "128 层块嵌套应当放行");
        if (!v) std::cerr << "      err: " << err << "\n";
    }
    over_depth(nest_blocks(129), "129 层块嵌套");
    {
        std::string err;
        ValuePtr v = Parser::parse(nest_arrays(128), &err);
        CHECK(v != nullptr, "128 层数组嵌套应当放行");
        if (!v) std::cerr << "      err: " << err << "\n";
    }
    over_depth(nest_arrays(129), "129 层数组嵌套");
    /* 交替嵌套里**一层 = 两个深度帧**（一个块 + 一个数组），故 64 层交替恰好用满 128 个帧
       （放行），65 层就是 130 个帧（超限）。
       ⚠️ 别把它写成 128 —— 我第一版就是那么写的，跑出来才想起两边计数是一起涨的。 */
    {
        std::string err;
        ValuePtr v = Parser::parse(nest_block_array(64), &err);
        CHECK(v != nullptr, "块/数组交替 64 层应当放行（= 128 个深度帧）");
        if (!v) std::cerr << "      err: " << err << "\n";
    }
    over_depth(nest_block_array(65), "块/数组交替 65 层（= 130 个深度帧）");

    if (failures == 0) {
        std::cout << "ALL LIMIT TESTS PASSED\n";
        return 0;
    }
    std::cout << failures << " FAILURES\n";
    return 1;
}
