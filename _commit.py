"""生成 git 提交信息。"""
import os

LINES = [
    "fix: tokenizer destroyed non-ASCII chars in quoted strings and bare words",
    "",
    "The byte-based tokenizer did `s.push(cc as char)` where cc is a u8, so",
    "UTF-8 multi-byte characters (e.g. Chinese) were split into Latin-1",
    "code points and re-encoded, turning text into double-encoded mojibake.",
    "",
    "Found in the field: resender's VersionFile parser returned garbled",
    "note fields. Regression test: utf8_in_quoted_string_survives_roundtrip.",
]

path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "_commit_msg.txt")
with open(path, "w", encoding="utf-8") as f:
    f.write("\n".join(LINES) + "\n")
print("written:", path)
