-- Nhóm task 6 (spec launcher, "Mở app con và truyền phiên đăng nhập"; design D4).
-- Mã khởi chạy dùng một lần, sống 60 giây, gắn với đúng một app. Chỉ lưu mã băm SHA-256.
CREATE TABLE launch_codes (
    code_hash  TEXT        PRIMARY KEY,
    subject    UUID        NOT NULL,
    client_id  TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    expires_at TIMESTAMPTZ NOT NULL,
    used_at    TIMESTAMPTZ
);
CREATE INDEX launch_codes_expires ON launch_codes (expires_at);
