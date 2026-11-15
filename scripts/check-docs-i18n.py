#!/usr/bin/env python3
"""Kiểm tra tài liệu có đủ bản tiếng Anh (tasks.md 15.1; spec platform-support "Ngôn ngữ").

Mỗi tài liệu tiếng Việt `docs/<đường dẫn>.md` (trừ biên bản trong `drills/` và `reports/`) phải có
bản tiếng Anh `docs/en/<đường dẫn>.md`; `README.md` phải có `README.en.md`. Hai bản phải cùng số
tiêu đề (dòng `#` ngoài khối mã) và cùng số khối mã, để phát hiện bản dịch bị thiếu phần.

    python3 scripts/check-docs-i18n.py
"""
import pathlib
import sys

root = pathlib.Path(__file__).resolve().parent.parent
SKIP_DIRS = {"en", "drills", "reports"}


def outline(path):
    headings, fences, in_code = 0, 0, False
    for line in path.read_text(encoding="utf-8").splitlines():
        if line.lstrip().startswith("```"):
            fences += 1
            in_code = not in_code
            continue
        if not in_code and line.startswith("#"):
            headings += 1
    return headings, fences


pairs = [(root / "README.md", root / "README.en.md")]
for vi in sorted((root / "docs").rglob("*.md")):
    rel = vi.relative_to(root / "docs")
    if SKIP_DIRS & set(rel.parts[:-1]):
        continue
    pairs.append((vi, root / "docs" / "en" / rel))

errors = []
for vi, en in pairs:
    if not en.exists():
        errors.append(f"thiếu bản tiếng Anh: {en.relative_to(root)} (của {vi.relative_to(root)})")
        continue
    a, b = outline(vi), outline(en)
    if a != b:
        errors.append(
            f"{en.relative_to(root)}: {b[0]} tiêu đề, {b[1]} dòng ``` "
            f"nhưng {vi.relative_to(root)} có {a[0]} và {a[1]}"
        )

if errors:
    print("\n".join(errors))
    sys.exit(1)
print(f"đủ {len(pairs)} cặp tài liệu Việt - Anh")
