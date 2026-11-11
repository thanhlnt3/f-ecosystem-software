"""App mẫu Python dùng F-SDK: qua launcher thì đăng nhập bằng mã khởi chạy, chạy thẳng thì mở
trình duyệt (PKCE); rồi in /v1/me (và quyền sử dụng nếu xin scope `purchases`) dạng JSON.

    cargo build -p fsdk-c
    FSDK_LIBRARY=target/debug/libfsdk_c.so python sdk/python/examples/login.py
"""

import json
import os
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fsdk  # noqa: E402


def main() -> int:
    try:
        with fsdk.Session.auto() as session:
            result = {"me": session.me()}
            if "purchases" in os.environ.get("FSDK_SCOPES", "").split():
                result["entitlements"] = session.entitlements()
            print(json.dumps(result, ensure_ascii=False))
            return 0
    except fsdk.FsdkError as e:
        print(f"lỗi: {e} (request_id {e.request_id})", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
