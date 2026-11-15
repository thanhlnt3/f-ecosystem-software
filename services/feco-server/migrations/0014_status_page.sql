-- Trang trạng thái công khai (spec platform-support, "Độ sẵn sàng"; tasks.md 15.3).
-- Máy chủ kiểm tra từng thành phần mỗi phút; hai lần lỗi liên tiếp mở một sự cố, lần kiểm tra
-- tốt đầu tiên sau đó đóng sự cố. Kết quả kiểm tra giữ 90 ngày.

CREATE TABLE status_checks (
    component   TEXT        NOT NULL,
    ok          BOOLEAN     NOT NULL,
    latency_ms  INTEGER     NOT NULL,
    checked_at  TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE INDEX status_checks_component_at ON status_checks (component, checked_at DESC);

CREATE TABLE status_incidents (
    id          UUID        PRIMARY KEY,
    component   TEXT        NOT NULL,
    started_at  TIMESTAMPTZ NOT NULL,
    resolved_at TIMESTAMPTZ,
    detail      TEXT        NOT NULL DEFAULT ''
);
CREATE INDEX status_incidents_started ON status_incidents (started_at DESC);
-- Mỗi thành phần có nhiều nhất một sự cố đang mở.
CREATE UNIQUE INDEX status_incidents_open ON status_incidents (component) WHERE resolved_at IS NULL;
