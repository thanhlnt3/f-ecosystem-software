-- Vai trò nền tảng (thay đổi OpenSpec platform-roles, design R1–R3).
-- Quản trị viên vẫn lưu ở admin_members (kèm 5 vai trò con); bảng này chỉ lưu quyền Lập trình viên.

CREATE TABLE role_grants (
    subject    UUID        PRIMARY KEY,
    role       TEXT        NOT NULL CHECK (role IN ('developer')),
    email      TEXT        NOT NULL,
    granted_by TEXT        NOT NULL,
    reason     TEXT        NOT NULL,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Thành viên nhóm nhà phát triển có từ trước giữ quyền (scenario "Nhà phát triển có từ trước").
INSERT INTO role_grants (subject, role, email, granted_by, reason)
SELECT DISTINCT ON (subject) subject, 'developer', email, 'migration',
       'Đã là thành viên nhà phát triển trước khi có phân quyền'
FROM developer_members
WHERE subject IS NOT NULL AND status = 'active'
ORDER BY subject, joined_at
ON CONFLICT (subject) DO NOTHING;

-- Yêu cầu quyền Lập trình viên; mỗi người tối đa một yêu cầu đang chờ.
CREATE TABLE role_requests (
    id            UUID        PRIMARY KEY,
    subject       UUID        NOT NULL,
    email         TEXT        NOT NULL,
    note          TEXT        NOT NULL DEFAULT '',
    status        TEXT        NOT NULL DEFAULT 'pending'
                              CHECK (status IN ('pending', 'approved', 'rejected')),
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_by    TEXT,
    decided_at    TIMESTAMPTZ,
    decision_note TEXT
);
CREATE UNIQUE INDEX role_requests_one_pending ON role_requests (subject) WHERE status = 'pending';
CREATE INDEX role_requests_status ON role_requests (status, created_at);
