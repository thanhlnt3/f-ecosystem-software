//! Adapter VNPAY (tài liệu sandbox.vnpayment.vn/apis, phiên bản 2.1.0):
//! - Thanh toán: chuyển khách tới `vpcpay.html` với các tham số `vnp_*` sắp tăng dần theo tên,
//!   mã hóa kiểu form (dấu cách thành `+`), ký HMAC-SHA512 bằng `vnp_HashSecret`.
//! - IPN: VNPAY gọi GET tới máy chủ F-Eco với cùng cách ký; F-Eco trả JSON `RspCode`.
//! - Truy vấn (`querydr`) và hoàn tiền (`refund`): POST JSON tới `merchant_webapi/api/transaction`,
//!   chữ ký là HMAC-SHA512 của các trường nối bằng `|` theo thứ tự trong tài liệu.
//!
//! Số tiền gửi VNPAY nhân 100. Thời gian theo GMT+7, dạng `yyyyMMddHHmmss`.

use std::collections::BTreeMap;

use feco_config::{Secret, VnpayConfig};
use hmac::{Hmac, KeyInit, Mac};
use serde_json::{json, Value};
use sha2::Sha512;
use time::{OffsetDateTime, UtcOffset};
use url::Url;

use crate::gateways::{GatewayError, PayStatus, RefundStatus};

pub const VERSION: &str = "2.1.0";

pub fn hmac512(key: &str, input: &str) -> String {
    let mut mac =
        Hmac::<Sha512>::new_from_slice(key.as_bytes()).expect("HMAC nhận khóa mọi độ dài");
    mac.update(input.as_bytes());
    mac.finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

fn same(a: &str, b: &str) -> bool {
    let (a, b) = (a.to_ascii_lowercase(), b.to_ascii_lowercase());
    a.len() == b.len()
        && a.bytes()
            .zip(b.bytes())
            .fold(0u8, |acc, (x, y)| acc | (x ^ y))
            == 0
}

/// Thời gian GMT+7 dạng `yyyyMMddHHmmss`.
pub fn vn_time(t: OffsetDateTime) -> String {
    let v = t.to_offset(UtcOffset::from_hms(7, 0, 0).expect("GMT+7"));
    format!(
        "{:04}{:02}{:02}{:02}{:02}{:02}",
        v.year(),
        u8::from(v.month()),
        v.day(),
        v.hour(),
        v.minute(),
        v.second()
    )
}

/// Chuỗi ký của URL thanh toán và IPN: tham số `vnp_*` (trừ chữ ký) sắp theo tên, mã hóa form.
pub fn sign_data(params: &BTreeMap<String, String>) -> String {
    form_urlencoded::Serializer::new(String::new())
        .extend_pairs(params.iter().filter(|(k, v)| {
            k.starts_with("vnp_")
                && *k != "vnp_SecureHash"
                && *k != "vnp_SecureHashType"
                && !v.is_empty()
        }))
        .finish()
}

/// Bỏ dấu tiếng Việt và ký tự đặc biệt (VNPAY yêu cầu `vnp_OrderInfo` không dấu).
pub fn ascii(s: &str) -> String {
    const MAP: &[(&str, char)] = &[
        ("àáạảãâầấậẩẫăằắặẳẵ", 'a'),
        ("ÀÁẠẢÃÂẦẤẬẨẪĂẰẮẶẲẴ", 'A'),
        ("èéẹẻẽêềếệểễ", 'e'),
        ("ÈÉẸẺẼÊỀẾỆỂỄ", 'E'),
        ("ìíịỉĩ", 'i'),
        ("ÌÍỊỈĨ", 'I'),
        ("òóọỏõôồốộổỗơờớợởỡ", 'o'),
        ("ÒÓỌỎÕÔỒỐỘỔỖƠỜỚỢỞỠ", 'O'),
        ("ùúụủũưừứựửữ", 'u'),
        ("ÙÚỤỦŨƯỪỨỰỬỮ", 'U'),
        ("ỳýỵỷỹ", 'y'),
        ("ỲÝỴỶỸ", 'Y'),
        ("đ", 'd'),
        ("Đ", 'D'),
    ];
    s.chars()
        .map(|c| {
            MAP.iter()
                .find(|(set, _)| set.contains(c))
                .map_or(c, |(_, r)| *r)
        })
        .filter(|c| c.is_ascii_alphanumeric() || matches!(c, ' ' | '-' | '.' | ':' | ','))
        .collect()
}

/// Mã ngân hàng VNPAY theo phương thức khách chọn.
pub fn bank_code(method: &str) -> Option<&'static str> {
    match method {
        "vietqr" => Some("VNPAYQR"),
        "domestic_card" => Some("VNBANK"),
        "international_card" => Some("INTCARD"),
        _ => None,
    }
}

