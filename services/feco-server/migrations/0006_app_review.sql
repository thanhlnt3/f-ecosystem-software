-- Nhóm task 9 (spec app-review).

ALTER TABLE app_versions DROP CONSTRAINT app_versions_status_check;
ALTER TABLE app_versions ADD CONSTRAINT app_versions_status_check CHECK (status IN
    ('draft', 'pending_review', 'approved', 'rejected', 'released', 'paused', 'revoked'));
ALTER TABLE app_versions
    -- checks: đang chờ kiểm tra tự động; manual: chờ người duyệt; done: đã có kết quả.
    ADD COLUMN review_stage    TEXT CHECK (review_stage IN ('checks', 'manual', 'done')),
    -- Lý do phải duyệt thủ công (phiên bản đầu, thêm scope, thêm quyền).
    ADD COLUMN manual_reason   TEXT,
    ADD COLUMN checks          JSONB,
    ADD COLUMN reject_count    INT NOT NULL DEFAULT 0,
    ADD COLUMN reject_code     TEXT,
    ADD COLUMN last_scanned_at TIMESTAMPTZ;

ALTER TABLE developer_accounts
    ADD COLUMN suspended_at     TIMESTAMPTZ,
    ADD COLUMN suspended_reason TEXT;

CREATE TABLE review_jobs (
    id          BIGSERIAL   PRIMARY KEY,
    version_id  UUID        NOT NULL REFERENCES app_versions (id) ON DELETE CASCADE,
    kind        TEXT        NOT NULL CHECK (kind IN ('submit', 'rescan')),
    status      TEXT        NOT NULL DEFAULT 'queued' CHECK (status IN ('queued', 'running', 'done', 'failed')),
    attempts    INT         NOT NULL DEFAULT 0,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    started_at  TIMESTAMPTZ,
    finished_at TIMESTAMPTZ,
    result      JSONB
);
CREATE INDEX review_jobs_queued ON review_jobs (status, id);

-- Cơ sở dữ liệu mã độc của engine chữ ký F-Eco (SHA-256 của tệp), cập nhật liên tục.
CREATE TABLE malware_signatures (
    sha256   TEXT        PRIMARY KEY,
    name     TEXT        NOT NULL,
    added_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

-- Mỗi lần từ chối (reject_no) được kháng nghị một lần.
CREATE TABLE review_appeals (
    version_id  UUID        NOT NULL REFERENCES app_versions (id) ON DELETE CASCADE,
    reject_no   INT         NOT NULL,
    message     TEXT        NOT NULL CHECK (length(message) <= 4000),
    status      TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'upheld', 'overturned')),
    resolution  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ,
    PRIMARY KEY (version_id, reject_no)
);

-- Cảnh báo cho quản trị viên (mã độc, phát hiện muộn...). Bảng quản trị ở nhóm 14.
CREATE TABLE admin_alerts (
    id              BIGSERIAL   PRIMARY KEY,
    kind            TEXT        NOT NULL,
    message         TEXT        NOT NULL,
    details         JSONB       NOT NULL DEFAULT '{}',
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    acknowledged_at TIMESTAMPTZ
);
