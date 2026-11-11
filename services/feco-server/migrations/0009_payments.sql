-- Nhóm task 13 (spec payments): giao dịch qua cổng thanh toán, hoàn tiền, hóa đơn, đối soát.
--
-- `payment_requests` (migration 0008) là khoản phải thu hoặc phải hoàn của đơn hàng. Mỗi lần khách
-- bấm thanh toán tạo một giao dịch (`payment_transactions`) với cổng; khóa idempotency là
-- (yêu cầu thanh toán, lần thử). Một yêu cầu chỉ được thu thành công một lần.

CREATE TABLE payment_transactions (
    id               UUID        PRIMARY KEY,
    request_id       UUID        NOT NULL REFERENCES payment_requests (id) ON DELETE CASCADE,
    attempt          INT         NOT NULL,
    provider         TEXT        NOT NULL CHECK (provider IN ('zalopay', 'manual')),
    -- Phương thức khách chọn: vietqr, domestic_card, zalopay_wallet, international_card, manual.
    method           TEXT        NOT NULL,
    -- Mã giao dịch phía F-Eco gửi cho cổng (ZaloPay: app_trans_id, dạng yymmdd_...).
    gateway_ref      TEXT        NOT NULL UNIQUE,
    amount_vnd       BIGINT      NOT NULL CHECK (amount_vnd > 0),
    status           TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'failed', 'expired')),
    order_url        TEXT,
    -- Mã giao dịch của cổng (ZaloPay: zp_trans_id), kênh thanh toán, 4 số cuối thẻ nếu cổng trả về.
    gateway_trans_id TEXT,
    channel          INT,
    card_last4       TEXT        CHECK (card_last4 IS NULL OR card_last4 ~ '^[0-9]{4}$'),
    refunded_vnd     BIGINT      NOT NULL DEFAULT 0,
    created_at       TIMESTAMPTZ NOT NULL,
    expires_at       TIMESTAMPTZ NOT NULL,
    settled_at       TIMESTAMPTZ,
    UNIQUE (request_id, attempt)
);
CREATE INDEX payment_transactions_pending ON payment_transactions (expires_at) WHERE status = 'pending';
CREATE INDEX payment_transactions_created ON payment_transactions (created_at);

-- Mọi tin từ cổng (callback, truy vấn chủ động, trang chuyển hướng, đối soát), đã lọc dữ liệu thẻ.
CREATE TABLE payment_events (
    id          BIGSERIAL   PRIMARY KEY,
    gateway_ref TEXT        NOT NULL,
    source      TEXT        NOT NULL CHECK (source IN ('callback', 'query', 'redirect', 'reconcile', 'manual')),
    verified    BOOLEAN     NOT NULL,
    outcome     TEXT        NOT NULL,
    payload     JSONB       NOT NULL DEFAULT '{}',
    at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX payment_events_ref ON payment_events (gateway_ref, id);

CREATE TABLE refunds (
    id               UUID        PRIMARY KEY,
    transaction_id   UUID        NOT NULL REFERENCES payment_transactions (id) ON DELETE CASCADE,
    -- Yêu cầu hoàn của đơn hàng (hủy đơn, tranh chấp) nếu có.
    order_request_id UUID        REFERENCES payment_requests (id) ON DELETE SET NULL,
    amount_vnd       BIGINT      NOT NULL CHECK (amount_vnd > 0),
    reason           TEXT        NOT NULL,
    -- double_charge: hệ thống tự hoàn khi một mốc bị thu hai lần.
    kind             TEXT        NOT NULL DEFAULT 'manual' CHECK (kind IN ('manual', 'order', 'double_charge')),
    status           TEXT        NOT NULL CHECK (status IN ('pending_approval', 'processing', 'succeeded', 'failed')),
    requested_by     TEXT        NOT NULL,
    approvals        TEXT[]      NOT NULL DEFAULT '{}',
    gateway_ref      TEXT        UNIQUE,
    gateway_refund_id TEXT,
    failure          TEXT,
    created_at       TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at       TIMESTAMPTZ
);
CREATE INDEX refunds_open ON refunds (status) WHERE status IN ('pending_approval', 'processing');

-- Thông tin xuất hóa đơn của khách hàng (cá nhân, hoặc doanh nghiệp có mã số thuế).
CREATE TABLE billing_profiles (
    subject    UUID        PRIMARY KEY,
    kind       TEXT        NOT NULL CHECK (kind IN ('individual', 'company')),
    name       TEXT        NOT NULL,
    tax_code   TEXT,
    address    TEXT        NOT NULL DEFAULT '',
    email      TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE SEQUENCE invoice_seq START 1;

-- Hóa đơn sau giao dịch thành công (original) và hóa đơn điều chỉnh khi hoàn tiền (adjustment).
CREATE TABLE invoices (
    id             UUID        PRIMARY KEY,
    number         TEXT        NOT NULL UNIQUE,
    subject        UUID        NOT NULL,
    transaction_id UUID        NOT NULL REFERENCES payment_transactions (id) ON DELETE CASCADE,
    refund_id      UUID        UNIQUE REFERENCES refunds (id) ON DELETE CASCADE,
    kind           TEXT        NOT NULL CHECK (kind IN ('original', 'adjustment')),
    adjusts        UUID        REFERENCES invoices (id),
    buyer_kind     TEXT        NOT NULL,
    buyer_name     TEXT        NOT NULL,
    buyer_tax_code TEXT,
    buyer_address  TEXT        NOT NULL DEFAULT '',
    amount_vnd     BIGINT      NOT NULL,
    -- Nhà cung cấp hóa đơn điện tử; 'draft' là bản nháp nội bộ khi chưa tích hợp (tasks.md 13.4).
    provider       TEXT        NOT NULL,
    provider_ref   TEXT,
    document       TEXT        NOT NULL,
    issued_at      TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX invoices_one_original ON invoices (transaction_id) WHERE kind = 'original';
CREATE INDEX invoices_subject ON invoices (subject, issued_at DESC);

-- Đối soát hằng ngày: số giao dịch đã đối chiếu và các chênh lệch.
CREATE TABLE reconciliation_runs (
    day        DATE        PRIMARY KEY,
    checked    INT         NOT NULL,
    mismatches JSONB       NOT NULL,
    ran_at     TIMESTAMPTZ NOT NULL DEFAULT now()
);
