-- Thay đổi oidc-login-and-token-reuse, design T2: băm refresh token mà Hydra cấp qua
-- POST /oauth2/token của F-Eco, để nhận ra refresh token bị dùng lại. Không lưu token gốc.

CREATE TABLE oauth_token_families (
    id                UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    subject           UUID        NOT NULL,
    client_id         TEXT        NOT NULL,
    created_at        TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Người dùng gỡ quyền app: token bị từ chối sau đó không phải dùng lại.
    revoked_at        TIMESTAMPTZ,
    -- Đã cảnh báo: mỗi chuỗi chỉ cảnh báo một lần.
    reuse_detected_at TIMESTAMPTZ
);
CREATE INDEX oauth_token_families_subject ON oauth_token_families (subject, client_id);
CREATE INDEX oauth_token_families_created ON oauth_token_families (created_at);

CREATE TABLE oauth_refresh_tokens (
    token_hash TEXT        PRIMARY KEY,  -- SHA-256 hex
    family     UUID        NOT NULL REFERENCES oauth_token_families (id) ON DELETE CASCADE,
    issued_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    rotated_at TIMESTAMPTZ
);
CREATE INDEX oauth_refresh_tokens_family ON oauth_refresh_tokens (family);