pub const METHODS: [&str; 3] = ["vietqr", "domestic_card", "international_card"];

/// Kết quả IPN đã kiểm chữ ký.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipn {
    pub txn_ref: String,
    pub amount: i64,
    pub transaction_no: String,
    pub pay_date: String,
    pub succeeded: bool,
    pub card_type: String,
}

#[derive(Clone)]
pub struct Vnpay {
    tmn_code: String,
    secret: Secret,
    payment_url: Url,
    api_url: Url,
    http: reqwest::Client,
}

impl Vnpay {
    pub fn new(c: &VnpayConfig) -> Self {
        Vnpay {
            tmn_code: c.tmn_code.clone(),
            secret: c.hash_secret.clone(),
            payment_url: c.payment_url.clone(),
            api_url: c.api_url.clone(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("tạo HTTP client"),
        }
    }

    /// Địa chỉ trang thanh toán của VNPAY cho một giao dịch.
    #[allow(clippy::too_many_arguments)]
    pub fn payment_url(
        &self,
        txn_ref: &str,
        amount: i64,
        info: &str,
        method: &str,
        return_url: &str,
        ip: &str,
        now: OffsetDateTime,
        expire_minutes: i64,
    ) -> String {
        let mut p: BTreeMap<String, String> = BTreeMap::new();
        let mut put = |k: &str, v: String| {
            p.insert(k.to_owned(), v);
        };
        put("vnp_Version", VERSION.into());
        put("vnp_Command", "pay".into());
        put("vnp_TmnCode", self.tmn_code.clone());
        put("vnp_Amount", (amount * 100).to_string());
        if let Some(b) = bank_code(method) {
            put("vnp_BankCode", b.into());
        }
        put("vnp_CreateDate", vn_time(now));
        put("vnp_CurrCode", "VND".into());
        put("vnp_IpAddr", ip.into());
        put("vnp_Locale", "vn".into());
        put("vnp_OrderInfo", ascii(info).chars().take(255).collect());
        put("vnp_OrderType", "other".into());
        put("vnp_ReturnUrl", return_url.into());
        put(
            "vnp_ExpireDate",
            vn_time(now + time::Duration::minutes(expire_minutes)),
        );
        put("vnp_TxnRef", txn_ref.into());
        let data = sign_data(&p);
        format!(
            "{}?{data}&vnp_SecureHash={}",
            self.payment_url,
            hmac512(self.secret.expose(), &data)
        )
    }

    /// Kiểm chữ ký IPN (hoặc trang chuyển về) rồi đọc kết quả. Thành công khi cả
    /// `vnp_ResponseCode` và `vnp_TransactionStatus` là `00`.
    pub fn verify_ipn(&self, params: &BTreeMap<String, String>) -> Option<Ipn> {
        let hash = params.get("vnp_SecureHash")?;
        if !same(&hmac512(self.secret.expose(), &sign_data(params)), hash)
            || params.get("vnp_TmnCode") != Some(&self.tmn_code)
        {
            return None;
        }
        let g = |k: &str| params.get(k).cloned().unwrap_or_default();
        Some(Ipn {
            txn_ref: g("vnp_TxnRef"),
            amount: g("vnp_Amount").parse::<i64>().ok()? / 100,
            transaction_no: g("vnp_TransactionNo"),
            pay_date: g("vnp_PayDate"),
            succeeded: g("vnp_ResponseCode") == "00" && g("vnp_TransactionStatus") == "00",
            card_type: g("vnp_CardType"),
        })
    }

    async fn call(&self, body: &Value) -> Result<Value, GatewayError> {
        let res = self
            .http
            .post(self.api_url.clone())
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::Http(e.without_url().to_string()))?;
        if !res.status().is_success() {
            return Err(GatewayError::Http(format!("HTTP {}", res.status())));
        }
        res.json()
            .await
            .map_err(|e| GatewayError::Http(e.to_string()))
    }

