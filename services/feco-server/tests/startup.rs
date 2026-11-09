//! Chạy chính tệp `feco-server` để kiểm tra nó từ chối khởi động khi cấu hình sai
//! (spec payments, scenario "Staging dùng khóa thật").

use std::process::Command;

fn server() -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_feco-server"));
    // Không để biến môi trường của máy chạy test ảnh hưởng kết quả.
    for (k, _) in std::env::vars() {
        if k.starts_with("FECO_") {
            cmd.env_remove(k);
        }
    }
    cmd
}

#[test]
fn refuses_to_start_when_staging_points_at_live_gateway() {
    let out = server()
        .env("FECO_ENV", "staging")
        .env("FECO_HTTP_ADDR", "127.0.0.1:0")
        .env("FECO_VNPAY_TMN_CODE", "LIVE0001")
        .env("FECO_VNPAY_HASH_SECRET", "live-secret-value")
        .env("FECO_VNPAY_PAYMENT_URL", "https://pay.vnpay.vn/vpcpay.html")
        .output()
        .unwrap();
    assert_eq!(out.status.code(), Some(78));
    let stderr = String::from_utf8_lossy(&out.stderr);
    assert!(stderr.contains("chỉ được dùng sandbox"), "{stderr}");
    assert!(
        !stderr.contains("live-secret-value"),
        "bí mật bị lộ ra stderr: {stderr}"
    );
}

#[test]
fn refuses_to_start_without_environment() {
    let out = server().output().unwrap();
    assert_eq!(out.status.code(), Some(78));
    assert!(String::from_utf8_lossy(&out.stderr).contains("FECO_ENV"));
}
