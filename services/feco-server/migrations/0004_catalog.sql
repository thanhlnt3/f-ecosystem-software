-- Nhóm task 7 (spec app-catalog).

-- App trong danh mục. Scope dữ liệu và mục đích lấy từ OAuth client trong Hydra (client_id).
CREATE TABLE catalog_apps (
    id                 TEXT        PRIMARY KEY CHECK (id ~ '^[a-z0-9][a-z0-9._-]{0,62}$'),
    client_id          TEXT        NOT NULL,
    name               TEXT        NOT NULL,
    developer          TEXT        NOT NULL,
    developer_verified BOOLEAN     NOT NULL DEFAULT false,
    -- Danh tính F-Eco của nhà phát triển (trả lời nhận xét). Cổng nhà phát triển (nhóm 8) sẽ quản lý.
    owner_subject      UUID,
    first_party        BOOLEAN     NOT NULL DEFAULT false,
    summary            TEXT        NOT NULL DEFAULT '',
    description        TEXT        NOT NULL DEFAULT '',
    category           TEXT        NOT NULL DEFAULT 'other',
    price_vnd          BIGINT      NOT NULL DEFAULT 0 CHECK (price_vnd >= 0),
    license            TEXT        NOT NULL,
    screenshots        TEXT[]      NOT NULL DEFAULT '{}',
    -- Tên, mô tả, nhà phát triển đã bỏ dấu và viết thường, để tìm không phân biệt dấu.
    search_text        TEXT        NOT NULL,
    status             TEXT        NOT NULL DEFAULT 'listed' CHECK (status IN ('listed', 'removed')),
    removed_reason     TEXT,
    removed_at         TIMESTAMPTZ,
    created_at         TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at         TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX catalog_apps_listed ON catalog_apps (status, category);

-- Phiên bản đã qua kiểm duyệt (nhóm 9 ghi vào đây sau khi duyệt; tuf_target là gói đã ký).
CREATE TABLE catalog_versions (
    app_id      TEXT        NOT NULL REFERENCES catalog_apps (id) ON DELETE CASCADE,
    version     TEXT        NOT NULL,
    platform    TEXT        NOT NULL,
    size_bytes  BIGINT      NOT NULL CHECK (size_bytes >= 0),
    tuf_target  TEXT        NOT NULL,
    changelog   TEXT        NOT NULL DEFAULT '',
    status      TEXT        NOT NULL DEFAULT 'released' CHECK (status IN ('released', 'revoked')),
    released_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app_id, version, platform)
);

-- Người dùng đã cài app (launcher báo sau khi cài thành công). Điều kiện để đánh giá.
CREATE TABLE catalog_installs (
    subject      UUID        NOT NULL,
    app_id       TEXT        NOT NULL REFERENCES catalog_apps (id) ON DELETE CASCADE,
    installed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject, app_id)
);

-- Mỗi người một nhận xét cho mỗi app (viết lại thì cập nhật).
CREATE TABLE catalog_reviews (
    id            UUID        PRIMARY KEY,
    app_id        TEXT        NOT NULL REFERENCES catalog_apps (id) ON DELETE CASCADE,
    subject       UUID        NOT NULL,
    author_name   TEXT        NOT NULL,
    rating        SMALLINT    NOT NULL CHECK (rating BETWEEN 1 AND 5),
    body          TEXT        NOT NULL CHECK (length(body) <= 4000),
    hidden        BOOLEAN     NOT NULL DEFAULT false,
    reply_body    TEXT        CHECK (length(reply_body) <= 4000),
    reply_at      TIMESTAMPTZ,
    created_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at    TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (app_id, subject)
);
CREATE INDEX catalog_reviews_app ON catalog_reviews (app_id, created_at DESC);

-- Báo cáo vi phạm: vào hàng chờ của quản trị viên; người báo cáo nhận email kết quả.
CREATE TABLE catalog_reports (
    id          UUID        PRIMARY KEY,
    reporter    UUID        NOT NULL,
    app_id      TEXT        NOT NULL REFERENCES catalog_apps (id) ON DELETE CASCADE,
    review_id   UUID        REFERENCES catalog_reviews (id) ON DELETE CASCADE,
    reason      TEXT        NOT NULL CHECK (reason IN ('malware', 'fraud', 'copyright', 'inappropriate', 'other')),
    details     TEXT        NOT NULL DEFAULT '' CHECK (length(details) <= 4000),
    status      TEXT        NOT NULL DEFAULT 'open' CHECK (status IN ('open', 'upheld', 'rejected')),
    resolution  TEXT,
    created_at  TIMESTAMPTZ NOT NULL DEFAULT now(),
    resolved_at TIMESTAMPTZ
);
CREATE INDEX catalog_reports_open ON catalog_reports (status, created_at);