    fn check_response(&self, v: &Value, fields: &[&str]) -> Result<(), GatewayError> {
        let s = |k: &str| match &v[k] {
            Value::String(x) => x.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        };
        let data = fields.iter().map(|f| s(f)).collect::<Vec<_>>().join("|");
        if !same(&hmac512(self.secret.expose(), &data), &s("vnp_SecureHash")) {
            return Err(GatewayError::BadSignature);
        }
        Ok(())
    }

    /// Truy vấn trạng thái (`querydr`). `create_date` là `vnp_CreateDate` của lần thanh toán.
    pub async fn query(
        &self,
        txn_ref: &str,
        create_date: &str,
        now: OffsetDateTime,
        server_ip: &str,
    ) -> Result<PayStatus, GatewayError> {
        let request_id = uuid::Uuid::new_v4().simple().to_string();
        let info = format!("Truy van {txn_ref}");
        let created = vn_time(now);
        let data = [
            request_id.as_str(),
            VERSION,
            "querydr",
            &self.tmn_code,
            txn_ref,
            create_date,
            &created,
            server_ip,
            &info,
        ]
        .join("|");
        let v = self
            .call(&json!({
                "vnp_RequestId": request_id,
                "vnp_Version": VERSION,
                "vnp_Command": "querydr",
                "vnp_TmnCode": self.tmn_code,
                "vnp_TxnRef": txn_ref,
                "vnp_OrderInfo": info,
                "vnp_TransactionDate": create_date,
                "vnp_CreateDate": created,
                "vnp_IpAddr": server_ip,
                "vnp_SecureHash": hmac512(self.secret.expose(), &data),
            }))
            .await?;
        let code = v["vnp_ResponseCode"].as_str().unwrap_or_default();
        if code == "91" {
            return Ok(PayStatus::Pending { processing: false });
        }
        if code != "00" {
            return Err(GatewayError::Rejected {
                code: code.to_owned(),
                message: v["vnp_Message"].as_str().unwrap_or_default().to_owned(),
            });
        }
        self.check_response(
            &v,
            &[
                "vnp_ResponseId",
                "vnp_Command",
                "vnp_ResponseCode",
                "vnp_Message",
                "vnp_TmnCode",
                "vnp_TxnRef",
                "vnp_Amount",
                "vnp_BankCode",
                "vnp_PayDate",
                "vnp_TransactionNo",
                "vnp_TransactionType",
                "vnp_TransactionStatus",
                "vnp_OrderInfo",
                "vnp_PromotionCode",
                "vnp_PromotionAmount",
            ],
        )?;
        let amount = v["vnp_Amount"]
            .as_str()
            .and_then(|a| a.parse::<i64>().ok())
            .or(v["vnp_Amount"].as_i64())
            .unwrap_or(0)
            / 100;
        Ok(
            match v["vnp_TransactionStatus"].as_str().unwrap_or_default() {
                "00" => PayStatus::Paid {
                    gateway_trans_id: v["vnp_TransactionNo"]
                        .as_str()
                        .map(str::to_owned)
                        .unwrap_or_else(|| v["vnp_TransactionNo"].to_string()),
                    amount,
                },
                "01" => PayStatus::Pending { processing: false },
                _ => PayStatus::Failed,
            },
        )
    }

    /// Hoàn tiền toàn phần (`02`) hoặc một phần (`03`); VNPAY trả kết quả ngay.
    #[allow(clippy::too_many_arguments)]
    pub async fn refund(
        &self,
        txn_ref: &str,
        transaction_no: &str,
        create_date: &str,
        amount: i64,
        full: bool,
        by: &str,
        now: OffsetDateTime,
        server_ip: &str,
    ) -> Result<RefundStatus, GatewayError> {
        let request_id = uuid::Uuid::new_v4().simple().to_string();
        let kind = if full { "02" } else { "03" };
        let amount = (amount * 100).to_string();
        let info = format!("Hoan tien {txn_ref}");
        let created = vn_time(now);
        let by = ascii(by);
        let data = [
            request_id.as_str(),
            VERSION,
            "refund",
            &self.tmn_code,
            kind,
            txn_ref,
            &amount,
            transaction_no,
            create_date,
            &by,
            &created,
            server_ip,
            &info,
        ]
        .join("|");
        let v = self
            .call(&json!({
                "vnp_RequestId": request_id,
                "vnp_Version": VERSION,
                "vnp_Command": "refund",
                "vnp_TmnCode": self.tmn_code,
                "vnp_TransactionType": kind,
                "vnp_TxnRef": txn_ref,
                "vnp_Amount": amount,
                "vnp_OrderInfo": info,
                "vnp_TransactionNo": transaction_no,
                "vnp_TransactionDate": create_date,
                "vnp_CreateBy": by,
                "vnp_CreateDate": created,
                "vnp_IpAddr": server_ip,
                "vnp_SecureHash": hmac512(self.secret.expose(), &data),
            }))
            .await?;
        Ok(match v["vnp_ResponseCode"].as_str().unwrap_or_default() {
            "00" => {
                self.check_response(
                    &v,
                    &[
                        "vnp_ResponseId",
                        "vnp_Command",
                        "vnp_ResponseCode",
                        "vnp_Message",
                        "vnp_TmnCode",
                        "vnp_TxnRef",
                        "vnp_Amount",
                        "vnp_BankCode",
                        "vnp_PayDate",
                        "vnp_TransactionNo",
                        "vnp_TransactionType",
                        "vnp_TransactionStatus",
                        "vnp_OrderInfo",
                    ],
                )?;
                RefundStatus::Succeeded
            }
            "94" => RefundStatus::Processing,
            code => RefundStatus::Failed(format!(
                "{code}: {}",
                v["vnp_Message"].as_str().unwrap_or_default()
            )),
        })
    }
}

