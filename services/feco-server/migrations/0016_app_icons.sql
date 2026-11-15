-- Logo của app (PNG base64, tối đa 64 KB): hiện trên F-Store và được ký vào metadata TUF để
-- F-Launcher hiển thị (thay đổi OpenSpec launcher-logos-and-updates).
ALTER TABLE catalog_apps ADD COLUMN icon TEXT;
