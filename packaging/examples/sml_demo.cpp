// sml_demo.cpp — 消费 libswsml_cpp 的 C++17 RAII 封装解析 SML。
//
// 安装 deb 后构建：
//   g++ -std=c++17 sml_demo.cpp $(pkg-config --cflags --libs swsml_cpp) -o sml_demo_cpp
// 运行：
//   ./sml_demo_cpp
//
// RAII 所有权：sml::Value 持有根节点、析构即释放；sml::Ref 是非持有借用视图，
// 不得比所属 Value 活得更久 —— 全程无需手动 free。
#include <sml/sml_rs.hpp>
#include <iostream>
#include <string>

int main(){
    std::cout << "libswsml_cpp " << sml::version() << "\n\n";

    const std::string doc =
        "name: John\n"
        "age: 30\n"
        "active: true\n"
        "score: 9.5\n"
        "tags: [ a b c ]\n"
        "server {\n"
        "  host: web.example\n"
        "  port: 8080\n"
        "}\n";

    auto r = sml::loads(doc);
    if (!r.ok()){
        std::cerr << "parse failed: " << r.error.str() << "\n";
        return 1;
    }
    const sml::Value &root = r.value;

    std::cout << "=== 字段读取 ===\n";
    std::cout << "name         = " << root.str_at("name").value_or("?") << "\n";
    std::cout << "age          = " << root.int_at("age").value_or(-1) << "\n";
    std::cout << "score        = " << root.get("score").as_real().value_or(-1) << "\n";
    std::cout << "active       = " << (root.get("active").as_bool().value_or(false) ? "true" : "false") << "\n";
    std::cout << "server.host  = " << root.str_at("server.host").value_or("?") << "\n";
    std::cout << "server.port  = " << root.int_at("server.port").value_or(-1) << "\n";

    std::cout << "\n=== 数组 ===\n";
    auto tags = root.get("tags");
    std::cout << "tags (" << tags.size() << "):";
    for (std::size_t i = 0; i < tags.size(); ++i)
        std::cout << " " << tags.at(i).str().value_or("?");
    std::cout << "\n";

    std::cout << "\n=== 往返：值树 -> SML ===\n" << root.dumps() << "\n";

    std::cout << "\n=== 错误诊断（RAII，无需手动 free）===\n";
    auto bad = sml::loads("@is NoSuchContract\nx: 1\n");
    if (!bad.ok())
        std::cout << "code=" << static_cast<int>(bad.error.code)
                  << " : " << bad.error.text << "\n";

    return 0;
}
