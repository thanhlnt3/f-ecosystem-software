-- Thay đổi app-update-notices, design U1: mỗi phiên bản phát hành cho mọi người (kênh stable,
-- 100%) chỉ được báo cập nhật một lần. Dòng có sẵn nghĩa là đã báo.

CREATE TABLE app_update_notices (
    app_id      TEXT        NOT NULL REFERENCES catalog_apps (id) ON DELETE CASCADE,
    version     TEXT        NOT NULL,
    notified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (app_id, version)
);

-- Các bản đang phát hành cho mọi người trước thay đổi này coi như đã báo, để việc định kỳ không
-- gửi lại thông báo cũ.
INSERT INTO app_update_notices (app_id, version)
SELECT DISTINCT app_id, version FROM catalog_versions
WHERE status = 'released' AND channel = 'stable' AND rollout_percent = 100
ON CONFLICT DO NOTHING;
