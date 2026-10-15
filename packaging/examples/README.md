# libswsml 消费示例

两个面向**已安装 deb** 的最小示例，演示如何链接并使用 libswsml：

| 文件 | 后端 | 链接 |
|---|---|---|
| `sml_demo.c`   | C-ABI（Rust cdylib） | `pkg-config --cflags --libs swsml` |
| `sml_demo.cpp` | C++17 RAII 封装      | `pkg-config --cflags --libs swsml_cpp` |

安装对应 deb 后：

```sh
# C（libswsml0 + libswsml-dev）
gcc sml_demo.c $(pkg-config --cflags --libs swsml) -o sml_demo && ./sml_demo

# C++（另装 libswsml-cpp0 + libswsml-cpp-dev）
g++ -std=c++17 sml_demo.cpp $(pkg-config --cflags --libs swsml_cpp) -o sml_demo_cpp && ./sml_demo_cpp
```

要点：
- 头文件装在 `/usr/include/sml/`，故 `#include <sml/sml_rs.h>` / `<sml/sml_rs.hpp>`。
- C-ABI：`sml_loads` 结果用 `sml_free` 释放，`char*` 输出用 `sml_free_str` 释放；
  `sml_get`/`sml_at`/`sml_get_path` 返回**借用**指针，不可释放、不跨越根节点。
- C++ RAII：`sml::Value` 析构即释放，`sml::Ref` 为非持有视图，无需手动 free。
- 两个后端**二选一**、不可同时 include（值模型不同）。
