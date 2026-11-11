"""Test của binding Python (không cần máy chủ): nạp thư viện, báo lỗi đúng định dạng, kiểm chữ ký
webhook. Luồng đăng nhập thật được kiểm ở E2E `sdk_end_to_end` của feco-server.

    FSDK_LIBRARY=target/debug/fsdk_c.dll python -m unittest discover -s sdk/python/tests
"""

import hashlib
import hmac
import os
import sys
import unittest
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1]))

import fsdk  # noqa: E402


class FsdkTest(unittest.TestCase):
    def test_abi(self):
        self.assertEqual(fsdk.abi_version(), 1)

    def test_errors_are_structured(self):
        for key in ("FECO_CLIENT_ID", "FECO_LAUNCH_CODE"):
            os.environ.pop(key, None)
        with self.assertRaises(fsdk.FsdkError) as ctx:
            fsdk.Session.from_launcher(code="ma")
        self.assertEqual(ctx.exception.error, "sdk_error")
        self.assertIn("FECO_CLIENT_ID", ctx.exception.message)
        with self.assertRaises(fsdk.FsdkError) as ctx:
            fsdk.Session.from_launcher()
        self.assertEqual(ctx.exception.error, "not_launched")
        with self.assertRaises(fsdk.FsdkError):
            fsdk.Session.from_launcher(code="ma", client_id="app", issuer="http://127.0.0.1:9/",
                                       server="http://127.0.0.1:9/")

    def test_webhook_signature(self):
        body = b'{"event":"user.revoked"}'
        mac = hmac.new(b"bi-mat", b"1000." + body, hashlib.sha256).hexdigest()
        header = f"t=1000,v1={mac}"
        self.assertTrue(fsdk.verify_webhook("bi-mat", header, body, 1100))
        self.assertFalse(fsdk.verify_webhook("khac", header, body, 1100))
        self.assertFalse(fsdk.verify_webhook("bi-mat", header, body, 2000))


if __name__ == "__main__":
    unittest.main()
