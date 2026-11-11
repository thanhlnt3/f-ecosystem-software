//! F-SDK cho C: giao diện C ổn định trên lõi Rust `fsdk` (spec app-sdk; design D7).
//! Khai báo cho C nằm ở `include/fsdk.h`; ABI có phiên bản ([`fsdk_abi_version`]).
//!
//! Quy ước:
//! - Hàm trả con trỏ: `NULL` khi lỗi, đọc lỗi bằng [`fsdk_last_error`] (JSON
//!   `{"error", "message", "message_vi", "request_id", "status"}`).
//! - Hàm trả `int`: `0` khi thành công.
//! - Chuỗi trả về do F-SDK cấp phát; giải phóng bằng [`fsdk_string_free`].
//! - Mọi chuỗi vào/ra là UTF-8 kết thúc bằng `\0`.

use std::{
    cell::RefCell,
    ffi::{c_char, c_int, CStr, CString},
    sync::OnceLock,
};

use fsdk::{Config, Error, Launch, Session};

/// Phiên đăng nhập (kiểu mờ đối với C).
pub struct FsdkSession {
    inner: Session,
}

fn runtime() -> &'static tokio::runtime::Runtime {
    static RT: OnceLock<tokio::runtime::Runtime> = OnceLock::new();
    RT.get_or_init(|| {
        tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .expect("tạo runtime của F-SDK")
    })
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::default());
}

fn set_error(e: &Error) {
    let value = match e {
        Error::Api(api) => serde_json::json!({
            "status": api.status, "error": api.error, "message": api.message,
            "message_vi": api.message_vi, "request_id": api.request_id,
        }),
        Error::ConsentRequired => serde_json::json!({
            "error": "consent_required", "message": e.to_string(), "message_vi": e.to_string(),
        }),
        Error::NotLaunched => serde_json::json!({
            "error": "not_launched", "message": e.to_string(), "message_vi": e.to_string(),
        }),
        other => serde_json::json!({
            "error": "sdk_error", "message": other.to_string(), "message_vi": other.to_string(),
        }),
    };
    let text = CString::new(value.to_string()).unwrap_or_default();
    LAST_ERROR.with(|cell| *cell.borrow_mut() = text);
}

fn clear_error() {
    LAST_ERROR.with(|cell| *cell.borrow_mut() = CString::default());
}

/// Đọc chuỗi C; `NULL` hoặc rỗng thành `None`.
///
/// # Safety
/// `ptr` là `NULL` hoặc trỏ tới chuỗi C hợp lệ, kết thúc bằng `\0`, còn sống trong lúc gọi.
unsafe fn opt_str(ptr: *const c_char) -> Option<String> {
    if ptr.is_null() {
        return None;
    }
    // SAFETY: người gọi bảo đảm `ptr` trỏ tới chuỗi C hợp lệ (xem hợp đồng của hàm).
    let s = unsafe { CStr::from_ptr(ptr) }
        .to_string_lossy()
        .into_owned();
    Some(s).filter(|s| !s.trim().is_empty())
}

fn out_string(s: String) -> *mut c_char {
    CString::new(s)
        .map(CString::into_raw)
        .unwrap_or(std::ptr::null_mut())
}

