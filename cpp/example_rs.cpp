/*
 * example_rs.cpp — SML v3 through the Rust cdylib backend (C++17 wrapper).
 *
 * Build:
 *   g++ -std=c++17 example_rs.cpp sml_rs.cpp -L<E:/snoware-target>/release -lsml -o example_rs
 */
#include "sml_rs.hpp"

#include <cstdio>
#include <fstream>
#include <iostream>
#include <string>

#ifdef _WIN32
#include <direct.h>
#define MKDIR(p) _mkdir(p)
#else
#include <sys/stat.h>
#define MKDIR(p) mkdir(p, 0755)
#endif

static int failures = 0;

static void check(bool cond, const char *msg) {
    if (cond) {
        std::cout << "  [ ok ] " << msg << "\n";
    } else {
        std::cout << "  [FAIL] " << msg << "\n";
        failures++;
    }
}

// ---------------------------------------------------------------------------
// 1. Load a document and walk the value tree directly — no JSON library needed.
// ---------------------------------------------------------------------------
static void demo_traverse() {
    std::cout << "=== 1. traverse the value tree ===\n";

    const std::string doc = "name: John\n"
                            "age: 27\n"
                            "active: true\n"
                            "server {\n"
                            "  host: web.example\n"
                            "  port: 8080\n"
                            "}\n"
                            "tags: [ a b c ]\n";

    auto r = sml::loads(doc);
    if (!r.ok()) {
        std::cout << "  load failed: " << r.error.str() << "\n";
        failures++;
        return;
    }

    const sml::Value &root = r.value;

    check(root.type() == sml::Type::Object, "root is an object");
    check(root.size() == 5, "root has 5 fields");

    // Scalars
    check(root.get("name").str().value_or("") == "John", "name == \"John\"");
    check(root.get("age").as_int().value_or(0) == 27, "age == 27");
    check(root.get("active").as_bool().value_or(false), "active == true");

    // Dotted path: one call instead of chained get()
    check(root.str_at("server.host").value_or("") == "web.example",
          "server.host read via path");
    check(root.int_at("server.port").value_or(0) == 8080,
          "server.port read via path");

    // Missing path yields nullopt (never a silent zero)
    check(!root.int_at("server.nope").has_value(), "missing path yields nullopt");

    // Arrays
    auto tags = root.get("tags");
    check(tags.type() == sml::Type::Array, "tags is an array");
    check(tags.size() == 3, "tags has 3 elements");
    check(!tags.at(3).valid(), "out-of-range index is invalid");

    // Round-trip back to SML
    check(!root.dumps().empty(), "dumps() produced output");

    std::cout << "\n";
}

// ---------------------------------------------------------------------------
// 2. Error reporting with location.
// ---------------------------------------------------------------------------
static void demo_error() {
    std::cout << "=== 2. error reporting ===\n";

    // Referencing an undefined contract is a guaranteed failure.
    // (The parser is deliberately tolerant of unclosed quotes/blocks,
    //  so those are not good error test cases.)
    auto r = sml::loads("@is NoSuchContract\nx: 1\n");

    check(!r.ok(), "undefined contract -> failure");
    check(r.error.code == sml::Err::Contract, "code == Err::Contract");
    check(!r.error.text.empty(), "error message is filled");
    check(r.error.source == "<string>", "source == \"<string>\"");
    std::cout << "       " << r.error.str() << "\n";

    std::cout << "\n";
}

// ---------------------------------------------------------------------------
// 3. Feature flags: tighten what a document may use.
// ---------------------------------------------------------------------------
static void demo_flags() {
    std::cout << "=== 3. feature flags ===\n";

    std::cout << "  mask: 0x" << std::hex << sml::feature_mask() << std::dec << "\n";
    for (const auto &n : sml::feature_names()) {
        std::cout << "    " << n << "\n";
    }

    const std::string doc = "secret: $env.MY_TOKEN\n";

    // Without SML_F_ENV the document must be rejected.
    auto denied = sml::loads(doc, SML_F_BASIC);
    check(!denied.ok(), "env rejected when SML_F_ENV is absent");

    // ... and accepted once granted.
    auto granted = sml::loads(doc, SML_F_BASIC | SML_F_ENV);
    std::cout << "  with SML_F_ENV: " << (granted.ok() ? "loaded" : "still failed") << "\n";

    std::cout << "\n";
}

// ---------------------------------------------------------------------------
// 4. File loading expands `include` (paths must be quoted).
// ---------------------------------------------------------------------------
static void demo_include() {
    std::cout << "=== 4. include expansion ===\n";

    std::string tmp = ".";
    if (const char *t = std::getenv("TMP")) {
        tmp = t;
    } else if (const char *t = std::getenv("TEMP")) {
        tmp = t;
    }

    const std::string dir = tmp + "/sml_rs_cpp_example";
    MKDIR(dir.c_str());
    MKDIR((dir + "/conf.d").c_str());

    {
        std::ofstream f(dir + "/conf.d/extra.sml", std::ios::binary);
        f << "from_name: ops\nmonth_count: 12\n";
    }
    {
        std::ofstream f(dir + "/main.sml", std::ios::binary);
        f << "include \"conf.d/extra.sml\"\nport: 8080\n";
    }

    auto r = sml::load_file(dir + "/main.sml");
    if (!r.ok()) {
        std::cout << "  [FAIL] load_file: " << r.error.str() << "\n";
        failures++;
        return;
    }

    check(r.value.str_at("from_name").value_or("") == "ops", "included field merged");
    check(r.value.int_at("month_count").value_or(0) == 12, "included field merged (count)");
    check(r.value.int_at("port").value_or(0) == 8080, "own field still present");

    std::cout << "\n";
}

// ---------------------------------------------------------------------------
// 5. Move semantics: ownership is transferred, never double-freed.
// ---------------------------------------------------------------------------
static void demo_move() {
    std::cout << "=== 5. move semantics ===\n";

    auto r = sml::loads("a: 1\n");
    check(r.ok(), "loaded");

    sml::Value moved = std::move(r.value);
    check(moved.valid(), "moved-to Value is valid");
    check(!r.value.valid(), "moved-from Value is empty");
    check(moved.int_at("a").value_or(0) == 1, "data survives the move");

    std::cout << "\n";
}

int main() {
    std::cout << "sml version: " << sml::version() << "\n\n";

    demo_traverse();
    demo_error();
    demo_flags();
    demo_include();
    demo_move();

    if (failures == 0) {
        std::cout << "=== ALL CHECKS PASSED ===\n";
        return 0;
    }
    std::cout << "=== " << failures << " CHECK(S) FAILED ===\n";
    return 1;
}
