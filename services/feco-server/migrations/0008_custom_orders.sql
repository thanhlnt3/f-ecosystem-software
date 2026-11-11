-- Nhóm task 12 (spec custom-build-orders): đơn xây app theo yêu cầu.

CREATE SEQUENCE order_code_seq START 1;

CREATE TABLE orders (
    id                    UUID        PRIMARY KEY,
    -- Mã đơn cho khách hàng, ví dụ DH-000123.
    code                  TEXT        NOT NULL UNIQUE,
    customer              UUID        NOT NULL,
    customer_email        TEXT        NOT NULL,
    title                 TEXT        NOT NULL,
    description           TEXT        NOT NULL,
    platforms             TEXT[]      NOT NULL DEFAULT '{}',
    wanted_by             DATE,
    budget_vnd            BIGINT      CHECK (budget_vnd IS NULL OR budget_vnd >= 0),
    status                TEXT        NOT NULL DEFAULT 'draft' CHECK (status IN
                              ('draft', 'submitted', 'quoted', 'in_progress', 'delivered', 'completed', 'cancelled', 'disputed')),
    -- Trạng thái trước khi mở tranh chấp, để quay về khi giải quyết xong.
    status_before_dispute TEXT,
    -- Điều khoản dịch vụ xây app khách đã chấp nhận (phiên bản, thời điểm).
    tos_version           TEXT,
    tos_accepted_at       TIMESTAMPTZ,
    -- Hạn gửi báo giá (3 ngày làm việc kể từ khi gửi).
    quote_due_at          TIMESTAMPTZ,
    -- Ngày đặt cọc thành công: thời hạn các mốc tính từ đây.
    started_at            TIMESTAMPTZ,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX orders_customer ON orders (customer, created_at DESC);
CREATE INDEX orders_status ON orders (status);

-- Lịch sử chuyển trạng thái: ai, khi nào, từ đâu sang đâu.
CREATE TABLE order_events (
    id          BIGSERIAL   PRIMARY KEY,
    order_id    UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    from_status TEXT,
    to_status   TEXT        NOT NULL,
    actor       TEXT        NOT NULL,
    note        TEXT        NOT NULL DEFAULT '',
    at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX order_events_order ON order_events (order_id, id);

-- Tin nhắn trong đơn; kind phân biệt tin thường với đề nghị điều chỉnh, yêu cầu sửa, tranh chấp.
CREATE TABLE order_messages (
    id         BIGSERIAL   PRIMARY KEY,
    order_id   UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    author     TEXT        NOT NULL CHECK (author IN ('customer', 'team', 'system')),
    author_name TEXT       NOT NULL DEFAULT '',
    kind       TEXT        NOT NULL DEFAULT 'message' CHECK (kind IN ('message', 'adjustment', 'revision', 'dispute')),
    body       TEXT        NOT NULL,
    at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX order_messages_order ON order_messages (order_id, id);

-- Tệp đính kèm của yêu cầu hoặc của tin nhắn (tổng mỗi đơn tối đa 200 MB khi gửi yêu cầu).
CREATE TABLE order_files (
    id         UUID        PRIMARY KEY,
    order_id   UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    message_id BIGINT      REFERENCES order_messages (id) ON DELETE SET NULL,
    name       TEXT        NOT NULL,
    size       BIGINT      NOT NULL,
    sha256     TEXT        NOT NULL,
    path       TEXT        NOT NULL,
    at         TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE order_quotes (
    id          UUID        PRIMARY KEY,
    order_id    UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    number      INT         NOT NULL,
    scope       TEXT        NOT NULL,
    exclusions  TEXT        NOT NULL,
    -- Quyền sở hữu hoặc quyền sử dụng mã nguồn, hiện trước khi khách chấp nhận.
    ownership   TEXT        NOT NULL,
    valid_until TIMESTAMPTZ NOT NULL,
    status      TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'superseded')),
    created_by  TEXT        NOT NULL,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    accepted_at TIMESTAMPTZ,
    UNIQUE (order_id, number)
);

CREATE TABLE order_milestones (
    id              UUID        PRIMARY KEY,
    quote_id        UUID        NOT NULL REFERENCES order_quotes (id) ON DELETE CASCADE,
    order_id        UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    position        INT         NOT NULL,
    title           TEXT        NOT NULL,
    deliverable     TEXT        NOT NULL,
    -- VND, đã gồm thuế.
    amount_vnd      BIGINT      NOT NULL CHECK (amount_vnd > 0),
    -- Thời hạn: số ngày tính từ ngày đặt cọc thành công.
    days            INT         NOT NULL CHECK (days > 0),
    free_revisions  INT         NOT NULL DEFAULT 2 CHECK (free_revisions >= 0),
    revisions_used  INT         NOT NULL DEFAULT 0,
    status          TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN
                        ('pending', 'awaiting_payment', 'in_progress', 'delivered', 'accepted', 'refunded')),
    paid            BOOLEAN     NOT NULL DEFAULT false,
    due_at          TIMESTAMPTZ,
    delivered_at    TIMESTAMPTZ,
    reminded_at     TIMESTAMPTZ,
    accepted_at     TIMESTAMPTZ,
    -- Bản build bàn giao: app riêng tư trong danh mục, phiên bản, ghi chú, kết quả kiểm tra.
    app_id          TEXT,
    app_version     TEXT,
    notes           TEXT        NOT NULL DEFAULT '',
    check_report    TEXT        NOT NULL DEFAULT '',
    UNIQUE (quote_id, position)
);

-- Yêu cầu thanh toán (thu hoặc hoàn) mà cổng thanh toán (nhóm 13) xử lý.
CREATE TABLE payment_requests (
    id           UUID        PRIMARY KEY,
    order_id     UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    milestone_id UUID        REFERENCES order_milestones (id) ON DELETE SET NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('charge', 'refund')),
    amount_vnd   BIGINT      NOT NULL CHECK (amount_vnd > 0),
    status       TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'cancelled', 'failed')),
    reason       TEXT        NOT NULL DEFAULT '',
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at   TIMESTAMPTZ
);
CREATE INDEX payment_requests_order ON payment_requests (order_id);

CREATE TABLE order_disputes (
    id           UUID        PRIMARY KEY,
    order_id     UUID        NOT NULL REFERENCES orders (id) ON DELETE CASCADE,
    milestone_id UUID        NOT NULL REFERENCES order_milestones (id) ON DELETE CASCADE,
    reason       TEXT        NOT NULL,
    opened_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    -- Quản trị viên phản hồi trong 2 ngày làm việc.
    respond_by   TIMESTAMPTZ NOT NULL,
    resolution   TEXT        CHECK (resolution IN ('continue', 'partial_refund', 'full_refund')),
    refund_vnd   BIGINT,
    resolved_by  TEXT,
    resolved_at  TIMESTAMPTZ
);

-- Bản build bàn giao là app riêng tư: chỉ khách hàng của đơn thấy trong danh mục.
ALTER TABLE catalog_apps ADD COLUMN private_to UUID;
