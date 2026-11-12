//! Điều phối các cổng thanh toán (design D9): mỗi giao dịch thuộc một cổng (`provider`), và mọi
//! thao tác (tạo, truy vấn, hoàn tiền) đi qua đây để chọn đúng adapter: ZaloPay, VNPAY, MoMo.
//!
//! Phương thức trên trang thanh toán là cặp `cổng:phương thức`, ví dụ `vnpay:vietqr`,
//! `zalopay:domestic_card`, `momo:momo_wallet`. Chỉ cổng đã cấu hình mới hiện.

use serde_json::{json, Value};
use time::OffsetDateTime;
use url::Url;
use uuid::Uuid;

pub use crate::zalopay::RefundStatus;
use crate::{payments::Transaction, vnpay, zalopay, AppState};

/// Kết quả truy vấn trạng thái thanh toán ở cổng.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PayStatus {
    Paid {
        gateway_trans_id: String,
        amount: i64,
    },
    Failed,
    Pending {
        processing: bool,
    },
}

#[derive(Debug, thiserror::Error)]
pub enum GatewayError {
    #[error("không gọi được cổng thanh toán: {0}")]
    Http(String),
    #[error("cổng thanh toán từ chối ({code}): {message}")]
    Rejected { code: String, message: String },
    #[error("chữ ký phản hồi của cổng không hợp lệ")]
    BadSignature,
    #[error("cổng {0} chưa được cấu hình")]
    NotConfigured(String),
}

impl From<zalopay::ZaloPayError> for GatewayError {
    fn from(e: zalopay::ZaloPayError) -> Self {
        match e {
            zalopay::ZaloPayError::Http(m) => GatewayError::Http(m),
            zalopay::ZaloPayError::Rejected {
                code,
                sub_code,
                message,
            } => GatewayError::Rejected {
                code: format!("{code}/{sub_code}"),
                message,
            },
        }
    }
}

/// Địa chỉ IP F-Eco khai với VNPAY khi gọi API từ máy chủ.
const SERVER_IP: &str = "127.0.0.1";

/// Các lựa chọn thanh toán của những cổng đã cấu hình, theo thứ tự hiện trên trang.
pub fn choices(state: &AppState) -> Vec<(&'static str, &'static str)> {
    let mut out = Vec::new();
    if state.vnpay.is_some() {
        out.extend(vnpay::METHODS.iter().map(|m| ("vnpay", *m)));
    }
    if state.momo.is_some() {
        out.extend(crate::momo::METHODS.iter().map(|m| ("momo", *m)));
    }
    if state.zalopay.is_some() {
        out.extend(zalopay::METHODS.iter().map(|m| ("zalopay", *m)));
    }
    out
}

/// Đọc lựa chọn `cổng:phương thức` (chỉ nhận cổng đã cấu hình).
pub fn parse_choice(state: &AppState, s: &str) -> Option<(&'static str, &'static str)> {
    let (p, m) = s.split_once(':')?;
    choices(state)
        .into_iter()
        .find(|(cp, cm)| *cp == p && *cm == m)
}

pub fn any(state: &AppState) -> bool {
    state.zalopay.is_some() || state.vnpay.is_some() || state.momo.is_some()
}

pub fn provider_label(p: &str) -> &'static str {
    match p {
        "vnpay" => "VNPAY",
        "momo" => "MoMo",
        "zalopay" => "ZaloPay",
        _ => "F-Eco",
    }
}

/// Mã giao dịch phía F-Eco theo quy định của từng cổng: ZaloPay cần tiền tố `yymmdd_`, VNPAY chỉ
/// nhận chữ và số.
pub fn new_ref(provider: &str, now: OffsetDateTime, id: Uuid) -> String {
    let short = &id.simple().to_string()[..20];
    match provider {
        "vnpay" => format!("{}{short}", zalopay::yymmdd(now)),
        _ => format!("{}_{short}", zalopay::yymmdd(now)),
    }
}

pub struct Create<'a> {
    pub provider: &'a str,
    pub method: &'a str,
    pub gateway_ref: &'a str,
    pub amount: i64,
    pub description: &'a str,
    pub app_user: &'a str,
    pub client_ip: &'a str,
    pub expire_minutes: i64,
    pub now: OffsetDateTime,
}

/// Tạo giao dịch ở cổng; trả địa chỉ chuyển khách tới và dữ liệu cần giữ cho các lần gọi sau.
pub async fn create(state: &AppState, c: &Create<'_>) -> Result<(String, Value), GatewayError> {
    let return_url = format!("{}payments/return", state.public_url);
    let base = &state.payment_callback_base;
    match c.provider {
        "zalopay" => {
            let zp = state
                .zalopay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("ZaloPay".into()))?;
            let created = zp
                .create(&zalopay::NewOrder {
                    app_trans_id: c.gateway_ref,
                    app_user: c.app_user,
                    amount: c.amount,
                    description: c.description,
                    method: c.method,
                    redirect_url: &return_url,
                    callback_url: &format!("{base}v1/payments/zalopay/callback"),
                    expire_seconds: c.expire_minutes * 60,
                    now: c.now,
                })
                .await?;
            Ok((created.order_url, json!({})))
        }
        "vnpay" => {
            let v = state
                .vnpay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("VNPAY".into()))?;
            // VNPAY lấy IPN URL từ cấu hình merchant; trang chuyển về mang mã giao dịch.
            let url = v.payment_url(
                c.gateway_ref,
                c.amount,
                c.description,
                c.method,
                &return_url,
                c.client_ip,
                c.now,
                c.expire_minutes,
            );
            Ok((url, json!({"create_date": vnpay::vn_time(c.now)})))
        }
        "momo" => {
            let m = state
                .momo
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("MoMo".into()))?;
            let url = m
                .create(
                    c.gateway_ref,
                    c.amount,
                    c.description,
                    &return_url,
                    &format!("{base}v1/payments/momo/ipn"),
                    c.expire_minutes,
                )
                .await?;
            Ok((url, json!({})))
        }
        other => Err(GatewayError::NotConfigured(other.into())),
    }
}