/// Cổng VNPAY giả trên loopback cho kiểm thử: cùng cách ký với tài liệu VNPAY.
pub mod testing {
    use std::{
        collections::{BTreeMap, HashMap},
        sync::{Arc, Mutex},
    };

    use axum::{extract::State, routing::post, Json, Router};
    use serde_json::{json, Value};

    use super::{hmac512, sign_data};

    #[derive(Debug, Clone, Default)]
    pub struct FakeTxn {
        pub amount: i64,
        pub create_date: String,
        pub bank_code: String,
        pub return_url: String,
        pub paid: Option<String>,
        pub refunded: i64,
    }

    #[derive(Clone)]
    pub struct FakeVnpay {
        pub tmn_code: String,
        pub secret: String,
        pub addr: std::net::SocketAddr,
        pub txns: Arc<Mutex<HashMap<String, FakeTxn>>>,
        next: Arc<Mutex<i64>>,
    }

    fn s(v: &Value, k: &str) -> String {
        match &v[k] {
            Value::String(x) => x.clone(),
            Value::Null => String::new(),
            other => other.to_string(),
        }
    }

    async fn api(State(f): State<FakeVnpay>, Json(b): Json<Value>) -> Json<Value> {
        let cmd = s(&b, "vnp_Command");
        let fields: &[&str] = if cmd == "querydr" {
            &[
                "vnp_RequestId",
                "vnp_Version",
                "vnp_Command",
                "vnp_TmnCode",
                "vnp_TxnRef",
                "vnp_TransactionDate",
                "vnp_CreateDate",
                "vnp_IpAddr",
                "vnp_OrderInfo",
            ]
        } else {
            &[
                "vnp_RequestId",
                "vnp_Version",
                "vnp_Command",
                "vnp_TmnCode",
                "vnp_TransactionType",
                "vnp_TxnRef",
                "vnp_Amount",
                "vnp_TransactionNo",
                "vnp_TransactionDate",
                "vnp_CreateBy",
                "vnp_CreateDate",
                "vnp_IpAddr",
                "vnp_OrderInfo",
            ]
        };
        let data = fields
            .iter()
            .map(|k| s(&b, k))
            .collect::<Vec<_>>()
            .join("|");
        if hmac512(&f.secret, &data) != s(&b, "vnp_SecureHash") {
            return Json(json!({"vnp_ResponseCode": "97", "vnp_Message": "Invalid Checksum"}));
        }
        let mut txns = f.txns.lock().unwrap();
        let Some(t) = txns.get_mut(&s(&b, "vnp_TxnRef")) else {
            return Json(json!({"vnp_ResponseCode": "91", "vnp_Message": "Transaction not found"}));
        };
        let mut res = json!({
            "vnp_ResponseId": "r1",
            "vnp_Command": cmd,
            "vnp_ResponseCode": "00",
            "vnp_Message": "OK",
            "vnp_TmnCode": f.tmn_code,
            "vnp_TxnRef": s(&b, "vnp_TxnRef"),
            "vnp_BankCode": t.bank_code,
            "vnp_PayDate": "20261112100000",
            "vnp_TransactionNo": t.paid.clone().unwrap_or_default(),
            "vnp_OrderInfo": s(&b, "vnp_OrderInfo"),
        });
        let fields: Vec<&str> = if cmd == "querydr" {
            res["vnp_Amount"] = json!((t.amount * 100).to_string());
            res["vnp_TransactionType"] = json!("01");
            res["vnp_TransactionStatus"] = json!(if t.paid.is_some() { "00" } else { "01" });
            res["vnp_PromotionCode"] = json!("");
            res["vnp_PromotionAmount"] = json!("");
            vec![
                "vnp_ResponseId",
                "vnp_Command",
                "vnp_ResponseCode",
                "vnp_Message",
                "vnp_TmnCode",
                "vnp_TxnRef",
                "vnp_Amount",
                "vnp_BankCode",
                "vnp_PayDate",
                "vnp_TransactionNo",
                "vnp_TransactionType",
                "vnp_TransactionStatus",
                "vnp_OrderInfo",
                "vnp_PromotionCode",
                "vnp_PromotionAmount",
            ]
        } else {
            let amount = s(&b, "vnp_Amount").parse::<i64>().unwrap_or(0) / 100;
            if t.paid.as_deref() != Some(s(&b, "vnp_TransactionNo").as_str())
                || t.refunded + amount > t.amount
            {
                return Json(json!({"vnp_ResponseCode": "02", "vnp_Message": "Invalid amount"}));
            }
            t.refunded += amount;
            res["vnp_Amount"] = json!(s(&b, "vnp_Amount"));
            res["vnp_TransactionType"] = json!(s(&b, "vnp_TransactionType"));
            res["vnp_TransactionStatus"] = json!("05");
            vec![
                "vnp_ResponseId",
                "vnp_Command",
                "vnp_ResponseCode",
                "vnp_Message",
                "vnp_TmnCode",
                "vnp_TxnRef",
                "vnp_Amount",
                "vnp_BankCode",
                "vnp_PayDate",
                "vnp_TransactionNo",
                "vnp_TransactionType",
                "vnp_TransactionStatus",
                "vnp_OrderInfo",
            ]
        };
        let data = fields
            .iter()
            .map(|k| s(&res, k))
            .collect::<Vec<_>>()
            .join("|");
        res["vnp_SecureHash"] = json!(hmac512(&f.secret, &data));
        Json(res)
    }

