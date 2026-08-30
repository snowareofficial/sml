# sml 纯 C 库构建
# 用法:  make           构建 sml_demo (原生实现)
#        make test      编译并运行自检示例
#        make rs        构建 example_rs (桥接 Rust cdylib 的 v3 能力)
#        make clean     清理

CC ?= gcc
CFLAGS ?= -O2 -std=c99 -Wall -Wextra
# Rust cdylib 产物目录 (cargo target-dir); 按需改成你的实际路径
RUST_LIB ?= E:/snoware-target/release

all: sml_demo

sml_demo: sml.c sml.h example.c
	$(CC) $(CFLAGS) sml.c example.c -o sml_demo

test: sml_demo
	./sml_demo

# 桥接 Rust 后端 (sml.dll / libsml.dll.a): 与原生实现并存, 获得 v3 能力
rs: example_rs.c sml_rs.h
	$(CC) $(CFLAGS) example_rs.c -L$(RUST_LIB) -lsml -o example_rs
	./example_rs

clean:
	rm -f sml_demo example_rs
