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
