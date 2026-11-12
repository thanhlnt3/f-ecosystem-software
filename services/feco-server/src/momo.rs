//! Adapter MoMo (developers.momo.vn, API `/v2/gateway/api/*`): tạo thanh toán ví MoMo
//! (`captureWallet`), kiểm chữ ký IPN, truy vấn, hoàn tiền và truy vấn hoàn tiền. Chữ ký là
//! HMAC-SHA256 (hex) bằng `secretKey` của chuỗi `key=value` nối bằng `&`, khóa sắp a→z theo tài
//! liệu. MoMo cần IPN trả HTTP 204.

use feco_config::{MomoConfig, Secret};
use serde_json::{json, Value};
use url::Url;

use crate::gateways::{GatewayError, PayStatus, RefundStatus};

pub const METHODS: [&str; 1] = ["momo_wallet"];

fn hmac(key: &str, input: &str) -> String {
    crate::zalopay::hmac_hex(key, input)
}

fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

fn s(v: &Value, k: &str) -> String {
    match &v[k] {
        Value::String(x) => x.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

/// Chuỗi ký của IPN (thứ tự trong tài liệu MoMo).
pub fn ipn_raw(access_key: &str, b: &Value) -> String {
    format!(
        "accessKey={access_key}&amount={}&extraData={}&message={}&orderId={}&orderInfo={}&orderType={}&partnerCode={}&payType={}&requestId={}&responseTime={}&resultCode={}&transId={}",
        s(b, "amount"),
        s(b, "extraData"),
        s(b, "message"),
        s(b, "orderId"),
        s(b, "orderInfo"),
        s(b, "orderType"),
        s(b, "partnerCode"),
        s(b, "payType"),
        s(b, "requestId"),
        s(b, "responseTime"),
        s(b, "resultCode"),
        s(b, "transId"),
    )
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipn {
    pub order_id: String,
    pub amount: i64,
    pub trans_id: String,
    pub succeeded: bool,
}

#[derive(Clone)]
pub struct Momo {
    partner_code: String,
    access_key: Secret,
    secret_key: Secret,
    base: Url,
    http: reqwest::Client,
}

impl Momo {
    pub fn new(c: &MomoConfig) -> Self {
        let mut base = c.endpoint.clone();
        base.set_path("/");
        base.set_query(None);
        Momo {
            partner_code: c.partner_code.clone(),
            access_key: c.access_key.clone(),
            secret_key: c.secret_key.clone(),
            base,
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("tạo HTTP client"),
        }
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value, GatewayError> {
        let url = self
            .base
            .join(path)
            .map_err(|e| GatewayError::Http(e.to_string()))?;
        let res = self
            .http
            .post(url)
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::Http(e.without_url().to_string()))?;
        res.json()
            .await
            .map_err(|e| GatewayError::Http(e.to_string()))
    }

    /// Tạo thanh toán ví MoMo; trả `payUrl` để chuyển khách tới.
    pub async fn create(
        &self,
        order_id: &str,
        amount: i64,
        info: &str,
        redirect_url: &str,
        ipn_url: &str,
        expire_minutes: i64,
    ) -> Result<String, GatewayError> {
        let request_type = "captureWallet";
        let raw = format!(
            "accessKey={}&amount={amount}&extraData=&ipnUrl={ipn_url}&orderId={order_id}&orderInfo={info}&partnerCode={}&redirectUrl={redirect_url}&requestId={order_id}&requestType={request_type}",
            self.access_key.expose(),
            self.partner_code
        );
        let v = self
            .post(
                "v2/gateway/api/create",
                &json!({
                    "partnerCode": self.partner_code,
                    "requestId": order_id,
                    "amount": amount,
                    "orderId": order_id,
                    "orderInfo": info,
                    "redirectUrl": redirect_url,
                    "ipnUrl": ipn_url,
                    "requestType": request_type,
                    "extraData": "",
                    "lang": "vi",
                    "orderExpireTime": expire_minutes,
                    "signature": hmac(self.secret_key.expose(), &raw),
                }),
            )
            .await?;
        if v["resultCode"].as_i64() != Some(0) {
            return Err(GatewayError::Rejected {
                code: s(&v, "resultCode"),
                message: s(&v, "message"),
            });
        }
        Ok(s(&v, "payUrl"))
    }

    /// Kiểm chữ ký IPN. Thành công khi `resultCode` là 0 (hoặc 9000: đã cấp quyền, thanh toán một bước).
    pub fn verify_ipn(&self, b: &Value) -> Option<Ipn> {
        if !same(
            &hmac(
                self.secret_key.expose(),
                &ipn_raw(self.access_key.expose(), b),
            ),
            &s(b, "signature"),
        ) || s(b, "partnerCode") != self.partner_code
        {
            return None;
        }
        Some(Ipn {
            order_id: s(b, "orderId"),
            amount: s(b, "amount").parse().ok()?,
            trans_id: s(b, "transId"),
            succeeded: matches!(b["resultCode"].as_i64(), Some(0 | 9000)),
        })
    }

    pub async fn query(&self, order_id: &str) -> Result<PayStatus, GatewayError> {
        let request_id = uuid::Uuid::new_v4().simple().to_string();
        let raw = format!(
            "accessKey={}&orderId={order_id}&partnerCode={}&requestId={request_id}",
            self.access_key.expose(),
            self.partner_code
        );
        let v = self
            .post(
                "v2/gateway/api/query",
                &json!({"partnerCode": self.partner_code, "requestId": request_id, "orderId": order_id, "lang": "vi", "signature": hmac(self.secret_key.expose(), &raw)}),
            )
            .await?;
        Ok(match v["resultCode"].as_i64() {
            Some(0 | 9000) => PayStatus::Paid {
                gateway_trans_id: s(&v, "transId"),
                amount: v["amount"].as_i64().unwrap_or(0),
            },
            Some(1000 | 7000 | 7002) => PayStatus::Pending { processing: true },
            // Lỗi của chính yêu cầu (không phải kết quả giao dịch): coi như chưa biết.
            Some(10 | 11 | 12 | 13 | 20 | 21 | 22 | 40 | 41 | 42 | 43 | 45 | 47) | None => {
                PayStatus::Pending { processing: false }
            }
            Some(_) => PayStatus::Failed,
        })
    }

    /// Hoàn tiền (toàn phần hoặc một phần). `refund_id` là mã đơn mới cho lệnh hoàn.
    pub async fn refund(
        &self,
        refund_id: &str,
        trans_id: &str,
        amount: i64,
        description: &str,
    ) -> Result<RefundStatus, GatewayError> {
        let raw = format!(
            "accessKey={}&amount={amount}&description={description}&orderId={refund_id}&partnerCode={}&requestId={refund_id}&transId={trans_id}",
            self.access_key.expose(),
            self.partner_code
        );
        let v = self
            .post(
                "v2/gateway/api/refund",
                &json!({
                    "partnerCode": self.partner_code,
                    "orderId": refund_id,
                    "requestId": refund_id,
                    "amount": amount,
                    "transId": trans_id.parse::<i64>().map(Value::from).unwrap_or(Value::from(trans_id)),
                    "lang": "vi",
                    "description": description,
                    "signature": hmac(self.secret_key.expose(), &raw),
                }),
            )
            .await?;
        Ok(match v["resultCode"].as_i64() {
            Some(0) => RefundStatus::Succeeded,
            Some(1080 | 7000 | 7002) => RefundStatus::Processing,
            _ => RefundStatus::Failed(format!("{}: {}", s(&v, "resultCode"), s(&v, "message"))),
        })
    }

    pub async fn query_refund(&self, refund_id: &str) -> Result<RefundStatus, GatewayError> {
        let request_id = uuid::Uuid::new_v4().simple().to_string();
        let raw = format!(
            "accessKey={}&orderId={refund_id}&partnerCode={}&requestId={request_id}",
            self.access_key.expose(),
            self.partner_code
        );
        let v = self
            .post(
                "v2/gateway/api/refund/query",
                &json!({"partnerCode": self.partner_code, "requestId": request_id, "orderId": refund_id, "lang": "vi", "signature": hmac(self.secret_key.expose(), &raw)}),
            )
            .await?;
        let trans = v["refundTrans"].as_array().cloned().unwrap_or_default();
        Ok(
            match trans.iter().map(|t| t["resultCode"].as_i64()).next() {
                Some(Some(0)) => RefundStatus::Succeeded,
                Some(Some(1080 | 7000 | 7002)) | None => RefundStatus::Processing,
                Some(other) => RefundStatus::Failed(format!("{other:?}")),
            },
        )
    }
}

/// Cổng MoMo giả trên loopback cho kiểm thử: cùng chuỗi ký với tài liệu MoMo.
pub mod testing {
    use std::{
        collections::HashMap,
        sync::{Arc, Mutex},
    };

    use axum::{extract::State, routing::post, Json, Router};
    use serde_json::{json, Value};

    use super::{hmac, ipn_raw, s};

    #[derive(Debug, Clone, Default)]
    pub struct FakeOrder {
        pub amount: i64,
        pub order_info: String,
        pub ipn_url: String,
        pub paid: Option<i64>,
    }

    #[derive(Clone)]
    pub struct FakeMomo {
        pub partner_code: String,
        pub access_key: String,
        pub secret_key: String,
        pub addr: std::net::SocketAddr,
        pub orders: Arc<Mutex<HashMap<String, FakeOrder>>>,
        /// Lệnh hoàn: mã lệnh → (transId, số tiền).
        pub refunds: Arc<Mutex<HashMap<String, (i64, i64)>>>,
    }

    async fn create(State(f): State<FakeMomo>, Json(b): Json<Value>) -> Json<Value> {
        let raw = format!(
            "accessKey={}&amount={}&extraData={}&ipnUrl={}&orderId={}&orderInfo={}&partnerCode={}&redirectUrl={}&requestId={}&requestType={}",
            f.access_key,
            s(&b, "amount"),
            s(&b, "extraData"),
            s(&b, "ipnUrl"),
            s(&b, "orderId"),
            s(&b, "orderInfo"),
            s(&b, "partnerCode"),
            s(&b, "redirectUrl"),
            s(&b, "requestId"),
            s(&b, "requestType"),
        );
        if hmac(&f.secret_key, &raw) != s(&b, "signature") {
            return Json(json!({"resultCode": 11, "message": "Sai chữ ký"}));
        }
        f.orders.lock().unwrap().insert(
            s(&b, "orderId"),
            FakeOrder {
                amount: b["amount"].as_i64().unwrap_or(0),
                order_info: s(&b, "orderInfo"),
                ipn_url: s(&b, "ipnUrl"),
                paid: None,
            },
        );
        Json(
            json!({"resultCode": 0, "message": "Thành công.", "payUrl": format!("http://{}/pay/{}", f.addr, s(&b, "orderId"))}),
        )
    }

    async fn query(State(f): State<FakeMomo>, Json(b): Json<Value>) -> Json<Value> {
        let raw = format!(
            "accessKey={}&orderId={}&partnerCode={}&requestId={}",
            f.access_key,
            s(&b, "orderId"),
            s(&b, "partnerCode"),
            s(&b, "requestId")
        );
        if hmac(&f.secret_key, &raw) != s(&b, "signature") {
            return Json(json!({"resultCode": 11, "message": "Sai chữ ký"}));
        }
        match f.orders.lock().unwrap().get(&s(&b, "orderId")) {
            Some(FakeOrder {
                paid: Some(t),
                amount,
                ..
            }) => Json(json!({"resultCode": 0, "transId": t, "amount": amount})),
            Some(_) => Json(json!({"resultCode": 1000, "message": "Đang chờ người dùng xác nhận"})),
            None => Json(json!({"resultCode": 42, "message": "orderId không tồn tại"})),
        }
    }

    async fn refund(State(f): State<FakeMomo>, Json(b): Json<Value>) -> Json<Value> {
        let raw = format!(
            "accessKey={}&amount={}&description={}&orderId={}&partnerCode={}&requestId={}&transId={}",
            f.access_key,
            s(&b, "amount"),
            s(&b, "description"),
            s(&b, "orderId"),
            s(&b, "partnerCode"),
            s(&b, "requestId"),
            s(&b, "transId")
        );
        if hmac(&f.secret_key, &raw) != s(&b, "signature") {
            return Json(json!({"resultCode": 11, "message": "Sai chữ ký"}));
        }
        let trans: i64 = s(&b, "transId").parse().unwrap_or(0);
        let paid: i64 = f
            .orders
            .lock()
            .unwrap()
            .values()
            .filter(|o| o.paid == Some(trans))
            .map(|o| o.amount)
            .sum();
        let mut refunds = f.refunds.lock().unwrap();
        let already: i64 = refunds
            .values()
            .filter(|(t, _)| *t == trans)
            .map(|(_, a)| a)
            .sum();
        let amount = b["amount"].as_i64().unwrap_or(0);
        if paid == 0 || already + amount > paid {
            return Json(json!({"resultCode": 1081, "message": "Số tiền hoàn vượt số đã trả"}));
        }
        refunds.insert(s(&b, "orderId"), (trans, amount));
        Json(json!({"resultCode": 0, "message": "Thành công."}))
    }

    async fn refund_query(State(f): State<FakeMomo>, Json(b): Json<Value>) -> Json<Value> {
        match f.refunds.lock().unwrap().get(&s(&b, "orderId")) {
            Some((t, a)) => Json(
                json!({"resultCode": 0, "refundTrans": [{"orderId": s(&b, "orderId"), "amount": a, "resultCode": 0, "transId": t}]}),
            ),
            None => Json(json!({"resultCode": 0, "refundTrans": []})),
        }
    }

    impl FakeMomo {
        pub async fn serve(
            partner_code: &str,
            access_key: &str,
            secret_key: &str,
        ) -> std::io::Result<FakeMomo> {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let fake = FakeMomo {
                partner_code: partner_code.to_owned(),
                access_key: access_key.to_owned(),
                secret_key: secret_key.to_owned(),
                addr: listener.local_addr()?,
                orders: Arc::default(),
                refunds: Arc::default(),
            };
            let app = Router::new()
                .route("/v2/gateway/api/create", post(create))
                .route("/v2/gateway/api/query", post(query))
                .route("/v2/gateway/api/refund", post(refund))
                .route("/v2/gateway/api/refund/query", post(refund_query))
                .with_state(fake.clone());
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Ok(fake)
        }

        pub fn endpoint(&self) -> String {
            format!("http://{}/v2/gateway/api/create", self.addr)
        }

        pub fn order(&self, order_id: &str) -> Option<FakeOrder> {
            self.orders.lock().unwrap().get(order_id).cloned()
        }

        /// Khách xác nhận trên ứng dụng MoMo: trả thân IPN đã ký.
        pub fn pay(&self, order_id: &str, trans_id: i64) -> Value {
            let mut orders = self.orders.lock().unwrap();
            let o = orders.get_mut(order_id).expect("có đơn trên cổng giả");
            o.paid = Some(trans_id);
            let mut b = json!({
                "partnerCode": self.partner_code,
                "orderId": order_id,
                "requestId": order_id,
                "amount": o.amount,
                "orderInfo": o.order_info,
                "orderType": "momo_wallet",
                "transId": trans_id,
                "resultCode": 0,
                "message": "Thành công.",
                "payType": "qr",
                "responseTime": 1794445000000i64,
                "extraData": "",
            });
            b["signature"] = json!(hmac(&self.secret_key, &ipn_raw(&self.access_key, &b)));
            b
        }

        pub fn refunded(&self, trans_id: i64) -> i64 {
            self.refunds
                .lock()
                .unwrap()
                .values()
                .filter(|(t, _)| *t == trans_id)
                .map(|(_, a)| a)
                .sum()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ipn_signature_is_checked() {
        let m = Momo::new(&MomoConfig {
            partner_code: "MOMOTEST".into(),
            access_key: Secret::new("ak"),
            secret_key: Secret::new("sk"),
            endpoint: Url::parse("https://test-payment.momo.vn/v2/gateway/api/create").unwrap(),
        });
        let mut b = json!({"partnerCode": "MOMOTEST", "orderId": "o1", "requestId": "o1", "amount": 5000, "orderInfo": "x", "orderType": "momo_wallet",
            "transId": 99, "resultCode": 0, "message": "ok", "payType": "qr", "responseTime": 1, "extraData": ""});
        b["signature"] = json!(hmac("sk", &ipn_raw("ak", &b)));
        let ipn = m.verify_ipn(&b).unwrap();
        assert!(ipn.succeeded && ipn.amount == 5000 && ipn.trans_id == "99");
        b["amount"] = json!(50);
        assert!(m.verify_ipn(&b).is_none());
        assert_eq!(m.base.as_str(), "https://test-payment.momo.vn/");
    }
}