    impl FakeVnpay {
        pub async fn serve(tmn_code: &str, secret: &str) -> std::io::Result<FakeVnpay> {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let fake = FakeVnpay {
                tmn_code: tmn_code.to_owned(),
                secret: secret.to_owned(),
                addr: listener.local_addr()?,
                txns: Arc::default(),
                next: Arc::default(),
            };
            let app = Router::new()
                .route("/merchant_webapi/api/transaction", post(api))
                .with_state(fake.clone());
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Ok(fake)
        }

        pub fn payment_url(&self) -> String {
            format!("http://{}/paymentv2/vpcpay.html", self.addr)
        }

        pub fn api_url(&self) -> String {
            format!("http://{}/merchant_webapi/api/transaction", self.addr)
        }

        /// Khách mở URL thanh toán: cổng kiểm chữ ký rồi ghi nhận giao dịch chờ trả.
        pub fn open(&self, url: &str) -> Result<String, String> {
            let u = url::Url::parse(url).map_err(|e| e.to_string())?;
            let p: BTreeMap<String, String> = u.query_pairs().into_owned().collect();
            if hmac512(&self.secret, &sign_data(&p))
                != p.get("vnp_SecureHash").cloned().unwrap_or_default()
            {
                return Err("sai chữ ký".into());
            }
            let r = p["vnp_TxnRef"].clone();
            self.txns.lock().unwrap().insert(
                r.clone(),
                FakeTxn {
                    amount: p["vnp_Amount"].parse::<i64>().unwrap_or(0) / 100,
                    create_date: p["vnp_CreateDate"].clone(),
                    bank_code: p.get("vnp_BankCode").cloned().unwrap_or_default(),
                    return_url: p["vnp_ReturnUrl"].clone(),
                    ..Default::default()
                },
            );
            Ok(r)
        }

