//! Adapter ZaloPay API v2 (https://docs.zalopay.vn): tạo đơn, kiểm chữ ký callback, truy vấn trạng
//! thái, hoàn tiền và truy vấn hoàn tiền.
//!
//! - `key1` ký yêu cầu gửi đi: tạo đơn `app_id|app_trans_id|app_user|amount|app_time|embed_data|item`,
//!   truy vấn `app_id|app_trans_id|key1`, hoàn tiền `app_id|zp_trans_id|amount|description|timestamp`,
//!   truy vấn hoàn tiền `app_id|m_refund_id|timestamp`.
//! - `key2` kiểm chữ ký callback: HMAC-SHA256 của chuỗi `data`.
//!
//! Khách nhập thẻ trên trang của ZaloPay (`order_url`): F-Eco không bao giờ nhận số thẻ hay CVV
//! (spec payments, "Không lưu dữ liệu thẻ").

use feco_config::{Secret, ZalopayConfig};
use hmac::{Hmac, KeyInit, Mac};
use serde::Deserialize;
use serde_json::{json, Value};
use sha2::Sha256;
use time::{OffsetDateTime, UtcOffset};
use url::Url;

/// Phương thức thanh toán trên trang ZaloPay (`preferred_payment_method` trong `embed_data`).
pub const METHODS: [&str; 4] = [
    "vietqr",
    "domestic_card",
    "zalopay_wallet",
    "international_card",
];

