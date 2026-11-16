# conformance: expect-ok
# `lex.ident` 开启后，白名单内的标识符照常可用（正对照：防白名单误伤）。
# 覆盖 ASCII、下划线、连字符、汉字、假名、谚文 —— 五端同码的依据是
# `docs/swsml/plan/featag-slices.md` §2 的白名单表。
@feature enable lex.ident
user_id: 1
x-y: 2
端口: 3
設定ファイル: 4
이름: 5
