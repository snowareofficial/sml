# sml 纯 C 库构建
# 用法:  make           构建 build/sml_demo (原生实现)
#        make test      编译并运行自检示例与回归用例
#        make rs        构建 build/example_rs (桥接 Rust cdylib 的 v3 能力)
#        make clean     清理 build/
#
# 产物一律落 build/（已 gitignore）：源码目录只放源码。

CC ?= gcc
CFLAGS ?= -O2 -std=c99 -Wall -Wextra
# Rust cdylib 产物目录 (cargo target-dir); 按需改成你的实际路径
RUST_LIB ?= ../rust/target/release

BUILD := build
CPPFLAGS += -I.

all: $(BUILD)/sml_demo

$(BUILD):
	mkdir -p $(BUILD)

$(BUILD)/sml_demo: sml.c sml.h examples/example.c | $(BUILD)
	$(CC) $(CFLAGS) $(CPPFLAGS) sml.c examples/example.c -o $@

test: $(BUILD)/sml_demo $(BUILD)/test_limits $(BUILD)/test_codes
	./$(BUILD)/sml_demo
	./$(BUILD)/test_limits
	./$(BUILD)/test_codes

# 回归用例：深度上限 +「err 允许为 NULL」（说明见 tests/test_limits.c 头部）
$(BUILD)/test_limits: sml.c sml.h tests/test_limits.c | $(BUILD)
	$(CC) $(CFLAGS) $(CPPFLAGS) sml.c tests/test_limits.c -o $@

# 错误码回归：触发条件 -> 期望码（码是跨端稳定契约，说明见 tests/test_codes.c 头部）
$(BUILD)/test_codes: sml.c sml.h sml_codes.h tests/test_codes.c | $(BUILD)
	$(CC) $(CFLAGS) $(CPPFLAGS) sml.c tests/test_codes.c -o $@

# 桥接 Rust 后端 (sml.dll / libsml.dll.a): 与原生实现并存, 获得 v3 能力
rs: examples/example_rs.c sml_rs.h | $(BUILD)
	$(CC) $(CFLAGS) $(CPPFLAGS) examples/example_rs.c -L$(RUST_LIB) -lsml -o $(BUILD)/example_rs
	./$(BUILD)/example_rs

clean:
	rm -rf $(BUILD)
