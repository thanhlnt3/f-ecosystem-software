"""F-SDK cho Python (spec app-sdk). License MIT OR Apache-2.0.

Binding mỏng trên giao diện C của F-SDK (thư viện ``fsdk_c``), nên Python dùng chung lõi
với Rust và C: mã khởi chạy, PKCE, tự làm mới token.

    import fsdk
    session = fsdk.Session.auto()        # qua launcher: mã khởi chạy; chạy thẳng: trình duyệt
    print(session.me()["name"])

Thư viện ``fsdk_c`` được tìm theo biến ``FSDK_LIBRARY``, rồi cạnh gói này.
"""

from __future__ import annotations

import ctypes
import json
import os
import sys
from pathlib import Path
from typing import Any, Optional

__all__ = ["Session", "FsdkError", "abi_version", "verify_webhook"]

ABI_VERSION = 1


def _library_name() -> str:
    if sys.platform == "win32":
        return "fsdk_c.dll"
    if sys.platform == "darwin":
        return "libfsdk_c.dylib"
    return "libfsdk_c.so"


def _load() -> ctypes.CDLL:
    path = os.environ.get("FSDK_LIBRARY") or str(Path(__file__).with_name(_library_name()))
    lib = ctypes.CDLL(path)
    c_str, session = ctypes.c_char_p, ctypes.c_void_p
    owned = ctypes.c_void_p  # chuỗi do F-SDK cấp phát: đọc rồi giải phóng
    signatures = {
        "fsdk_abi_version": ([], ctypes.c_uint32),
        "fsdk_last_error": ([], c_str),
        "fsdk_session_from_launch_code": ([c_str] * 6, session),
        "fsdk_login_browser": ([c_str] * 5, session),
        "fsdk_access_token": ([session], owned),
        "fsdk_refresh_token": ([session], owned),
        "fsdk_me_json": ([session], owned),
        "fsdk_entitlements_json": ([session], owned),
        "fsdk_report_crash": ([session, c_str], ctypes.c_int),
        "fsdk_string_free": ([owned], None),
        "fsdk_session_free": ([session], None),
    }
    for name, (args, ret) in signatures.items():
        fn = getattr(lib, name)
        fn.argtypes = args
        fn.restype = ret
    if lib.fsdk_abi_version() != ABI_VERSION:
        raise ImportError(f"fsdk_c có ABI {lib.fsdk_abi_version()}, binding cần {ABI_VERSION}")
    return lib


_lib: Optional[ctypes.CDLL] = None


def _l() -> ctypes.CDLL:
    global _lib
    if _lib is None:
        _lib = _load()
    return _lib


def abi_version() -> int:
    return int(_l().fsdk_abi_version())


class FsdkError(Exception):
    """Lỗi F-SDK hoặc API F-Eco: ``error``, ``message``, ``message_vi``, ``request_id``, ``status``."""

    def __init__(self, data: dict[str, Any]):
        self.error = data.get("error", "sdk_error")
        self.message = data.get("message", "")
        self.message_vi = data.get("message_vi", "")
        self.request_id = data.get("request_id", "")
        self.status = data.get("status", 0)
        super().__init__(f"{self.error}: {self.message_vi or self.message}")


def _raise() -> None:
    raw = _l().fsdk_last_error() or b""
    try:
        data = json.loads(raw.decode("utf-8")) if raw else {}
    except ValueError:
        data = {"message": raw.decode("utf-8", "replace")}
    raise FsdkError(data)


def _enc(value: Optional[str]) -> Optional[bytes]:
    return value.encode("utf-8") if value else None


def _take(ptr: Optional[int]) -> Optional[str]:
    if not ptr:
        return None
    try:
        return ctypes.string_at(ptr).decode("utf-8")
    finally:
        _l().fsdk_string_free(ptr)


class Session:
    """Phiên đăng nhập. Tham số bỏ trống thì lấy từ biến môi trường (launcher đặt sẵn
    ``FECO_LAUNCH_CODE``, ``FECO_CLIENT_ID``, ``FECO_ISSUER_URL``, ``FECO_SERVER_URL``)."""

    def __init__(self, handle: int):
        self._handle = ctypes.c_void_p(handle)

    @classmethod
    def from_launcher(cls, code: Optional[str] = None, client_id: Optional[str] = None,
                      issuer: Optional[str] = None, server: Optional[str] = None,
                      redirect_uri: Optional[str] = None, scopes: Optional[str] = None) -> "Session":
        handle = _l().fsdk_session_from_launch_code(
            _enc(code), _enc(client_id), _enc(issuer), _enc(server), _enc(redirect_uri), _enc(scopes))
        if not handle:
            _raise()
        return cls(handle)

    @classmethod
    def login_browser(cls, client_id: Optional[str] = None, issuer: Optional[str] = None,
                      server: Optional[str] = None, redirect_uri: Optional[str] = None,
                      scopes: Optional[str] = None) -> "Session":
        handle = _l().fsdk_login_browser(
            _enc(client_id), _enc(issuer), _enc(server), _enc(redirect_uri), _enc(scopes))
        if not handle:
            _raise()
        return cls(handle)

    @classmethod
    def auto(cls, **kwargs: Optional[str]) -> "Session":
        """Qua launcher thì dùng mã khởi chạy; chạy thẳng (hoặc chưa đồng ý scope) thì trình duyệt."""
        if os.environ.get("FECO_LAUNCH_CODE"):
            try:
                return cls.from_launcher(**kwargs)
            except FsdkError as e:
                if e.error != "consent_required":
                    raise
        kwargs.pop("code", None)
        return cls.login_browser(**kwargs)

    def _string(self, fn_name: str) -> str:
        value = _take(getattr(_l(), fn_name)(self._handle))
        if value is None:
            _raise()
        return value  # type: ignore[return-value]

    def access_token(self) -> str:
        return self._string("fsdk_access_token")

    def refresh_token(self) -> Optional[str]:
        return _take(_l().fsdk_refresh_token(self._handle))

    def me(self) -> dict[str, Any]:
        return json.loads(self._string("fsdk_me_json"))

    def entitlements(self) -> list[dict[str, Any]]:
        return json.loads(self._string("fsdk_entitlements_json"))

    def report_crash(self, version: str) -> None:
        if _l().fsdk_report_crash(self._handle, version.encode("utf-8")) != 0:
            _raise()

    def close(self) -> None:
        if self._handle:
            _l().fsdk_session_free(self._handle)
            self._handle = ctypes.c_void_p()

    def __enter__(self) -> "Session":
        return self

    def __exit__(self, *exc: object) -> None:
        self.close()

    def __del__(self) -> None:
        try:
            self.close()
        except Exception:
            pass


def verify_webhook(secret: str, header: str, body: bytes, now: int) -> bool:
    """Kiểm chữ ký ``F-Eco-Signature: t=<giây>,v1=<hex>`` (HMAC-SHA256, lệch tối đa 5 phút)."""
    import hashlib
    import hmac

    parts = dict(p.strip().split("=", 1) for p in header.split(",") if "=" in p)
    try:
        t = int(parts["t"])
    except (KeyError, ValueError):
        return False
    if abs(now - t) > 300 or "v1" not in parts:
        return False
    mac = hmac.new(secret.encode(), f"{t}.".encode() + body, hashlib.sha256).hexdigest()
    return hmac.compare_digest(mac, parts["v1"])
