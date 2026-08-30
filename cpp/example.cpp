// SPDX-License-Identifier: MulanPSL-2.0
// example.cpp - usage demo + compile verification for sml C++.
#include "sml.hpp"
#include <iostream>

using namespace sml;

static void print(const ValuePtr& v, int d=0) {
    std::string pad((size_t)d*2, ' ');
    switch (v->tag) {
        case Value::Tag::Null: std::cout << "null"; break;
        case Value::Tag::Bool: std::cout << (v->b?"true":"false"); break;
        case Value::Tag::Int:  std::cout << v->i; break;
        case Value::Tag::Float: std::cout << v->f; break;
        case Value::Tag::Str:  std::cout << "\"" << v->s << "\""; break;
        case Value::Tag::Arr:
            std::cout << "[\n";
            for (auto& e : v->arr) { std::cout << pad << "  "; print(e, d+1); std::cout << "\n"; }
            std::cout << pad << "]";
            break;
        case Value::Tag::Obj: {
            std::cout << "{\n";
            for (auto& kv : v->obj) {
                std::cout << pad << "  " << kv.first << ": ";
                print(kv.second, d+1); std::cout << "\n";
            }
            std::cout << pad << "}";
            break;
        }
    }
}

int main() {
    const char* text = R"(
# SML showcase (aligned with Rust)
name: Soup
version: 5.5
stable: true
tags: [ lang config ]
empty: []

server:
{
    host: 0.0.0.0
    port: 8080
    aliases:
    [
        { name: alpha }
        { name: beta }
    ]
}

# fragment (snippet)
@base
{
    timeout: 30
}
derived: &base

# bare block
pool greeter
{
    max: 10
}

# contract
@contract User
{
    id: int
    name: str
    role: enum [ admin user ] default "user"
    age: int optional min 0 max 150
}
user @is User
{
    id: 1
    name: Alice
    age: 30
}
)";

    std::string err;
    auto v = Parser::parse(text, &err);
    if (!v) {
        std::cerr << "parse error: " << err << "\n";
        return 1;
    }
    print(v);
    std::cout << "\n\n--- round-trip ---\n";
    std::cout << Parser::to_sml(v) << "\n";

    // value access
    std::cout << "name = "; print(v->get("name")); std::cout << "\n";
    return 0;
}
