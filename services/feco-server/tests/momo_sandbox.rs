//! Kiểm thử với sandbox thật của MoMo (tasks.md 13.2): tạo thanh toán ví, truy vấn chủ động, khóa
//! sai bị từ chối. Mục đích là chắc chắn cách ký HMAC-SHA256 khớp cổng thật. Chỉ chạy khi có
//! khóa trong biến môi trường (không đưa khóa vào mã nguồn). Có thể dùng khóa thử công khai trong
//! tài liệu cho nhà phát triển của MoMo, hoặc khóa sandbox của tài khoản M4B:
//!
//! ```bash
//! FECO_MOMO_SANDBOX_PARTNER_CODE=... FECO_MOMO_SANDBOX_ACCESS_KEY=... \
//!   FECO_MOMO_SANDBOX_SECRET_KEY=... cargo test -p feco-server --test momo_sandbox
//! ```
//!
//! Không gửi dữ liệu cá nhân nào: chỉ một mã đơn ngẫu nhiên, số tiền 10.000 ₫.

use feco_config::{MomoConfig, Secret};
use feco_server::gateways::{GatewayError, PayStatus};
use feco_server::momo::Momo;
use url::Url;

fn sandbox(secret: Option<&str>) -> Option<Momo> {
    let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (partner, access, secret_key) = (
        get("FECO_MOMO_SANDBOX_PARTNER_CODE")?,
        get("FECO_MOMO_SANDBOX_ACCESS_KEY")?,
        get("FECO_MOMO_SANDBOX_SECRET_KEY")?,
    );
    Some(Momo::new(&MomoConfig {
        partner_code: partner,
        access_key: Secret::new(access),
        secret_key: Secret::new(secret.unwrap_or(&secret_key)),
        endpoint: Url::parse("https://test-payment.momo.vn/").unwrap(),
    }))
}

#[tokio::test]
async fn create_and_query_against_the_real_sandbox() {
    let Some(momo) = sandbox(None) else {
        eprintln!(
            "bỏ qua: đặt FECO_MOMO_SANDBOX_PARTNER_CODE, _ACCESS_KEY, _SECRET_KEY để thử với sandbox MoMo"
        );
        return;
    };
    let order_id = format!("feco{}", uuid::Uuid::new_v4().simple());
    let pay_url = momo
        .create(
            &order_id,
            10_000,
            "F-Eco - kiem thu sandbox",
            "https://f-eco.invalid/payments/return",
            "https://f-eco.invalid/v1/payments/momo/ipn",
            15,
        )
        .await
        .expect("sandbox nhận đơn ký bằng secretKey");
    assert!(pay_url.starts_with("https://"), "{pay_url}");

    // Chưa ai trả tiền: cổng báo đang chờ, không phải thất bại hay sai chữ ký.
    let status = momo.query(&order_id).await.expect("truy vấn được");
    assert!(matches!(status, PayStatus::Pending { .. }), "{status:?}");

    // Khóa sai: cổng từ chối, không tạo đơn.
    let wrong = sandbox(Some("khoa-sai")).unwrap();
    let other = format!("{order_id}x");
    let err = wrong
        .create(
            &other,
            10_000,
            "F-Eco",
            "https://f-eco.invalid/r",
            "https://f-eco.invalid/i",
            15,
        )
        .await
        .unwrap_err();
    assert!(matches!(err, GatewayError::Rejected { .. }), "{err:?}");
}