fn into_session(r: fsdk::Result<Session>) -> *mut FsdkSession {
    match r {
        Ok(inner) => {
            clear_error();
            Box::into_raw(Box::new(FsdkSession { inner }))
        }
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

fn config_or_error(
    client_id: Option<String>,
    issuer: Option<String>,
    server: Option<String>,
    redirect_uri: Option<String>,
    scopes: Option<String>,
) -> Result<Config, Error> {
    let env = |k: &str| std::env::var(k).ok().filter(|v| !v.trim().is_empty());
    let need = |v: Option<String>, k: &str| {
        v.or_else(|| env(k))
            .ok_or_else(|| Error::Config(format!("thiếu {k}")))
    };
    let mut config = Config::new(
        &need(client_id, "FECO_CLIENT_ID")?,
        &need(issuer, "FECO_ISSUER_URL")?,
        &need(server, "FECO_SERVER_URL")?,
    )?;
    if let Some(uri) = redirect_uri.or_else(|| env("FSDK_REDIRECT_URI")) {
        config = config.redirect_uri(&uri);
    }
    if let Some(s) = scopes.or_else(|| env("FSDK_SCOPES")) {
        config = config.scopes(&s);
    }
    Ok(config)
}

/// Phiên bản ABI (tăng khi đổi giao diện không tương thích ngược).
#[no_mangle]
pub extern "C" fn fsdk_abi_version() -> u32 {
    fsdk::ABI_VERSION
}

/// Lỗi gần nhất của luồng hiện tại (JSON), hoặc chuỗi rỗng. Con trỏ hợp lệ tới lần gọi F-SDK kế
/// tiếp trên cùng luồng; không giải phóng.
#[no_mangle]
pub extern "C" fn fsdk_last_error() -> *const c_char {
    LAST_ERROR.with(|cell| cell.borrow().as_ptr())
}

/// Phiên từ mã khởi chạy. Tham số `NULL` thì lấy từ biến môi trường (`FECO_LAUNCH_CODE`,
/// `FECO_CLIENT_ID`, `FECO_ISSUER_URL`, `FECO_SERVER_URL`, `FSDK_REDIRECT_URI`, `FSDK_SCOPES`).
///
/// # Safety
/// Mỗi tham số là `NULL` hoặc chuỗi C hợp lệ kết thúc bằng `\0`.
#[no_mangle]
pub unsafe extern "C" fn fsdk_session_from_launch_code(
    code: *const c_char,
    client_id: *const c_char,
    issuer: *const c_char,
    server: *const c_char,
    redirect_uri: *const c_char,
    scopes: *const c_char,
) -> *mut FsdkSession {
    // SAFETY: hợp đồng của hàm bảo đảm mọi con trỏ là NULL hoặc chuỗi C hợp lệ.
    let (code, client_id, issuer, server, redirect_uri, scopes) = unsafe {
        (
            opt_str(code),
            opt_str(client_id),
            opt_str(issuer),
            opt_str(server),
            opt_str(redirect_uri),
            opt_str(scopes),
        )
    };
    let launch = match code.map(|code| Launch { code }).or_else(Launch::from_env) {
        Some(l) => l,
        None => return into_session(Err(Error::NotLaunched)),
    };
    let config = match config_or_error(client_id, issuer, server, redirect_uri, scopes) {
        Ok(c) => c,
        Err(e) => return into_session(Err(e)),
    };
    into_session(runtime().block_on(fsdk::session_from_launch(&config, &launch)))
}

/// Đăng nhập qua trình duyệt hệ thống bằng PKCE (chạy ngoài launcher). Tham số `NULL` thì lấy từ
/// biến môi trường như [`fsdk_session_from_launch_code`]. Hàm chặn tới khi đăng nhập xong.
///
/// # Safety
/// Mỗi tham số là `NULL` hoặc chuỗi C hợp lệ kết thúc bằng `\0`.
#[no_mangle]
pub unsafe extern "C" fn fsdk_login_browser(
    client_id: *const c_char,
    issuer: *const c_char,
    server: *const c_char,
    redirect_uri: *const c_char,
    scopes: *const c_char,
) -> *mut FsdkSession {
    // SAFETY: hợp đồng của hàm bảo đảm mọi con trỏ là NULL hoặc chuỗi C hợp lệ.
    let args = unsafe {
        (
            opt_str(client_id),
            opt_str(issuer),
            opt_str(server),
            opt_str(redirect_uri),
            opt_str(scopes),
        )
    };
    let config = match config_or_error(args.0, args.1, args.2, args.3, args.4) {
        Ok(c) => c,
        Err(e) => return into_session(Err(e)),
    };
    into_session(runtime().block_on(fsdk::login_with_browser(&config)))
}

/// Mượn phiên từ con trỏ.
///
/// # Safety
/// `session` là `NULL` hoặc con trỏ do F-SDK trả về và chưa được giải phóng.
unsafe fn borrow<'a>(session: *const FsdkSession) -> Option<&'a FsdkSession> {
    // SAFETY: người gọi bảo đảm con trỏ là NULL hoặc trỏ tới FsdkSession còn sống.
    unsafe { session.as_ref() }
}

fn string_result(r: fsdk::Result<String>) -> *mut c_char {
    match r {
        Ok(s) => {
            clear_error();
            out_string(s)
        }
        Err(e) => {
            set_error(&e);
            std::ptr::null_mut()
        }
    }
}

fn null_session() -> Error {
    Error::Config("session là NULL".into())
}

/// Access token còn hạn (tự làm mới khi cần). Giải phóng bằng [`fsdk_string_free`].
///
/// # Safety
/// `session` là con trỏ do F-SDK trả về và chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_access_token(session: *const FsdkSession) -> *mut c_char {
    // SAFETY: hợp đồng của hàm.
    let Some(s) = (unsafe { borrow(session) }) else {
        return string_result(Err(null_session()));
    };
    string_result(runtime().block_on(s.inner.access_token()))
}

/// Refresh token hiện tại (để app lưu vào kho bảo mật), hoặc `NULL` nếu không có.
///
/// # Safety
/// `session` là con trỏ do F-SDK trả về và chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_refresh_token(session: *const FsdkSession) -> *mut c_char {
    // SAFETY: hợp đồng của hàm.
    let Some(s) = (unsafe { borrow(session) }) else {
        return string_result(Err(null_session()));
    };
    match runtime().block_on(s.inner.refresh_token()) {
        Some(t) => string_result(Ok(t)),
        None => std::ptr::null_mut(),
    }
}

