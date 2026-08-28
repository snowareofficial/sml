"""生成 git 提交信息（PowerShell 不支持 heredoc/内联多行）。"""
import os

LINES = [
    "fix: dump_inline lost nested arrays/objects inside array elements",
    "",
    "An array of objects (e.g. entries list in a config) that contained a nested",
    "array was serialized with the placeholder [..] instead of the real values,",
    'so round-tripping turned chunks: [c1 c2] into chunks: [..], which parsed',
    'back as [".."]. dump_inline now recurses into nested values.',
    "",
    "Regression test: nested_array_inside_object_inside_array_survives_roundtrip.",
]

path = os.path.join(os.path.dirname(os.path.abspath(__file__)), "_commit_msg.txt")
with open(path, "w", encoding="utf-8") as f:
    f.write("\n".join(LINES) + "\n")
print("written:", path)
