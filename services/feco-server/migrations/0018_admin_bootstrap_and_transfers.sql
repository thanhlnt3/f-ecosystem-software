-- Thay đổi production-readiness: tài khoản quản trị viên mặc định, quản trị viên chuyển F-Coin.

-- Tài khoản quản trị viên mặc định (FECO_ADMIN_EMAIL). Không bị hạ vai trò hay xóa.
CREATE TABLE system_admins (
    subject    UUID        PRIMARY KEY,
    email      TEXT        NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Hai loại bút toán mới: chuyển từ ví quản trị viên, cấp từ quỹ nền tảng (tài khoản issuance).
ALTER TABLE wallet_txns DROP CONSTRAINT wallet_txns_kind_check;
ALTER TABLE wallet_txns ADD CONSTRAINT wallet_txns_kind_check CHECK (kind IN (
    'topup', 'withdrawal_hold', 'withdrawal_release', 'withdrawal_paid',
    'purchase', 'escrow_fund', 'escrow_release', 'escrow_refund',
    'admin_transfer', 'admin_grant'));

CREATE SEQUENCE wallet_grant_seq START 1;

-- Yêu cầu cấp F-Coin từ quỹ nền tảng, từ 1.000.000 ₫ trở lên: chờ quản trị viên Tài chính
-- thứ hai duyệt.
CREATE TABLE wallet_grants (
    id              UUID        PRIMARY KEY,
    code            TEXT        NOT NULL UNIQUE,
    recipient       UUID        NOT NULL,
    recipient_email TEXT        NOT NULL,
    coins           BIGINT      NOT NULL CHECK (coins BETWEEN 1 AND 1000000),
    amount_vnd      BIGINT      NOT NULL CHECK (amount_vnd > 0),
    reason          TEXT        NOT NULL CHECK (length(reason) BETWEEN 1 AND 500),
    status          TEXT        NOT NULL DEFAULT 'pending'
                                CHECK (status IN ('pending', 'approved', 'rejected')),
    requested_by    TEXT        NOT NULL,
    decided_by      TEXT,
    decision_note   TEXT,
    txn_id          UUID        REFERENCES wallet_txns (id),
    idem_key        TEXT        NOT NULL UNIQUE,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    decided_at      TIMESTAMPTZ,
    CHECK (decided_by IS NULL OR decided_by <> requested_by)
);
CREATE INDEX wallet_grants_pending ON wallet_grants (created_at) WHERE status = 'pending';

-- Rà soát trước production: số người duyệt của lệnh rút tính theo tổng các lệnh rút của người đó
-- trong 24 giờ (chia nhỏ lệnh không né được quy tắc hai người duyệt). NULL: lệnh cũ, tính theo
-- số tiền của chính lệnh.
ALTER TABLE wallet_withdrawals ADD COLUMN approvals_required SMALLINT
    CHECK (approvals_required IS NULL OR approvals_required BETWEEN 1 AND 2);
