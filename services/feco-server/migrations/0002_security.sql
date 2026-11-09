-- Nhóm task 4 (spec platform-security).

-- Nhật ký kiểm toán: chỉ ghi thêm. Trigger chặn mọi lệnh sửa, chặn xóa bản ghi chưa
-- quá 1 năm (chỉ tác vụ lưu trữ được xóa bản ghi hết hạn), và chặn TRUNCATE.
-- Ở production, feco-server kết nối bằng vai trò không có quyền ALTER/DISABLE TRIGGER.
CREATE TABLE audit_log (
    id         BIGSERIAL PRIMARY KEY,
    at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    actor_type TEXT        NOT NULL CHECK (actor_type IN ('user', 'admin', 'app', 'system')),
    actor_id   TEXT,
    action     TEXT        NOT NULL,
    target     TEXT,
    ip         TEXT,
    details    JSONB       NOT NULL DEFAULT '{}'
);
CREATE INDEX audit_log_at ON audit_log (at);
CREATE INDEX audit_log_actor ON audit_log (actor_id, at DESC);

CREATE FUNCTION audit_log_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'UPDATE' THEN
        RAISE EXCEPTION 'audit_log chỉ được ghi thêm (append-only): không được sửa';
    END IF;
    IF TG_OP = 'DELETE' AND OLD.at > now() - interval '1 year' THEN
        RAISE EXCEPTION 'audit_log chỉ được ghi thêm (append-only): chỉ xóa được bản ghi quá 1 năm';
    END IF;
    RETURN OLD;
END $$;

CREATE FUNCTION audit_log_no_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'audit_log chỉ được ghi thêm (append-only): không được TRUNCATE';
END $$;

CREATE TRIGGER audit_log_guard BEFORE UPDATE OR DELETE ON audit_log
    FOR EACH ROW EXECUTE FUNCTION audit_log_guard();
CREATE TRIGGER audit_log_no_truncate BEFORE TRUNCATE ON audit_log
    FOR EACH STATEMENT EXECUTE FUNCTION audit_log_no_truncate();

-- Thiết bị đã từng đăng nhập của mỗi người dùng (phát hiện đăng nhập bất thường).
CREATE TABLE known_devices (
    subject    UUID        NOT NULL,
    device     TEXT        NOT NULL,
    country    TEXT        NOT NULL DEFAULT '',
    first_seen TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_seen  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject, device, country)
);

-- Token của nút "Không phải tôi" trong email cảnh báo; chỉ lưu mã băm, dùng một lần.
CREATE TABLE security_alert_tokens (
    token_hash TEXT PRIMARY KEY,
    subject    UUID        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    used_at    TIMESTAMPTZ
);

-- Khóa ký webhook và file xuất dữ liệu được mã hóa AES-256-GCM (src/crypto.rs).
ALTER TABLE data_exports ADD COLUMN encrypted BOOLEAN NOT NULL DEFAULT false;
