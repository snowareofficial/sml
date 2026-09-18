# ⚠️ 这是 **tree-sitter 语法覆盖** fixture —— `test/parse/` 下的用例只验**语法**、不验语义，
# 所以它故意把多种构造堆在一起：契约 + `loose`、片段与 `&` 引用、`$env` 内联、`@version v4`、
# `@include`、词中 `@`（邮箱）、路径、带冒号的引号键、含连字符的键。
# 因此它在**语义上是不合法的**：`port: $env.PORT` 展开成字符串却声明为 `int`、`@version v4`
# 超出部分实现、`@include` 在部分实现里还没做 —— **Rust 实测同样拒它**（E-CONTRACT-002）。
# **别"顺手修好"它**：它存在的意义就是语法面够宽；要改先去 W8（Zed grammar 编译验证）那头确认。
@contract Server loose {
    host: str
    port: int default 5432
    tags: [str] optional
}

@base { region: cn-north-1 }

region: &base

db {
    @is Server
    host: db1.internal
    port: $env.PORT
    tags: [ active standby retired ]
}

@version v4
@include "conf.d/db.sml"

# 词中的 @ 是普通字符，邮箱无需引号
to: a@b.c
from: "SML Team <dev@mail.swebase.cn>"
path: a/b/c
"ns:tag": value
read-write: 1
