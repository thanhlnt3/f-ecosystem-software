# Tích hợp ThesisAI với F-Eco

ThesisAI là ứng dụng web giúp sinh viên luyện bảo vệ đồ án tốt nghiệp với hội đồng AI. Ứng dụng gồm hai repo:

| Phần | Repo | Commit đã dùng để tạo bản vá |
|---|---|---|
| Frontend (Next.js 16, cổng 3000) | https://github.com/justAchillGuy8386/thesisai-fe | `671822188b949e2290f1c73c12bd32a47b178a0d` |
| Backend (FastAPI, cổng 8000) | https://github.com/justAchillGuy8386/thesisai-be | `db66b57ea87394da334fda0b1cb12a6be248275f` |

Trong F-Eco, ThesisAI là **app web**:
- F-Store có trang ThesisAI với nút "Mở ứng dụng web".
- F-Launcher hiện ThesisAI với nhãn "App web". Nút "Thêm" ghi app vào Thư viện; "Mở" mở ThesisAI bằng trình duyệt.
- Người dùng đăng nhập ThesisAI bằng tài khoản F-Eco (OIDC). Trình duyệt đã đăng nhập F-Eco thì không phải nhập lại mật khẩu.

Thư mục này chỉ có bản vá, không có mã của ThesisAI. Tác giả ThesisAI áp bản vá vào repo của mình.

## Bản vá

| Tệp | Nội dung |
|---|---|
| `thesisai-be.patch` | Route `/api/auth/feco/login` và `/api/auth/feco/callback` (Authlib), cột `users.feco_subject` và migration Alembic, cấu hình `FECO_*`, `SessionMiddleware`, thư viện `authlib` và `itsdangerous` (BSD-3-Clause) |
| `thesisai-fe.patch` | Trang `/auth/feco` và nút "Đăng nhập bằng F-Eco" ở trang đăng nhập; nâng Next.js 16.4.0 lên 16.4.6 (vá hai lỗ hổng CRITICAL, xem `SCAN.md`) và các thư viện phụ có lỗ hổng trong `package-lock.json` |

Áp bản vá:

```bash
cd thesisai-be
git apply ../f-ecosystem/integrations/thesisai/thesisai-be.patch
cd ../thesisai-fe
git apply ../f-ecosystem/integrations/thesisai/thesisai-fe.patch
```

## Luồng đăng nhập

1. F-Launcher (hoặc nút "Đăng nhập bằng F-Eco") mở `http://localhost:3000/auth/feco`.
2. Trang đó chuyển sang `GET http://localhost:8000/api/auth/feco/login`.
3. Backend chuyển tới F-Eco: authorization code + PKCE, `state` và `nonce` trong cookie phiên ký bằng `SECRET_KEY`.
4. F-Eco hỏi người dùng đồng ý chia sẻ tên và email (lần đầu), rồi chuyển về `GET /api/auth/feco/callback`.
5. Backend đổi mã lấy token bằng client secret và kiểm chữ ký ID token. Sau đó gọi `GET {FECO_SERVER_URL}v1/me` để lấy tên, email, `email_verified`.
6. Backend tìm người dùng theo `feco_subject` (`sub` pairwise của F-Eco). Chưa có thì:
   - email đã có tài khoản ThesisAI và F-Eco xác nhận email (`email_verified`): liên kết tài khoản đó;
   - email đã có tài khoản nhưng chưa xác minh: từ chối, người dùng xác minh email trên F-Eco hoặc đăng nhập bằng mật khẩu;
   - chưa có tài khoản: tạo tài khoản vai trò `student`, mật khẩu ngẫu nhiên (chỉ đăng nhập qua F-Eco).
7. Backend cấp JWT của ThesisAI rồi chuyển về `http://localhost:3000/auth/feco#token=...`. Token nằm sau dấu `#` nên không vào nhật ký máy chủ hay header Referer.
8. Trang `/auth/feco` xóa token khỏi thanh địa chỉ, gọi `/api/auth/me`, lưu như đăng nhập thường và vào `/dashboard`.

Đăng nhập bằng email và mật khẩu của ThesisAI vẫn giữ.

## Chạy cùng F-Eco dev

1. Chạy F-Eco dev (README của F-Eco, các bước 1–7).
2. Đưa ThesisAI vào kho TUF và danh mục dev:

   ```bash
   scripts/dev-apps.sh thesisai
   ```

   Lệnh này:
   - tạo client OAuth phía máy chủ (scope `openid profile email`, chuyển hướng `http://localhost:8000/api/auth/feco/callback`);
   - lưu client ID và secret trong `.dev/thesisai-client-id` và `.dev/thesisai-client-secret` (không commit);
   - ký gói app web với logo, ghi danh mục F-Store;
   - in các biến cần đặt cho backend ThesisAI.
3. Đặt các biến trong `.env` của backend ThesisAI:

   | Biến | Giá trị dev |
   |---|---|
   | `FECO_ISSUER_URL` | `http://127.0.0.1:4444/` |
   | `FECO_SERVER_URL` | `http://127.0.0.1:8080/` |
   | `FECO_CLIENT_ID` | nội dung `.dev/thesisai-client-id` |
   | `FECO_CLIENT_SECRET` | nội dung `.dev/thesisai-client-secret` |
   | `FECO_REDIRECT_URI` | `http://localhost:8000/api/auth/feco/callback` |
   | `FRONTEND_URL` | `http://localhost:3000` |

4. Chạy ThesisAI theo README của từng repo. ThesisAI cần:
   - PostgreSQL có phần mở rộng pgvector;
   - Ollama với mô hình `qwen2.5:7b` và `bge-m3` (khoảng 6 GB);
   - thư viện Python (`pip install -r requirements.txt`, gồm faster-whisper) và npm (`npm ci`).

   Sau khi áp bản vá, chạy `alembic upgrade head` để thêm cột `feco_subject`.
5. Trong F-Launcher, mở Cửa hàng, chọn ThesisAI, bấm "Thêm" rồi "Mở".

## Môi trường thật

- Đổi `web_url` của app sang địa chỉ HTTPS của ThesisAI (`THESISAI_URL=https://... scripts/dev-apps.sh thesisai` cho dev, hoặc qua quy trình phát hành của F-Eco). Launcher chỉ mở `https://`, hoặc `http://localhost` và `http://127.0.0.1`.
- Đăng ký địa chỉ chuyển hướng HTTPS của backend.
- Đặt `SECRET_KEY` riêng, không dùng giá trị mặc định trong `config.py`; bật `https_only` cho cookie phiên.

## Đã kiểm thử

- Phía F-Eco, E2E `web_apps_and_server_side_login`:
  - client phía máy chủ đi hết luồng authorization code + PKCE với client secret;
  - `sub` là pairwise và `/v1/me` trả email;
  - sai secret bị từ chối (`invalid_client`);
  - địa chỉ chuyển hướng chưa đăng ký không nhận được mã.
- Bản vá ThesisAI:
  - áp sạch lên đúng commit ở bảng trên;
  - các tệp Python qua `python -m py_compile`.

Chưa chạy ThesisAI thật vì máy dev chưa có thư viện, Ollama và mô hình của ThesisAI (QUESTIONS.md, mục 92).
