# conformance: expect-error E-CONTRACT-013
# 模式里的量词超上限（`次: 999999` > 单量词上限 1000）⇒ 模式定义非法。
# 此前只有 JS 用细分码（E-LIMIT-009），其余四端一律 E-CONTRACT-013；
# 2026-11-16 按"四端同码"归并（JS 改报 013），本用例钉住这一格。
@type name: T { 序列: [ { 名: 号, 类: 数字, 次: 999999 } ] }
@contract C { f: T }
