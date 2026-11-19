-- Chat giữa khách hàng và nhà phát triển (thay đổi OpenSpec chat-and-source-scan, design C1).

CREATE TABLE conversations (
    id                    UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    kind                  TEXT        NOT NULL CHECK (kind IN ('direct', 'project')),
    -- Người bắt đầu (trực tiếp) hoặc khách hàng của dự án.
    customer              UUID        NOT NULL,
    -- Nhà phát triển (trực tiếp) hoặc bên nhận dự án.
    developer             UUID        NOT NULL,
    project_id            UUID        UNIQUE REFERENCES projects (id) ON DELETE CASCADE,
    -- App trong F-Store mà khách hàng bấm "Nhắn nhà phát triển" (ngữ cảnh).
    app_id                TEXT,
    -- Tên hiển thị của hai bên lúc tạo hội thoại (danh sách hội thoại không phải hỏi Kratos).
    customer_name         TEXT        NOT NULL DEFAULT '',
    developer_name        TEXT        NOT NULL DEFAULT '',
    -- Nhà phát triển chặn (chỉ loại direct).
    blocked_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_message_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Id tin cuối cùng mỗi bên đã đọc.
    customer_read         BIGINT      NOT NULL DEFAULT 0,
    developer_read        BIGINT      NOT NULL DEFAULT 0,
    -- Lần cuối báo "có tin nhắn mới" cho mỗi bên (gộp thông báo 15 phút).
    customer_notified_at  TIMESTAMPTZ,
    developer_notified_at TIMESTAMPTZ,
    CHECK (customer <> developer),
    CHECK ((kind = 'project') = (project_id IS NOT NULL))
);
CREATE UNIQUE INDEX conversations_direct ON conversations (customer, developer) WHERE kind = 'direct';
CREATE INDEX conversations_customer ON conversations (customer, last_message_at DESC);
CREATE INDEX conversations_developer ON conversations (developer, last_message_at DESC);
-- Đếm hội thoại trực tiếp mới trong 24 giờ.
CREATE INDEX conversations_created ON conversations (customer, created_at) WHERE kind = 'direct';

CREATE TABLE chat_messages (
    id              BIGSERIAL   PRIMARY KEY,
    conversation_id UUID        NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    -- NULL với tin hệ thống (và tin quản trị chuyển từ project_messages).
    sender          UUID,
    side            TEXT        NOT NULL CHECK (side IN ('customer', 'developer', 'admin', 'system')),
    body            TEXT        NOT NULL CHECK (char_length(body) <= 5000),
    at              TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX chat_messages_conv ON chat_messages (conversation_id, id);
-- Giới hạn 30 tin mỗi phút mỗi người.
CREATE INDEX chat_messages_sender ON chat_messages (sender, at) WHERE sender IS NOT NULL;

CREATE TABLE chat_attachments (
    id           UUID        PRIMARY KEY DEFAULT gen_random_uuid(),
    message_id   BIGINT      NOT NULL REFERENCES chat_messages (id) ON DELETE CASCADE,
    name         TEXT        NOT NULL,
    size         BIGINT      NOT NULL,
    sha256       TEXT        NOT NULL,
    path         TEXT        NOT NULL,
    scan_status  TEXT        NOT NULL DEFAULT 'pending'
                     CHECK (scan_status IN ('pending', 'clean', 'infected', 'error')),
    scan_note    TEXT,
    scanned_at   TIMESTAMPTZ,
    -- Số lần quét (engine lỗi thì quét lại, tối đa vài lần).
    scan_attempts INT        NOT NULL DEFAULT 0
);
CREATE INDEX chat_attachments_message ON chat_attachments (message_id);
CREATE INDEX chat_attachments_unscanned ON chat_attachments (scan_status)
    WHERE scan_status IN ('pending', 'error');

CREATE TABLE chat_reports (
    id              BIGSERIAL   PRIMARY KEY,
    conversation_id UUID        NOT NULL REFERENCES conversations (id) ON DELETE CASCADE,
    reporter        UUID        NOT NULL,
    reason          TEXT        NOT NULL,
    at              TIMESTAMPTZ NOT NULL DEFAULT now(),
    handled_at      TIMESTAMPTZ,
    handled_by      TEXT
);
CREATE INDEX chat_reports_open ON chat_reports (at) WHERE handled_at IS NULL;

-- Nhóm thông báo tùy chọn "Tin nhắn" (design C6).
ALTER TABLE notification_preferences DROP CONSTRAINT notification_preferences_category_check;
ALTER TABLE notification_preferences ADD CONSTRAINT notification_preferences_category_check
    CHECK (category IN ('app_updates', 'recommendations', 'newsletter', 'messages'));

-- Mỗi dự án có một hội thoại; tin cũ trong project_messages được chép sang và coi như đã đọc.
INSERT INTO conversations (kind, customer, developer, project_id, customer_name, developer_name,
                           created_at, last_message_at)
SELECT 'project', p.customer, p.provider, p.id, p.customer_email,
       COALESCE(NULLIF(pp.display_name, ''), p.provider_email), p.created_at,
       COALESCE((SELECT max(m.at) FROM project_messages m WHERE m.project_id = p.id), p.created_at)
FROM projects p
LEFT JOIN provider_profiles pp ON pp.subject = p.provider;

INSERT INTO chat_messages (conversation_id, sender, side, body, at)
SELECT c.id,
       CASE m.side WHEN 'customer' THEN p.customer WHEN 'provider' THEN p.provider END,
       CASE m.side WHEN 'provider' THEN 'developer' ELSE m.side END,
       left(m.body, 5000),
       m.at
FROM project_messages m
JOIN projects p ON p.id = m.project_id
JOIN conversations c ON c.project_id = p.id
ORDER BY m.id;

UPDATE conversations c
SET customer_read = x.last_id, developer_read = x.last_id
FROM (SELECT conversation_id, max(id) AS last_id FROM chat_messages GROUP BY conversation_id) x
WHERE x.conversation_id = c.id;
