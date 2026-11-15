-- Thay đổi OpenSpec fcoin-wallet-and-marketplace: sáu vai trò, ví F-Coin, tỷ giá, nạp, rút, mua app,
-- dự án phần mềm và hợp đồng.

-- ---------- R1. Sáu vai trò ----------
ALTER TABLE role_grants DROP CONSTRAINT role_grants_role_check;
ALTER TABLE role_grants ADD CONSTRAINT role_grants_role_check
    CHECK (role IN ('customer', 'business_customer', 'developer', 'dev_company'));

ALTER TABLE role_requests
    ADD COLUMN role       TEXT NOT NULL DEFAULT 'developer'
        CHECK (role IN ('customer', 'business_customer', 'developer', 'dev_company')),
    ADD COLUMN legal_name TEXT,
    ADD COLUMN tax_code   TEXT;

-- ---------- R2. Sổ cái ghi kép ----------
CREATE TABLE wallet_accounts (
    id            UUID        PRIMARY KEY,
    kind          TEXT        NOT NULL CHECK (kind IN ('user', 'system')),
    -- Ví người dùng: mã người dùng; tài khoản hệ thống: issuance, escrow, withdrawals, fees.
    subject       UUID        UNIQUE,
    code          TEXT        UNIQUE,
    email         TEXT,
    balance       BIGINT      NOT NULL DEFAULT 0,
    frozen_at     TIMESTAMPTZ,
    frozen_reason TEXT,
    frozen_by     TEXT,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((kind = 'user') = (subject IS NOT NULL)),
    CHECK ((kind = 'system') = (code IS NOT NULL)),
    -- Số dư ví người dùng không bao giờ âm, kể cả khi mã có lỗi.
    CONSTRAINT wallet_user_nonnegative CHECK (kind <> 'user' OR balance >= 0)
);

INSERT INTO wallet_accounts (id, kind, code) VALUES
    (gen_random_uuid(), 'system', 'issuance'),
    (gen_random_uuid(), 'system', 'escrow'),
    (gen_random_uuid(), 'system', 'withdrawals'),
    (gen_random_uuid(), 'system', 'fees');

CREATE SEQUENCE wallet_txn_seq START 1;

