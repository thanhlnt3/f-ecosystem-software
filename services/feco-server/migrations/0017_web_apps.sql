-- Thay đổi launcher-sidebar-and-web-apps: app web trong F-Store.
-- Địa chỉ mở app web (https://, hoặc http://localhost, http://127.0.0.1 cho dev). Máy chủ kiểm bằng
-- feco_tuf::repo::valid_web_url trước khi ghi; NULL với app desktop.
ALTER TABLE catalog_apps ADD COLUMN web_url TEXT
    CHECK (web_url IS NULL OR web_url ~ '^(https://|http://(localhost|127\.0\.0\.1)[:/])');
