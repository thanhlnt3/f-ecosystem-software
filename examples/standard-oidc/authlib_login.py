"""App mẫu tích hợp F-Eco bằng thư viện OIDC chuẩn (Authlib), không dùng F-SDK.

Khám phá cấu hình tại /.well-known/openid-configuration, đăng nhập bằng authorization code + PKCE,
rồi kiểm chữ ký ID token qua JWKS.

    pip install authlib requests
    python examples/standard-oidc/authlib_login.py <issuer> <client_id> <redirect_uri>

Chương trình in địa chỉ đăng nhập, đọc địa chỉ callback từ stdin (app thật nhận qua máy chủ
loopback của mình), rồi in claims của ID token đã kiểm chữ ký dạng JSON.
"""

import json
import sys

import requests
from authlib.common.security import generate_token
from authlib.integrations.requests_client import OAuth2Session
from authlib.jose import JsonWebKey, jwt


def main() -> int:
    issuer, client_id, redirect_uri = sys.argv[1:4]
    meta = requests.get(
        issuer.rstrip("/") + "/.well-known/openid-configuration", timeout=30
    ).json()
    if meta["issuer"] != issuer:
        print(f"issuer không khớp: {meta['issuer']}", file=sys.stderr)
        return 1
    client = OAuth2Session(
        client_id,
        scope="openid profile",
        redirect_uri=redirect_uri,
        code_challenge_method="S256",
        token_endpoint_auth_method="none",
    )
    verifier = generate_token(48)
    nonce = generate_token(20)
    url, state = client.create_authorization_url(
        meta["authorization_endpoint"], code_verifier=verifier, nonce=nonce
    )
    print(url, flush=True)
    callback = sys.stdin.readline().strip()
    token = client.fetch_token(
        meta["token_endpoint"],
        authorization_response=callback,
        state=state,
        code_verifier=verifier,
    )
    keys = JsonWebKey.import_key_set(requests.get(meta["jwks_uri"], timeout=30).json())
    claims = jwt.decode(
        token["id_token"],
        keys,
        claims_options={
            "iss": {"essential": True, "value": issuer},
            "aud": {"essential": True, "value": client_id},
            "nonce": {"essential": True, "value": nonce},
        },
    )
    claims.validate()
    aud = claims["aud"] if isinstance(claims["aud"], list) else [claims["aud"]]
    print(
        json.dumps(
            {
                "sub": claims["sub"],
                "aud": client_id if client_id in aud else aud,
                "iss": claims["iss"],
            }
        )
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