/// `GET /v1/me` dạng JSON.
///
/// # Safety
/// `session` là con trỏ do F-SDK trả về và chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_me_json(session: *const FsdkSession) -> *mut c_char {
    // SAFETY: hợp đồng của hàm.
    let Some(s) = (unsafe { borrow(session) }) else {
        return string_result(Err(null_session()));
    };
    string_result(runtime().block_on(async { s.inner.me().await.map(|m| m.raw.to_string()) }))
}

/// `GET /v1/entitlements` dạng JSON (mảng).
///
/// # Safety
/// `session` là con trỏ do F-SDK trả về và chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_entitlements_json(session: *const FsdkSession) -> *mut c_char {
    // SAFETY: hợp đồng của hàm.
    let Some(s) = (unsafe { borrow(session) }) else {
        return string_result(Err(null_session()));
    };
    string_result(runtime().block_on(async {
        s.inner.entitlements().await.map(|list| {
            serde_json::Value::Array(
                list.into_iter()
                    .map(|e| {
                        serde_json::json!({
                            "kind": e.kind, "source": e.source,
                            "granted_at": e.granted_at, "expires_at": e.expires_at,
                        })
                    })
                    .collect(),
            )
            .to_string()
        })
    }))
}

/// Báo một lần app gặp sự cố. Trả 0 khi thành công.
///
/// # Safety
/// `session` là con trỏ do F-SDK trả về; `version` là chuỗi C hợp lệ.
#[no_mangle]
pub unsafe extern "C" fn fsdk_report_crash(
    session: *const FsdkSession,
    version: *const c_char,
) -> c_int {
    // SAFETY: hợp đồng của hàm.
    let (Some(s), Some(version)) = (unsafe { borrow(session) }, unsafe { opt_str(version) }) else {
        set_error(&Error::Config("session hoặc version là NULL".into()));
        return -1;
    };
    match runtime().block_on(s.inner.report_crash(&version)) {
        Ok(()) => {
            clear_error();
            0
        }
        Err(e) => {
            set_error(&e);
            -1
        }
    }
}

/// Giải phóng chuỗi do F-SDK trả về. `NULL` thì bỏ qua.
///
/// # Safety
/// `s` là `NULL` hoặc chuỗi do F-SDK trả về, chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_string_free(s: *mut c_char) {
    if !s.is_null() {
        // SAFETY: chuỗi được tạo bằng CString::into_raw trong F-SDK và chưa giải phóng.
        drop(unsafe { CString::from_raw(s) });
    }
}

/// Giải phóng phiên. `NULL` thì bỏ qua.
///
/// # Safety
/// `session` là `NULL` hoặc con trỏ do F-SDK trả về, chưa được giải phóng.
#[no_mangle]
pub unsafe extern "C" fn fsdk_session_free(session: *mut FsdkSession) {
    if !session.is_null() {
        // SAFETY: con trỏ được tạo bằng Box::into_raw trong F-SDK và chưa giải phóng.
        drop(unsafe { Box::from_raw(session) });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn last_error() -> serde_json::Value {
        // SAFETY: fsdk_last_error luôn trả chuỗi C hợp lệ do F-SDK giữ.
        let text = unsafe { CStr::from_ptr(fsdk_last_error()) }
            .to_string_lossy()
            .into_owned();
        serde_json::from_str(&text).unwrap()
    }

    #[test]
    fn abi_and_errors() {
        assert_eq!(fsdk_abi_version(), 1);
        // SAFETY: các con trỏ NULL được hàm chấp nhận.
        let s = unsafe {
            fsdk_session_from_launch_code(
                c"ma".as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        if std::env::var("FECO_CLIENT_ID").is_err() {
            assert!(s.is_null());
            assert_eq!(last_error()["error"], "sdk_error");
            assert!(last_error()["message"]
                .as_str()
                .unwrap()
                .contains("FECO_CLIENT_ID"));
        }
        // SAFETY: NULL được chấp nhận.
        unsafe {
            assert!(fsdk_me_json(std::ptr::null()).is_null());
            assert_eq!(fsdk_report_crash(std::ptr::null(), c"1.0.0".as_ptr()), -1);
            fsdk_session_free(std::ptr::null_mut());
            fsdk_string_free(std::ptr::null_mut());
        }
    }

    #[test]
    fn unreachable_server_reports_network_error() {
        // SAFETY: mọi chuỗi là C literal hợp lệ.
        let s = unsafe {
            fsdk_session_from_launch_code(
                c"ma-khoi-chay".as_ptr(),
                c"app".as_ptr(),
                c"http://127.0.0.1:9/".as_ptr(),
                c"http://127.0.0.1:9/".as_ptr(),
                std::ptr::null(),
                std::ptr::null(),
            )
        };
        assert!(s.is_null());
        assert_eq!(last_error()["error"], "sdk_error");
    }
}