/// Truy vấn chủ động trạng thái một giao dịch.
pub async fn query(
    state: &AppState,
    t: &Transaction,
    now: OffsetDateTime,
) -> Result<PayStatus, GatewayError> {
    match t.provider.as_str() {
        "zalopay" => {
            let zp = state
                .zalopay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("ZaloPay".into()))?;
            Ok(match zp.query(&t.gateway_ref).await? {
                zalopay::OrderStatus::Paid {
                    zp_trans_id,
                    amount,
                } => PayStatus::Paid {
                    gateway_trans_id: zp_trans_id,
                    amount,
                },
                zalopay::OrderStatus::Failed => PayStatus::Failed,
                zalopay::OrderStatus::Pending { processing } => PayStatus::Pending { processing },
            })
        }
        "vnpay" => {
            let v = state
                .vnpay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("VNPAY".into()))?;
            let create_date = t.gateway_meta["create_date"].as_str().unwrap_or_default();
            v.query(&t.gateway_ref, create_date, now, SERVER_IP).await
        }
        "momo" => {
            let m = state
                .momo
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("MoMo".into()))?;
            m.query(&t.gateway_ref).await
        }
        other => Err(GatewayError::NotConfigured(other.into())),
    }
}

/// Gửi lệnh hoàn tới cổng của giao dịch gốc; trả mã lệnh hoàn và kết quả (có thể còn đang xử lý).
pub async fn refund(
    state: &AppState,
    t: &Transaction,
    id: Uuid,
    amount: i64,
    description: &str,
    by: &str,
    now: OffsetDateTime,
) -> Result<(String, RefundStatus), GatewayError> {
    let trans = t.gateway_trans_id.clone().unwrap_or_default();
    match t.provider.as_str() {
        "zalopay" => {
            let zp = state
                .zalopay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("ZaloPay".into()))?;
            let m_refund_id = format!(
                "{}_{}_{}",
                zalopay::yymmdd(now),
                zp.app_id(),
                &id.simple().to_string()[..16]
            );
            let status = zp
                .refund(&m_refund_id, &trans, amount, description, now)
                .await?;
            Ok((m_refund_id, status))
        }
        "vnpay" => {
            let v = state
                .vnpay
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("VNPAY".into()))?;
            let create_date = t.gateway_meta["create_date"].as_str().unwrap_or_default();
            let full = amount >= t.amount_vnd && t.refunded_vnd == 0;
            let status = v
                .refund(
                    &t.gateway_ref,
                    &trans,
                    create_date,
                    amount,
                    full,
                    by,
                    now,
                    SERVER_IP,
                )
                .await?;
            Ok((format!("vnpay_{}", id.simple()), status))
        }
        "momo" => {
            let m = state
                .momo
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("MoMo".into()))?;
            let refund_id = format!(
                "{}_rf_{}",
                zalopay::yymmdd(now),
                &id.simple().to_string()[..16]
            );
            let status = m.refund(&refund_id, &trans, amount, description).await?;
            Ok((refund_id, status))
        }
        other => Err(GatewayError::NotConfigured(other.into())),
    }
}

/// Hỏi kết quả một lệnh hoàn còn đang xử lý.
pub async fn query_refund(
    state: &AppState,
    provider: &str,
    refund_ref: &str,
    now: OffsetDateTime,
) -> Result<RefundStatus, GatewayError> {
    match provider {
        "zalopay" => Ok(state
            .zalopay
            .as_ref()
            .ok_or_else(|| GatewayError::NotConfigured("ZaloPay".into()))?
            .query_refund(refund_ref, now)
            .await?),
        "momo" => {
            state
                .momo
                .as_ref()
                .ok_or_else(|| GatewayError::NotConfigured("MoMo".into()))?
                .query_refund(refund_ref)
                .await
        }
        // VNPAY trả kết quả hoàn ngay; lệnh "đang xử lý" (mã 94) được hỏi lại bằng querydr của nó.
        _ => Ok(RefundStatus::Processing),
    }
}

/// Đường dẫn tương đối của một cổng (cho tài liệu, nhật ký).
pub fn callback_path(provider: &str) -> &'static str {
    match provider {
        "vnpay" => "v1/payments/vnpay/ipn",
        "momo" => "v1/payments/momo/ipn",
        _ => "v1/payments/zalopay/callback",
    }
}

pub fn callback_url(base: &Url, provider: &str) -> String {
    format!("{base}{}", callback_path(provider))
}
