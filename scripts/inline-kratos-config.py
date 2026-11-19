"""Tạo bản cấu hình Kratos để chạy trực tiếp trên máy (không qua Docker).

Thay mọi đường dẫn `file:///etc/config/kratos/<tệp>` trong infra/kratos/kratos.yml
bằng nội dung `base64://...` của tệp tương ứng trong infra/kratos/, vì Kratos trên
Windows không đọc được đường dẫn dạng file:///C:/...

Dùng: python scripts/inline-kratos-config.py <thư mục infra/kratos> <tệp ra> [<tệp nguồn>]

<tệp nguồn> mặc định là kratos.yml; truyền kratos.e2e-oidc.yml để sinh phần bổ sung cho E2E.
"""

import base64
import pathlib
import re
import sys


def main() -> None:
    src = pathlib.Path(sys.argv[1])
    out = pathlib.Path(sys.argv[2])
    name = sys.argv[3] if len(sys.argv) > 3 else "kratos.yml"
    text = (src / name).read_text(encoding="utf-8")

    def inline(match: re.Match) -> str:
        data = (src / match.group(1)).read_bytes()
        return "base64://" + base64.b64encode(data).decode()

    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text(re.sub(r"file:///etc/config/kratos/([\w./-]+)", inline, text), encoding="utf-8")


if __name__ == "__main__":
    main()
