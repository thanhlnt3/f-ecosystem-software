-- Nhóm task 8 (spec developer-portal).

CREATE TABLE developer_accounts (
    id                    UUID        PRIMARY KEY,
    kind                  TEXT        NOT NULL CHECK (kind IN ('individual', 'organization')),
    display_name          TEXT        NOT NULL,
    -- Tổ chức: tên pháp lý, mã số doanh nghiệp, tên miền.
    legal_name            TEXT,
    business_id           TEXT,
    domain                TEXT,
    -- Giá trị bản ghi TXT dùng để xác minh tên miền: feco-domain-verification=<domain_token>.
    domain_token          TEXT        NOT NULL,
    domain_verified_at    TIMESTAMPTZ,
    -- Thỏa thuận nhà phát triển: phiên bản và thời điểm chấp nhận.
    agreement_version     TEXT,
    agreement_accepted_at TIMESTAMPTZ,
    agreement_accepted_by UUID,
    created_at            TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE TABLE developer_members (
    account_id UUID        NOT NULL REFERENCES developer_accounts (id) ON DELETE CASCADE,
    email      TEXT        NOT NULL,
    subject    UUID,
    role       TEXT        NOT NULL CHECK (role IN ('owner', 'admin', 'developer', 'viewer')),
    status     TEXT        NOT NULL CHECK (status IN ('invited', 'active')),
    invited_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    joined_at  TIMESTAMPTZ,
    PRIMARY KEY (account_id, email)
);
-- Mỗi người dùng thuộc tối đa một tài khoản nhà phát triển.
CREATE UNIQUE INDEX developer_members_subject ON developer_members (subject) WHERE subject IS NOT NULL;

-- Token cho CLI `f-dev`; chỉ lưu mã băm SHA-256.
CREATE TABLE developer_tokens (
    id           UUID        PRIMARY KEY,
    account_id   UUID        NOT NULL REFERENCES developer_accounts (id) ON DELETE CASCADE,
    subject      UUID        NOT NULL,
    name         TEXT        NOT NULL,
    token_hash   TEXT        NOT NULL UNIQUE,
    created_at   TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_used_at TIMESTAMPTZ,
    revoked_at   TIMESTAMPTZ
);

CREATE TABLE developer_apps (
    app_id     TEXT        PRIMARY KEY CHECK (app_id ~ '^[a-z0-9][a-z0-9._-]{0,62}$'),
    account_id UUID        NOT NULL REFERENCES developer_accounts (id),
    client_id  TEXT        NOT NULL,
    name       TEXT        NOT NULL,
    -- desktop: client công khai dùng PKCE; server: có client secret xoay vòng được.
    kind       TEXT        NOT NULL CHECK (kind IN ('desktop', 'server')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ
);

CREATE TABLE app_versions (
    id              UUID        PRIMARY KEY,
    app_id          TEXT        NOT NULL REFERENCES developer_apps (app_id),
    version         TEXT        NOT NULL,
    manifest        JSONB       NOT NULL,
    manifest_toml   TEXT        NOT NULL,
    status          TEXT        NOT NULL DEFAULT 'draft'
        CHECK (status IN ('draft', 'pending_review', 'approved', 'rejected', 'released', 'paused')),
    channel         TEXT        CHECK (channel IN ('stable', 'beta')),
    rollout_percent INT         NOT NULL DEFAULT 0 CHECK (rollout_percent BETWEEN 0 AND 100),
    created_by      UUID        NOT NULL,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT now(),
    submitted_at    TIMESTAMPTZ,
    reviewed_at     TIMESTAMPTZ,
    review_note     TEXT,
    released_at     TIMESTAMPTZ,
    UNIQUE (app_id, version)
);

CREATE TABLE app_packages (
    version_id   UUID        NOT NULL REFERENCES app_versions (id) ON DELETE CASCADE,
    platform     TEXT        NOT NULL,
    sha256       TEXT        NOT NULL,
    size_bytes   BIGINT      NOT NULL,
    storage_path TEXT        NOT NULL,
    uploaded_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (version_id, platform)
);

-- Số liệu tổng hợp cho nhà phát triển. app_activity chỉ dùng để đếm người dùng hoạt động;
-- không bao giờ hiện danh tính cho nhà phát triển.
CREATE TABLE app_activity (
    app_id  TEXT NOT NULL,
    subject UUID NOT NULL,
    day     DATE NOT NULL,
    PRIMARY KEY (app_id, subject, day)
);
CREATE TABLE app_crashes (
    app_id  TEXT   NOT NULL,
    version TEXT   NOT NULL,
    day     DATE   NOT NULL,
    count   BIGINT NOT NULL DEFAULT 0,
    PRIMARY KEY (app_id, version, day)
);

ALTER TABLE catalog_installs ADD COLUMN version TEXT;
ALTER TABLE catalog_versions
    ADD COLUMN channel TEXT NOT NULL DEFAULT 'stable' CHECK (channel IN ('stable', 'beta')),
    ADD COLUMN rollout_percent INT NOT NULL DEFAULT 100 CHECK (rollout_percent BETWEEN 0 AND 100);
