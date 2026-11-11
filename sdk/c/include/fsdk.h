/*
 * F-SDK cho C (spec app-sdk). License MIT OR Apache-2.0.
 *
 * - Hàm trả con trỏ: NULL khi lỗi; đọc lỗi bằng fsdk_last_error() (JSON gồm
 *   "error", "message", "message_vi", "request_id", "status").
 * - Hàm trả int: 0 khi thành công.
 * - Chuỗi trả về do F-SDK cấp phát: giải phóng bằng fsdk_string_free().
 * - Mọi chuỗi là UTF-8 kết thúc bằng '\0'. Tham số chuỗi NULL thì lấy từ biến môi trường
 *   (FECO_LAUNCH_CODE, FECO_CLIENT_ID, FECO_ISSUER_URL, FECO_SERVER_URL, FSDK_REDIRECT_URI,
 *   FSDK_SCOPES).
 */
#ifndef FSDK_H
#define FSDK_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define FSDK_ABI_VERSION 1

typedef struct FsdkSession fsdk_session;

uint32_t fsdk_abi_version(void);
const char *fsdk_last_error(void);

/* Phiên từ mã khởi chạy mà F-Launcher truyền cho app. */
fsdk_session *fsdk_session_from_launch_code(const char *code, const char *client_id,
                                            const char *issuer, const char *server,
                                            const char *redirect_uri, const char *scopes);
/* Đăng nhập qua trình duyệt hệ thống bằng PKCE (khi chạy ngoài launcher). Chặn tới khi xong. */
fsdk_session *fsdk_login_browser(const char *client_id, const char *issuer, const char *server,
                                 const char *redirect_uri, const char *scopes);

char *fsdk_access_token(const fsdk_session *session);
char *fsdk_refresh_token(const fsdk_session *session);
char *fsdk_me_json(const fsdk_session *session);
char *fsdk_entitlements_json(const fsdk_session *session);
int fsdk_report_crash(const fsdk_session *session, const char *version);

void fsdk_string_free(char *s);
void fsdk_session_free(fsdk_session *session);

#ifdef __cplusplus
}
#endif

#endif /* FSDK_H */