CREATE TABLE wallet_txns (
    id         UUID        PRIMARY KEY,
    -- Mã cho người dùng, ví dụ GD-000123.
    code       TEXT        NOT NULL UNIQUE,
    kind       TEXT        NOT NULL CHECK (kind IN (
                   'topup', 'withdrawal_hold', 'withdrawal_release', 'withdrawal_paid',
                   'purchase', 'escrow_fund', 'escrow_release', 'escrow_refund')),
    memo       TEXT        NOT NULL DEFAULT '',
    -- Tham chiếu nghiệp vụ: mã lệnh nạp, lệnh rút, app, dự án.
    reference  TEXT,
    idem_key   TEXT        NOT NULL UNIQUE,
    created_by TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX wallet_txns_created ON wallet_txns (created_at);

CREATE TABLE wallet_entries (
    id            BIGSERIAL   PRIMARY KEY,
    txn_id        UUID        NOT NULL REFERENCES wallet_txns (id),
    account_id    UUID        NOT NULL REFERENCES wallet_accounts (id),
    amount        BIGINT      NOT NULL CHECK (amount <> 0),
    balance_after BIGINT      NOT NULL,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX wallet_entries_account ON wallet_entries (account_id, id DESC);
CREATE INDEX wallet_entries_txn ON wallet_entries (txn_id);

-- ---------- R3. Tỷ giá và phí ----------
CREATE TABLE coin_rates (
    id           BIGSERIAL   PRIMARY KEY,
    vnd_per_coin BIGINT      NOT NULL CHECK (vnd_per_coin BETWEEN 1 AND 1000000),
    set_by       TEXT        NOT NULL,
    reason       TEXT        NOT NULL,
    set_at       TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO coin_rates (vnd_per_coin, set_by, reason)
VALUES (1000, 'migration', 'Tỷ giá ban đầu: 1 F-Coin = 1.000 ₫');

CREATE TABLE wallet_settings (
    key        TEXT        PRIMARY KEY CHECK (key IN ('app_fee_bps', 'project_fee_bps')),
    value      BIGINT      NOT NULL CHECK (value BETWEEN 0 AND 3000),
    updated_by TEXT        NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
INSERT INTO wallet_settings (key, value, updated_by) VALUES
    ('app_fee_bps', 0, 'migration'),
    ('project_fee_bps', 0, 'migration');

-- ---------- R4. Nạp ----------
CREATE SEQUENCE topup_seq START 1;

CREATE TABLE wallet_topups (
    id           UUID        PRIMARY KEY,
    code         TEXT        NOT NULL UNIQUE,
    subject      UUID        NOT NULL,
    email        TEXT        NOT NULL,
    coins        BIGINT      NOT NULL CHECK (coins > 0),
    vnd_per_coin BIGINT      NOT NULL,
    amount_vnd   BIGINT      NOT NULL CHECK (amount_vnd > 0),
    status       TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'succeeded', 'cancelled')),
    txn_id       UUID        REFERENCES wallet_txns (id),
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    settled_at   TIMESTAMPTZ
);
CREATE INDEX wallet_topups_subject ON wallet_topups (subject, created_at DESC);

ALTER TABLE payment_requests ALTER COLUMN order_id DROP NOT NULL;
ALTER TABLE payment_requests ADD COLUMN topup_id UUID REFERENCES wallet_topups (id) ON DELETE CASCADE;
ALTER TABLE payment_requests ADD CONSTRAINT payment_requests_owner
    CHECK ((order_id IS NULL) <> (topup_id IS NULL));
CREATE UNIQUE INDEX payment_requests_topup ON payment_requests (topup_id) WHERE topup_id IS NOT NULL;

-- ---------- R5. Rút ----------
CREATE SEQUENCE withdrawal_seq START 1;

CREATE TABLE wallet_withdrawals (
    id             UUID        PRIMARY KEY,
    code           TEXT        NOT NULL UNIQUE,
    subject        UUID        NOT NULL,
    email          TEXT        NOT NULL,
    coins          BIGINT      NOT NULL CHECK (coins > 0),
    vnd_per_coin   BIGINT      NOT NULL,
    amount_vnd     BIGINT      NOT NULL CHECK (amount_vnd > 0),
    method         TEXT        NOT NULL CHECK (method IN ('source', 'bank')),
    -- Hoàn về giao dịch nạp: giao dịch cổng thanh toán của lần nạp.
    source_tx      UUID        REFERENCES payment_transactions (id),
    -- Chuyển khoản: số tài khoản mã hóa (crypto::DataKey), chỉ hiện 4 số cuối.
    bank_name      TEXT,
    account_holder TEXT,
    account_last4  TEXT,
    account_enc    TEXT,
    note           TEXT        NOT NULL DEFAULT '',
    status         TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN
                       ('pending', 'approved', 'processing', 'paid', 'rejected', 'cancelled', 'failed')),
    approvals      TEXT[]      NOT NULL DEFAULT '{}',
    decision_note  TEXT,
    payout_ref     TEXT,
    refund_id      UUID,
    created_at     TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at     TIMESTAMPTZ,
    paid_at        TIMESTAMPTZ,
    CHECK ((method = 'source') = (source_tx IS NOT NULL)),
    CHECK (method <> 'bank' OR (bank_name IS NOT NULL AND account_holder IS NOT NULL AND account_enc IS NOT NULL))
);
CREATE INDEX wallet_withdrawals_subject ON wallet_withdrawals (subject, created_at DESC);
CREATE INDEX wallet_withdrawals_open ON wallet_withdrawals (status) WHERE status IN ('pending', 'approved', 'processing');

ALTER TABLE refunds DROP CONSTRAINT refunds_kind_check;
ALTER TABLE refunds ADD CONSTRAINT refunds_kind_check
    CHECK (kind IN ('manual', 'order', 'double_charge', 'withdrawal'));
ALTER TABLE refunds ADD COLUMN withdrawal_id UUID REFERENCES wallet_withdrawals (id);

-- ---------- R6. Giá app và mua app ----------
ALTER TABLE catalog_apps ADD COLUMN price_coin BIGINT NOT NULL DEFAULT 0 CHECK (price_coin >= 0);
UPDATE catalog_apps SET price_coin = (price_vnd + 999) / 1000;
ALTER TABLE developer_apps ADD COLUMN price_coin BIGINT NOT NULL DEFAULT 0
    CHECK (price_coin BETWEEN 0 AND 1000000);

CREATE TABLE app_purchases (
    id         UUID        PRIMARY KEY,
    app_id     TEXT        NOT NULL,
    buyer      UUID        NOT NULL,
    price_coin BIGINT      NOT NULL CHECK (price_coin > 0),
    fee_coin   BIGINT      NOT NULL DEFAULT 0,
    seller     UUID,
    txn_id     UUID        NOT NULL REFERENCES wallet_txns (id),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (app_id, buyer)
);

-- ---------- R7. Dự án phần mềm ----------
CREATE TABLE provider_profiles (
    subject      UUID        PRIMARY KEY,
    email        TEXT        NOT NULL,
    kind         TEXT        NOT NULL CHECK (kind IN ('individual', 'company')),
    display_name TEXT        NOT NULL,
    headline     TEXT        NOT NULL DEFAULT '',
    skills       TEXT        NOT NULL DEFAULT '',
    accepting    BOOLEAN     NOT NULL DEFAULT true,
    search_text  TEXT        NOT NULL DEFAULT '',
    updated_at   TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE SEQUENCE project_seq START 1;

CREATE TABLE projects (
    id                    UUID        PRIMARY KEY,
    code                  TEXT        NOT NULL UNIQUE,
    customer              UUID        NOT NULL,
    customer_email        TEXT        NOT NULL,
    provider              UUID        NOT NULL,
    provider_email        TEXT        NOT NULL,
    title                 TEXT        NOT NULL,
    description           TEXT        NOT NULL,
    budget_coin           BIGINT      CHECK (budget_coin IS NULL OR budget_coin > 0),
    wanted_days           INT         CHECK (wanted_days IS NULL OR wanted_days > 0),
    price_coin            BIGINT      CHECK (price_coin IS NULL OR price_coin > 0),
    days                  INT,
    status                TEXT        NOT NULL DEFAULT 'requested' CHECK (status IN
                              ('requested', 'negotiating', 'agreed', 'in_progress', 'delivered',
                               'completed', 'disputed', 'resolved', 'cancelled', 'refunded')),
    escrow_coin           BIGINT      NOT NULL DEFAULT 0 CHECK (escrow_coin >= 0),
    status_before_dispute TEXT,
    delivery_note         TEXT,
    delivery_link         TEXT,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at            TIMESTAMPTZ NOT NULL DEFAULT now(),
    funded_at             TIMESTAMPTZ,
    delivered_at          TIMESTAMPTZ,
    closed_at             TIMESTAMPTZ,
    CHECK (customer <> provider)
);
CREATE INDEX projects_customer ON projects (customer, updated_at DESC);
CREATE INDEX projects_provider ON projects (provider, updated_at DESC);
CREATE INDEX projects_status ON projects (status);

CREATE TABLE project_offers (
    id         UUID        PRIMARY KEY,
    project_id UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    side       TEXT        NOT NULL CHECK (side IN ('customer', 'provider')),
    price_coin BIGINT      NOT NULL CHECK (price_coin BETWEEN 1 AND 100000000),
    days       INT         NOT NULL CHECK (days BETWEEN 1 AND 3650),
    note       TEXT        NOT NULL DEFAULT '',
    status     TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'accepted', 'superseded')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at TIMESTAMPTZ
);
CREATE INDEX project_offers_project ON project_offers (project_id, created_at);

CREATE TABLE project_events (
    id          BIGSERIAL   PRIMARY KEY,
    project_id  UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    from_status TEXT,
    to_status   TEXT        NOT NULL,
    actor       TEXT        NOT NULL,
    note        TEXT        NOT NULL DEFAULT '',
    at          TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX project_events_project ON project_events (project_id, id);

CREATE TABLE project_messages (
    id         BIGSERIAL   PRIMARY KEY,
    project_id UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    side       TEXT        NOT NULL CHECK (side IN ('customer', 'provider', 'admin', 'system')),
    body       TEXT        NOT NULL,
    at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX project_messages_project ON project_messages (project_id, id);

-- ---------- R8. Hợp đồng ----------
CREATE SEQUENCE contract_seq START 1;

CREATE TABLE project_contracts (
    id                 UUID        PRIMARY KEY,
    number             TEXT        NOT NULL UNIQUE,
    project_id         UUID        NOT NULL REFERENCES projects (id) ON DELETE CASCADE,
    created_by         UUID        NOT NULL,
    creator_side       TEXT        NOT NULL CHECK (creator_side IN ('customer', 'provider')),
    -- Họ tên (hoặc tên pháp lý) đại diện mỗi bên, phải nhập đúng khi ký.
    customer_name      TEXT        NOT NULL,
    provider_name      TEXT        NOT NULL,
    body               TEXT        NOT NULL,
    sha256             TEXT        NOT NULL,
    status             TEXT        NOT NULL DEFAULT 'draft' CHECK (status IN ('draft', 'signed', 'void')),
    customer_signed_at TIMESTAMPTZ,
    provider_signed_at TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE UNIQUE INDEX project_contracts_one_active ON project_contracts (project_id) WHERE status <> 'void';
