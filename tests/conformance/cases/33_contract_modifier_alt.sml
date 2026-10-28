# 钉住：契约修饰符的**另一套记号** `?`（可选）/ `=`（默认值）。
# 现状（本轮实测）：**只有 C 认这套**（c/sml.c 的契约体解析器只吃 `?` / `=` / min / max / loose），
# 而 Rust / C++ / JS / Lua 只认单词形式 `optional` / `default`（见 13 / 14）。
# 两个方向都钉住，才能说清「是没实现」还是「实现成了另一套语法」——
# 13/14 只说 C 不认单词形式，本条说 C 认的是别的记号，而另外四端反过来不认它。
# ⇒ 预期 DIVERGE，已登记 known_divergences.md。
@contract Addr {
    city: str
    country: str = CN
    zip: str?
}

a { @is Addr  city: beijing }