        /// Khách trả xong: trả về tham số IPN đã ký (VNPAY gọi GET tới IPN URL với chúng).
        pub fn pay(&self, txn_ref: &str, card_type: &str) -> BTreeMap<String, String> {
            let mut n = self.next.lock().unwrap();
            *n += 1;
            let no = format!("{}", 14_000_000 + *n);
            let mut txns = self.txns.lock().unwrap();
            let t = txns.get_mut(txn_ref).expect("có giao dịch trên cổng giả");
            t.paid = Some(no.clone());
            let mut p: BTreeMap<String, String> = BTreeMap::new();
            for (k, v) in [
                ("vnp_TmnCode", self.tmn_code.clone()),
                ("vnp_Amount", (t.amount * 100).to_string()),
                ("vnp_BankCode", "NCB".into()),
                ("vnp_CardType", card_type.into()),
                ("vnp_OrderInfo", "Thanh toan".into()),
                ("vnp_PayDate", "20261112100000".into()),
                ("vnp_ResponseCode", "00".into()),
                ("vnp_TransactionNo", no),
                ("vnp_TransactionStatus", "00".into()),
                ("vnp_TxnRef", txn_ref.into()),
            ] {
                p.insert(k.into(), v);
            }
            let hash = hmac512(&self.secret, &sign_data(&p));
            p.insert("vnp_SecureHash".into(), hash);
            p
        }

        pub fn refunded(&self, txn_ref: &str) -> i64 {
            self.txns
                .lock()
                .unwrap()
                .get(txn_ref)
                .map_or(0, |t| t.refunded)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sign_data_sorts_and_form_encodes() {
        let mut p = BTreeMap::new();
        p.insert("vnp_OrderInfo".to_owned(), "Thanh toan don 1".to_owned());
        p.insert("vnp_Amount".to_owned(), "300000".to_owned());
        p.insert(
            "vnp_ReturnUrl".to_owned(),
            "https://f-eco.vn/payments/return".to_owned(),
        );
        p.insert("vnp_SecureHash".to_owned(), "x".to_owned());
        assert_eq!(sign_data(&p), "vnp_Amount=300000&vnp_OrderInfo=Thanh+toan+don+1&vnp_ReturnUrl=https%3A%2F%2Ff-eco.vn%2Fpayments%2Freturn");
    }

    #[test]
    fn hmac512_known_vector() {
        // RFC 4231, test case 2.
        assert_eq!(
            hmac512("Jefe", "what do ya want for nothing?"),
            "164b7a7bfcf819e2e395fbe73b56e0a387bd64222e831fd610270cd7ea2505549758bf75c05a994a6d034f65f8f0e6fdcaeab1a34d4a6b4b636e070a38bce737"
        );
    }

    #[test]
    fn vietnamese_text_loses_its_marks() {
        assert_eq!(
            ascii("Đơn DH-000017 · mốc 1: Đặt cọc"),
            "Don DH-000017  moc 1: Dat coc"
        );
        assert_eq!(
            vn_time(time::macros::datetime!(2026-11-11 23:30:05 UTC)),
            "20261112063005"
        );
    }

    #[test]
    fn ipn_needs_the_secret() {
        let v = Vnpay::new(&VnpayConfig {
            tmn_code: "DEMO0001".into(),
            hash_secret: Secret::new("bi-mat"),
            payment_url: Url::parse("https://sandbox.vnpayment.vn/paymentv2/vpcpay.html").unwrap(),
            api_url: Url::parse("https://sandbox.vnpayment.vn/merchant_webapi/api/transaction")
                .unwrap(),
        });
        let url = v.payment_url(
            "261112abc",
            3000,
            "Đặt cọc",
            "vietqr",
            "https://f-eco.vn/payments/return",
            "1.2.3.4",
            time::macros::datetime!(2026-11-12 01:00 UTC),
            15,
        );
        assert!(
            url.contains("vnp_Amount=300000")
                && url.contains("vnp_BankCode=VNPAYQR")
                && url.contains("vnp_ExpireDate=20261112081500"),
            "{url}"
        );
        let mut p: BTreeMap<String, String> = [
            ("vnp_TmnCode", "DEMO0001"),
            ("vnp_Amount", "300000"),
            ("vnp_ResponseCode", "00"),
            ("vnp_TransactionStatus", "00"),
            ("vnp_TxnRef", "261112abc"),
            ("vnp_TransactionNo", "14000001"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect();
        p.insert("vnp_SecureHash".into(), hmac512("bi-mat", &sign_data(&p)));
        let ipn = v.verify_ipn(&p).unwrap();
        assert!(ipn.succeeded && ipn.amount == 3000);
        p.insert("vnp_Amount".into(), "30000000".into());
        assert!(v.verify_ipn(&p).is_none(), "số tiền bị sửa");
    }
}
