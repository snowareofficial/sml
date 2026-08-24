# sml 纯 C 库构建
# 用法:  make           构建 sml_demo
#        make test      编译并运行自检示例
#        make clean     清理

CC ?= gcc
CFLAGS ?= -O2 -std=c99 -Wall -Wextra

all: sml_demo

sml_demo: sml.c sml.h example.c
	$(CC) $(CFLAGS) sml.c example.c -o sml_demo

test: sml_demo
	./sml_demo

clean:
	rm -f sml_demo
