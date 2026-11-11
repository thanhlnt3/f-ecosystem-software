//! Kiểm thử với sandbox thật của ZaloPay (tasks.md 13.2): tạo đơn và truy vấn chủ động, để chắc
//! chắn cách ký `key1` khớp với cổng thật. Chỉ chạy khi có khóa sandbox trong biến môi trường
//! (không đưa khóa vào mã nguồn):
//!
//! ```bash
//! FECO_ZALOPAY_SANDBOX_APP_ID=... FECO_ZALOPAY_SANDBOX_KEY1=... FECO_ZALOPAY_SANDBOX_KEY2=... \
//!   cargo test -p feco-server --test zalopay_sandbox
//! ```
//!
//! Không gửi dữ liệu cá nhân nào: người dùng là một mã cố định, số tiền 10.000 ₫.

use feco_config::{Secret, ZalopayConfig};
use feco_server::zalopay::{self, NewOrder, OrderStatus, ZaloPay, ZaloPayError};
use time::OffsetDateTime;
use url::Url;

fn sandbox(key1: Option<&str>) -> Option<ZaloPay> {
    let get = |k: &str| std::env::var(k).ok().filter(|v| !v.is_empty());
    let (app_id, k1, k2) = (
        get("FECO_ZALOPAY_SANDBOX_APP_ID")?,
        get("FECO_ZALOPAY_SANDBOX_KEY1")?,
        get("FECO_ZALOPAY_SANDBOX_KEY2")?,
    );
    Some(ZaloPay::new(&ZalopayConfig {
        app_id: app_id.parse().expect("app_id là số"),
        key1: Secret::new(key1.unwrap_or(&k1)),
        key2: Secret::new(k2),
        endpoint: Url::parse("https://sb-openapi.zalopay.vn/").unwrap(),
    }))
}

#[tokio::test]
async fn create_and_query_against_the_real_sandbox() {
    let Some(zp) = sandbox(None) else {
        eprintln!(
            "bỏ qua: đặt FECO_ZALOPAY_SANDBOX_APP_ID, _KEY1, _KEY2 để thử với sandbox ZaloPay"
        );
        return;
    };
    let now = OffsetDateTime::now_utc();
    let app_trans_id = format!("{}_feco{}", zalopay::yymmdd(now), now.unix_timestamp());
    let order = |id: &'static str| NewOrder {
        app_trans_id: id,
        app_user: "feco-sandbox-test",
        amount: 10_000,
        description: "F-Eco - kiem thu sandbox",
        method: "vietqr",
        redirect_url: "https://f-eco.invalid/payments/return",
        callback_url: "https://f-eco.invalid/v1/payments/zalopay/callback",
        expire_seconds: 900,
        now,
    };
    let id: &'static str = Box::leak(app_trans_id.clone().into_boxed_str());
    let created = zp
        .create(&order(id))
        .await
        .expect("sandbox nhận đơn ký bằng key1");
    assert!(
        created.order_url.starts_with("https://"),
        "{}",
        created.order_url
    );
    // Chưa ai trả tiền: cổng báo đang chờ (không phải thất bại hay sai chữ ký).
    assert!(matches!(
        zp.query(&app_trans_id).await.unwrap(),
        OrderStatus::Pending { .. }
    ));
    // Khóa sai: cổng từ chối.
    let wrong = sandbox(Some("khoa-sai")).unwrap();
    let other: &'static str = Box::leak(format!("{app_trans_id}x").into_boxed_str());
    assert!(matches!(
        wrong.create(&order(other)).await,
        Err(ZaloPayError::Rejected { .. })
    ));
}