pub fn hmac_hex(key: &str, input: &str) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(key.as_bytes()).expect("HMAC nhận khóa mọi độ dài");
    mac.update(input.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// So sánh hai chuỗi hex trong thời gian không phụ thuộc nội dung.
fn same(a: &str, b: &str) -> bool {
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// `yymmdd` theo giờ Việt Nam (GMT+7), tiền tố bắt buộc của `app_trans_id` và `m_refund_id`.
pub fn yymmdd(now: OffsetDateTime) -> String {
    let vn = now.to_offset(UtcOffset::from_hms(7, 0, 0).expect("GMT+7"));
    format!(
        "{:02}{:02}{:02}",
        vn.year() % 100,
        u8::from(vn.month()),
        vn.day()
    )
}

fn millis(t: OffsetDateTime) -> i64 {
    (t.unix_timestamp_nanos() / 1_000_000) as i64
}

#[derive(Debug, thiserror::Error)]
pub enum ZaloPayError {
    #[error("không gọi được ZaloPay: {0}")]
    Http(String),
    #[error("ZaloPay trả lỗi {code} ({sub_code}): {message}")]
    Rejected {
        code: i64,
        sub_code: i64,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct NewOrder<'a> {
    pub app_trans_id: &'a str,
    pub app_user: &'a str,
    pub amount: i64,
    pub description: &'a str,
    pub method: &'a str,
    pub redirect_url: &'a str,
    pub callback_url: &'a str,
    pub expire_seconds: i64,
    pub now: OffsetDateTime,
}

#[derive(Debug, Clone)]
pub struct Created {
    pub order_url: String,
    pub zp_trans_token: String,
}

/// Kết quả truy vấn đơn: 1 thành công, 2 thất bại, 3 đang xử lý hoặc chưa thanh toán.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OrderStatus {
    Paid { zp_trans_id: String, amount: i64 },
    Failed,
    Pending { processing: bool },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RefundStatus {
    Succeeded,
    Failed(String),
    Processing,
}

/// Dữ liệu callback đã kiểm chữ ký.
#[derive(Debug, Clone, Deserialize)]
pub struct CallbackData {
    pub app_id: i64,
    pub app_trans_id: String,
    pub amount: i64,
    pub zp_trans_id: i64,
    #[serde(default)]
    pub channel: i64,
    #[serde(default)]
    pub server_time: i64,
}

#[derive(Clone)]
pub struct ZaloPay {
    app_id: i64,
    key1: Secret,
    key2: Secret,
    endpoint: Url,
    http: reqwest::Client,
}

impl ZaloPay {
    pub fn new(c: &ZalopayConfig) -> Self {
        ZaloPay {
            app_id: c.app_id,
            key1: c.key1.clone(),
            key2: c.key2.clone(),
            endpoint: c.endpoint.clone(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(20))
                .build()
                .expect("tạo HTTP client"),
        }
    }

    pub fn app_id(&self) -> i64 {
        self.app_id
    }

    async fn post(&self, path: &str, form: &[(&str, String)]) -> Result<Value, ZaloPayError> {
        let url = self
            .endpoint
            .join(path)
            .map_err(|e| ZaloPayError::Http(e.to_string()))?;
        let body = form_urlencoded::Serializer::new(String::new())
            .extend_pairs(form.iter().map(|(k, v)| (*k, v.as_str())))
            .finish();
        let res = self
            .http
            .post(url)
            .header("content-type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| ZaloPayError::Http(e.without_url().to_string()))?;
        if !res.status().is_success() {
            return Err(ZaloPayError::Http(format!("HTTP {}", res.status())));
        }
        res.json()
            .await
            .map_err(|e| ZaloPayError::Http(e.to_string()))
    }

    fn rejected(v: &Value) -> ZaloPayError {
        ZaloPayError::Rejected {
            code: v["return_code"].as_i64().unwrap_or(0),
            sub_code: v["sub_return_code"].as_i64().unwrap_or(0),
            message: v["sub_return_message"]
                .as_str()
                .or(v["return_message"].as_str())
                .unwrap_or_default()
                .to_owned(),
        }
    }

    /// Tạo đơn trên ZaloPay; khách được chuyển tới `order_url` để trả tiền.
    pub async fn create(&self, o: &NewOrder<'_>) -> Result<Created, ZaloPayError> {
        let app_time = millis(o.now).to_string();
        let embed_data =
            json!({ "redirecturl": o.redirect_url, "preferred_payment_method": [o.method] })
                .to_string();
        let item = "[]".to_owned();
        let amount = o.amount.to_string();
        let app_id = self.app_id.to_string();
        let mac = hmac_hex(
            self.key1.expose(),
            &[
                app_id.as_str(),
                o.app_trans_id,
                o.app_user,
                &amount,
                &app_time,
                &embed_data,
                &item,
            ]
            .join("|"),
        );
        let v = self
            .post(
                "v2/create",
                &[
                    ("app_id", app_id),
                    ("app_user", o.app_user.to_owned()),
                    ("app_trans_id", o.app_trans_id.to_owned()),
                    ("app_time", app_time),
                    ("expire_duration_seconds", o.expire_seconds.to_string()),
                    ("amount", amount),
                    ("description", o.description.to_owned()),
                    ("callback_url", o.callback_url.to_owned()),
                    ("item", item),
                    ("embed_data", embed_data),
                    ("bank_code", String::new()),
                    ("mac", mac),
                ],
            )
            .await?;
        if v["return_code"].as_i64() != Some(1) {
            return Err(Self::rejected(&v));
        }
        Ok(Created {
            order_url: v["order_url"].as_str().unwrap_or_default().to_owned(),
            zp_trans_token: v["zp_trans_token"].as_str().unwrap_or_default().to_owned(),
        })
    }

    /// Truy vấn chủ động trạng thái một đơn.
    pub async fn query(&self, app_trans_id: &str) -> Result<OrderStatus, ZaloPayError> {
        let app_id = self.app_id.to_string();
        let mac = hmac_hex(
            self.key1.expose(),
            &format!("{app_id}|{app_trans_id}|{}", self.key1.expose()),
        );
        let v = self
            .post(
                "v2/query",
                &[
                    ("app_id", app_id),
                    ("app_trans_id", app_trans_id.to_owned()),
                    ("mac", mac),
                ],
            )
            .await?;
        Ok(match v["return_code"].as_i64() {
            Some(1) => OrderStatus::Paid {
                zp_trans_id: match &v["zp_trans_id"] {
                    Value::String(s) => s.clone(),
                    other => other.to_string(),
                },
                amount: v["amount"].as_i64().unwrap_or(0),
            },
            Some(2) => OrderStatus::Failed,
            _ => OrderStatus::Pending {
                processing: v["is_processing"].as_bool().unwrap_or(false),
            },
        })
    }

    /// Kiểm chữ ký callback (`mac` = HMAC-SHA256(key2, data)) rồi đọc dữ liệu.
    pub fn verify_callback(&self, data: &str, mac: &str) -> Option<CallbackData> {
        if !same(
            &hmac_hex(self.key2.expose(), data),
            &mac.to_ascii_lowercase(),
        ) {
            return None;
        }
        let d: CallbackData = serde_json::from_str(data).ok()?;
        (d.app_id == self.app_id).then_some(d)
    }

    /// Lệnh hoàn tiền. ZaloPay chỉ xác nhận đã nhận lệnh; kết quả cuối lấy bằng [`Self::query_refund`].
    pub async fn refund(
        &self,
        m_refund_id: &str,
        zp_trans_id: &str,
        amount: i64,
        description: &str,
        now: OffsetDateTime,
    ) -> Result<RefundStatus, ZaloPayError> {
        let app_id = self.app_id.to_string();
        let timestamp = millis(now).to_string();
        let mac = hmac_hex(
            self.key1.expose(),
            &format!("{app_id}|{zp_trans_id}|{amount}|{description}|{timestamp}"),
        );
        let v = self
            .post(
                "v2/refund",
                &[
                    ("app_id", app_id),
                    ("m_refund_id", m_refund_id.to_owned()),
                    ("zp_trans_id", zp_trans_id.to_owned()),
                    ("amount", amount.to_string()),
                    ("timestamp", timestamp),
                    ("description", description.to_owned()),
                    ("mac", mac),
                ],
            )
            .await?;
        Ok(refund_status(&v))
    }

    pub async fn query_refund(
        &self,
        m_refund_id: &str,
        now: OffsetDateTime,
    ) -> Result<RefundStatus, ZaloPayError> {
        let app_id = self.app_id.to_string();
        let timestamp = millis(now).to_string();
        let mac = hmac_hex(
            self.key1.expose(),
            &format!("{app_id}|{m_refund_id}|{timestamp}"),
        );
        let v = self
            .post(
                "v2/query_refund",
                &[
                    ("app_id", app_id),
                    ("m_refund_id", m_refund_id.to_owned()),
                    ("timestamp", timestamp),
                    ("mac", mac),
                ],
            )
            .await?;
        Ok(refund_status(&v))
    }
}

fn refund_status(v: &Value) -> RefundStatus {
    match v["return_code"].as_i64() {
        Some(1) => RefundStatus::Succeeded,
        Some(2) => RefundStatus::Failed(
            v["sub_return_message"]
                .as_str()
                .or(v["return_message"].as_str())
                .unwrap_or_default()
                .to_owned(),
        ),
        _ => RefundStatus::Processing,
    }
}

/// Cổng ZaloPay giả trên loopback cho kiểm thử: cùng giao thức và cùng cách ký với API v2.
pub mod testing {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use axum::{extract::State, routing::post, Form, Json, Router};
    use serde_json::{json, Value};

    use super::hmac_hex;

    #[derive(Debug, Clone, Default)]
    pub struct FakeOrder {
        pub app_trans_id: String,
        pub amount: i64,
        pub description: String,
        pub embed_data: String,
        pub callback_url: String,
        pub paid: Option<i64>,
        pub failed: bool,
    }

    #[derive(Default)]
    pub struct Inner {
        pub orders: HashMap<String, FakeOrder>,
        pub refunds: HashMap<String, (String, i64)>,
        /// Số thẻ khách nhập trên trang của cổng (chỉ cổng giữ, không bao giờ gửi cho F-Eco).
        pub cards: Vec<String>,
        next_zp: i64,
    }

    #[derive(Clone)]
    pub struct FakeZaloPay {
        pub app_id: i64,
        pub key1: String,
        pub key2: String,
        pub addr: std::net::SocketAddr,
        pub inner: Arc<Mutex<Inner>>,
    }

    fn ok() -> Json<Value> {
        Json(
            json!({"return_code": 1, "return_message": "Giao dịch thành công", "sub_return_code": 1}),
        )
    }

    fn bad_mac() -> Json<Value> {
        Json(
            json!({"return_code": 2, "return_message": "Giao dịch thất bại", "sub_return_code": -402, "sub_return_message": "Mac không hợp lệ"}),
        )
    }

    async fn create(
        State(f): State<FakeZaloPay>,
        Form(p): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        let g = |k: &str| p.get(k).cloned().unwrap_or_default();
        let input = [
            g("app_id"),
            g("app_trans_id"),
            g("app_user"),
            g("amount"),
            g("app_time"),
            g("embed_data"),
            g("item"),
        ]
        .join("|");
        if hmac_hex(&f.key1, &input) != g("mac") || g("app_id") != f.app_id.to_string() {
            return bad_mac();
        }
        let mut inner = f.inner.lock().unwrap();
        if inner.orders.contains_key(&g("app_trans_id")) {
            return Json(
                json!({"return_code": 2, "sub_return_code": -68, "sub_return_message": "Trùng mã giao dịch"}),
            );
        }
        inner.orders.insert(
            g("app_trans_id"),
            FakeOrder {
                app_trans_id: g("app_trans_id"),
                amount: g("amount").parse().unwrap_or(0),
                description: g("description"),
                embed_data: g("embed_data"),
                callback_url: g("callback_url"),
                ..Default::default()
            },
        );
        let mut res = ok().0;
        res["order_url"] = json!(format!("http://{}/pay/{}", f.addr, g("app_trans_id")));
        res["zp_trans_token"] = json!(format!("token-{}", g("app_trans_id")));
        Json(res)
    }

    async fn query(
        State(f): State<FakeZaloPay>,
        Form(p): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        let g = |k: &str| p.get(k).cloned().unwrap_or_default();
        if hmac_hex(
            &f.key1,
            &format!("{}|{}|{}", g("app_id"), g("app_trans_id"), f.key1),
        ) != g("mac")
        {
            return bad_mac();
        }
        let inner = f.inner.lock().unwrap();
        match inner.orders.get(&g("app_trans_id")) {
            Some(FakeOrder {
                paid: Some(zp),
                amount,
                ..
            }) => Json(
                json!({"return_code": 1, "return_message": "Giao dịch thành công", "is_processing": false, "amount": amount, "zp_trans_id": zp}),
            ),
            Some(o) if o.failed => Json(
                json!({"return_code": 2, "return_message": "Giao dịch thất bại", "is_processing": false}),
            ),
            Some(_) => Json(
                json!({"return_code": 3, "return_message": "Giao dịch chưa thanh toán", "is_processing": false}),
            ),
            None => Json(
                json!({"return_code": 2, "return_message": "Giao dịch thất bại", "sub_return_code": -92, "sub_return_message": "Không tìm thấy giao dịch"}),
            ),
        }
    }

    async fn refund(
        State(f): State<FakeZaloPay>,
        Form(p): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        let g = |k: &str| p.get(k).cloned().unwrap_or_default();
        let input = format!(
            "{}|{}|{}|{}|{}",
            g("app_id"),
            g("zp_trans_id"),
            g("amount"),
            g("description"),
            g("timestamp")
        );
        if hmac_hex(&f.key1, &input) != g("mac") {
            return bad_mac();
        }
        let mut inner = f.inner.lock().unwrap();
        let paid: i64 = inner
            .orders
            .values()
            .filter(|o| o.paid.map(|z| z.to_string()) == Some(g("zp_trans_id")))
            .map(|o| o.amount)
            .sum();
        let already: i64 = inner
            .refunds
            .values()
            .filter(|(z, _)| *z == g("zp_trans_id"))
            .map(|(_, a)| a)
            .sum();
        let amount: i64 = g("amount").parse().unwrap_or(0);
        if paid == 0 || already + amount > paid {
            return Json(
                json!({"return_code": 2, "sub_return_code": -13, "sub_return_message": "Số tiền hoàn không hợp lệ"}),
            );
        }
        inner
            .refunds
            .insert(g("m_refund_id"), (g("zp_trans_id"), amount));
        let mut res = ok().0;
        res["return_code"] = json!(3);
        res["refund_id"] = json!(9_000_000 + inner.refunds.len() as i64);
        Json(res)
    }

    async fn query_refund(
        State(f): State<FakeZaloPay>,
        Form(p): Form<HashMap<String, String>>,
    ) -> Json<Value> {
        let g = |k: &str| p.get(k).cloned().unwrap_or_default();
        if hmac_hex(
            &f.key1,
            &format!("{}|{}|{}", g("app_id"), g("m_refund_id"), g("timestamp")),
        ) != g("mac")
        {
            return bad_mac();
        }
        if f.inner
            .lock()
            .unwrap()
            .refunds
            .contains_key(&g("m_refund_id"))
        {
            ok()
        } else {
            Json(json!({"return_code": 2, "sub_return_message": "Không tìm thấy lệnh hoàn"}))
        }
    }

    impl FakeZaloPay {
        pub async fn serve(app_id: i64, key1: &str, key2: &str) -> std::io::Result<FakeZaloPay> {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let fake = FakeZaloPay {
                app_id,
                key1: key1.to_owned(),
                key2: key2.to_owned(),
                addr: listener.local_addr()?,
                inner: Arc::default(),
            };
            let app = Router::new()
                .route("/v2/create", post(create))
                .route("/v2/query", post(query))
                .route("/v2/refund", post(refund))
                .route("/v2/query_refund", post(query_refund))
                .with_state(fake.clone());
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Ok(fake)
        }

        pub fn endpoint(&self) -> String {
            format!("http://{}/", self.addr)
        }

        pub fn order(&self, app_trans_id: &str) -> Option<FakeOrder> {
            self.inner.lock().unwrap().orders.get(app_trans_id).cloned()
        }

        /// Khách trả tiền trên trang của cổng. `card` là số thẻ khách nhập (chỉ cổng giữ). Trả về
        /// thân callback đã ký (`data`, `mac`, `type`) để test gửi tới F-Eco, hoặc không gửi để mô
        /// phỏng mất webhook.
        pub fn pay(&self, app_trans_id: &str, channel: i64, card: Option<&str>) -> Value {
            let mut inner = self.inner.lock().unwrap();
            inner.next_zp += 1;
            let zp = 250_000_000_000 + inner.next_zp;
            if let Some(c) = card {
                inner.cards.push(c.to_owned());
            }
            let o = inner
                .orders
                .get_mut(app_trans_id)
                .expect("có đơn trên cổng giả");
            o.paid = Some(zp);
            let data = json!({
                "app_id": self.app_id,
                "app_trans_id": app_trans_id,
                "app_time": 0,
                "app_user": "feco",
                "amount": o.amount,
                "embed_data": o.embed_data,
                "item": "[]",
                "zp_trans_id": zp,
                "server_time": 0,
                "channel": channel,
                "merchant_user_id": "",
                "user_fee_amount": 0,
                "discount_amount": 0
            })
            .to_string();
            json!({"data": data, "mac": hmac_hex(&self.key2, &data), "type": 1})
        }

        pub fn fail(&self, app_trans_id: &str) {
            if let Some(o) = self.inner.lock().unwrap().orders.get_mut(app_trans_id) {
                o.failed = true;
            }
        }

        /// Tổng đã hoàn cho một giao dịch.
        pub fn refunded(&self, zp_trans_id: &str) -> i64 {
            self.inner
                .lock()
                .unwrap()
                .refunds
                .values()
                .filter(|(z, _)| z == zp_trans_id)
                .map(|(_, a)| a)
                .sum()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hmac_matches_known_vector() {
        // RFC 4231, test case 2.
        assert_eq!(
            hmac_hex("Jefe", "what do ya want for nothing?"),
            "5bdcc146bf60754e6a042426089575c75a003f089d2739839dec58b964ec3843"
        );
    }

    #[test]
    fn transaction_ids_start_with_vietnam_date() {
        // 23:30 UTC ngày 10 là 06:30 ngày 11 ở Việt Nam.
        assert_eq!(
            yymmdd(time::macros::datetime!(2026-11-10 23:30 UTC)),
            "261111"
        );
    }

    #[test]
    fn callbacks_need_the_key2_signature() {
        let zp = ZaloPay::new(&ZalopayConfig {
            app_id: 2553,
            key1: Secret::new("k1"),
            key2: Secret::new("k2"),
            endpoint: Url::parse("http://127.0.0.1:1/").unwrap(),
        });
        let data = r#"{"app_id":2553,"app_trans_id":"261111_abc","amount":9000000,"zp_trans_id":250000000001,"channel":38}"#;
        let d = zp.verify_callback(data, &hmac_hex("k2", data)).unwrap();
        assert_eq!(
            (d.amount, d.app_trans_id.as_str()),
            (9_000_000, "261111_abc")
        );
        assert!(
            zp.verify_callback(data, &hmac_hex("k1", data)).is_none(),
            "ký bằng key1"
        );
        let forged = data.replace("9000000", "9");
        assert!(
            zp.verify_callback(&forged, &hmac_hex("k2", data)).is_none(),
            "dữ liệu bị sửa"
        );
        let other_app = data.replace("2553", "2554");
        assert!(
            zp.verify_callback(&other_app, &hmac_hex("k2", &other_app))
                .is_none(),
            "app khác"
        );
    }
}
