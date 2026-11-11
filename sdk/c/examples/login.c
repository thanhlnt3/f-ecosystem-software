/*
 * App mẫu C dùng F-SDK: được F-Launcher mở thì đăng nhập bằng mã khởi chạy, không thì mở trình
 * duyệt (PKCE); rồi in /v1/me.
 *
 *   cargo build -p fsdk-c
 *   cc sdk/c/examples/login.c -Isdk/c/include -Ltarget/debug -lfsdk_c -o login
 *   ./login              # hoặc: ./login --abi  (chỉ in phiên bản ABI)
 */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include "fsdk.h"

int main(int argc, char **argv) {
    if (argc > 1 && strcmp(argv[1], "--abi") == 0) {
        printf("fsdk abi %u (header %d)\n", fsdk_abi_version(), FSDK_ABI_VERSION);
        return fsdk_abi_version() == FSDK_ABI_VERSION ? 0 : 1;
    }
    fsdk_session *session = getenv("FECO_LAUNCH_CODE")
        ? fsdk_session_from_launch_code(NULL, NULL, NULL, NULL, NULL, NULL)
        : fsdk_login_browser(NULL, NULL, NULL, NULL, NULL);
    if (session == NULL) {
        fprintf(stderr, "đăng nhập lỗi: %s\n", fsdk_last_error());
        return 1;
    }
    char *me = fsdk_me_json(session);
    if (me == NULL) {
        fprintf(stderr, "/v1/me lỗi: %s\n", fsdk_last_error());
        fsdk_session_free(session);
        return 1;
    }
    printf("%s\n", me);
    fsdk_string_free(me);
    fsdk_session_free(session);
    return 0;
}
