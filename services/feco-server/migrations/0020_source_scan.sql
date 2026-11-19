-- Thay đổi chat-and-source-scan, design S2: mã nguồn kèm phiên bản app, quét khi duyệt.

CREATE TABLE source_submissions (
    version_id   UUID        PRIMARY KEY REFERENCES app_versions (id) ON DELETE CASCADE,
    origin       TEXT        NOT NULL CHECK (origin IN ('upload', 'git')),
    -- origin git: https://github.com/<owner>/<repo>, commit đủ 40 ký tự hex.
    repository   TEXT,
    commit       TEXT,
    -- origin upload: tệp zip trong kho gói.
    storage_path TEXT,
    size_bytes   BIGINT,
    sha256       TEXT,
    status       TEXT        NOT NULL DEFAULT 'pending' CHECK (status IN ('pending', 'scanned', 'failed')),
    -- Báo cáo đầy đủ của feco-codescan (bí mật đã che).
    report       JSONB,
    scanned_at   TIMESTAMPTZ,
    submitted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CHECK ((origin = 'git') = (repository IS NOT NULL AND commit IS NOT NULL)),
    CHECK ((origin = 'upload') = (storage_path IS NOT NULL))
);

-- Job kiểm duyệt xếp lại sau lỗi tạm thời (tải mã nguồn thất bại) chỉ chạy sau thời điểm này.
ALTER TABLE review_jobs ADD COLUMN not_before TIMESTAMPTZ;
