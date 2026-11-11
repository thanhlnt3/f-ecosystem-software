-- Nhóm task 10 (spec app-sdk, "Kiểm tra quyền sử dụng").
-- Quyền của một người dùng với một app (ví dụ app được bàn giao từ đơn xây app theo yêu cầu).
CREATE TABLE entitlements (
    client_id  TEXT        NOT NULL,
    subject    UUID        NOT NULL,
    kind       TEXT        NOT NULL CHECK (kind IN ('owner', 'license', 'subscription')),
    -- Nguồn cấp quyền, ví dụ custom-build:<mã đơn>, purchase:<mã đơn>.
    source     TEXT        NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ,
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY (client_id, subject, kind)
);
CREATE INDEX entitlements_subject ON entitlements (subject);
