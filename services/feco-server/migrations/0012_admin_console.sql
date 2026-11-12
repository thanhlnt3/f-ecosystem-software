-- Nhóm task 14 (spec admin-console, notifications).

-- Thành viên bảng quản trị và vai trò: reviewer (người duyệt app), support (hỗ trợ),
-- project_manager (quản lý dự án), finance (tài chính), sysadmin (quản trị hệ thống).
CREATE TABLE admin_members (
    subject    UUID        PRIMARY KEY,
    email      TEXT        NOT NULL,
    roles      TEXT[]      NOT NULL CHECK (roles <@ ARRAY['reviewer', 'support', 'project_manager', 'finance', 'sysadmin']::TEXT[]),
    added_by   TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Trung tâm thông báo (web và F-Launcher).
CREATE TABLE notifications (
    id         BIGSERIAL   PRIMARY KEY,
    subject    UUID        NOT NULL,
    category   TEXT        NOT NULL,
    title      TEXT        NOT NULL,
    body       TEXT        NOT NULL,
    link       TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    read_at    TIMESTAMPTZ
);
CREATE INDEX notifications_subject ON notifications (subject, id DESC);

-- Tùy chọn nhận thông báo cho các loại tắt được (cập nhật app, gợi ý app, bản tin).
CREATE TABLE notification_preferences (
    subject    UUID        NOT NULL,
    category   TEXT        NOT NULL CHECK (category IN ('app_updates', 'recommendations', 'newsletter')),
    enabled    BOOLEAN     NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject, category)
);
