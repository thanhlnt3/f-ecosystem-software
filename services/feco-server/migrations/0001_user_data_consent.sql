-- Nhóm task 3 (spec user-data-consent).

-- Bản ghi đồng ý: chỉ ghi thêm, không sửa. Mỗi lần người dùng đồng ý, từ chối hoặc
-- thu hồi quyền của một app là một dòng, kèm phiên bản nội dung màn hình đồng ý đã hiện.
CREATE TABLE consent_records (
    id              BIGSERIAL PRIMARY KEY,
    subject         UUID        NOT NULL,              -- id danh tính Kratos
    client_id       TEXT        NOT NULL,              -- OAuth client của app con
    action          TEXT        NOT NULL CHECK (action IN ('grant', 'deny', 'revoke')),
    granted_scopes  TEXT[]      NOT NULL DEFAULT '{}',
    content_version TEXT        NOT NULL DEFAULT '',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX consent_records_subject ON consent_records (subject, created_at DESC);

-- Nhật ký app con đọc dữ liệu người dùng; giữ 90 ngày.
CREATE TABLE data_access_log (
    id         BIGSERIAL PRIMARY KEY,
    subject    UUID        NOT NULL,
    client_id  TEXT        NOT NULL,
    scopes     TEXT[]      NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX data_access_log_subject ON data_access_log (subject, client_id, created_at DESC);
CREATE INDEX data_access_log_created ON data_access_log (created_at);

-- Địa chỉ nhận sự kiện của mỗi app (khai báo trong manifest, nhóm 8) và khóa ký HMAC.
CREATE TABLE app_webhooks (
    client_id  TEXT PRIMARY KEY,
    url        TEXT        NOT NULL,
    secret     TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hàng đợi webhook gửi đi (gửi lại theo backoff trong 72 giờ).
CREATE TABLE webhook_outbox (
    id              BIGSERIAL PRIMARY KEY,
    client_id       TEXT        NOT NULL,
    event_type      TEXT        NOT NULL,
    payload         JSONB       NOT NULL,
    attempts        INT         NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    delivered_at    TIMESTAMPTZ,
    failed_at       TIMESTAMPTZ,
    last_error      TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX webhook_outbox_due ON webhook_outbox (next_attempt_at)
    WHERE delivered_at IS NULL AND failed_at IS NULL;

-- Yêu cầu xuất dữ liệu cá nhân.
CREATE TABLE data_exports (
    id             UUID PRIMARY KEY,
    subject        UUID        NOT NULL,
    status         TEXT        NOT NULL CHECK (status IN ('pending', 'ready', 'failed')),
    download_token TEXT UNIQUE,
    file           BYTEA,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    ready_at       TIMESTAMPTZ,
    expires_at     TIMESTAMPTZ
);
CREATE INDEX data_exports_subject ON data_exports (subject, created_at DESC);
