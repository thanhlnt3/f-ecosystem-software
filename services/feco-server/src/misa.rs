//! Adapter hóa đơn điện tử MISA meInvoice (MISA Developer Portal, `apis/itg/meinvoice`):
//! - `POST invoice/token` (header `ClientID`, `ClientSecret`; thân `taxcode`, `username`,
//!   `password`) lấy token (hạn 14 ngày), gửi kèm `Authorization: Bearer` và `ClientID`.
//! - `POST invoice/publishing` với `SignType` 2 (ký số trên máy chủ, HSM) phát hành hóa đơn; hóa
//!   đơn điều chỉnh dùng `ReferenceType` 2 kèm số, ký hiệu, ngày của hóa đơn gốc.
//! - `POST invoice/Download?downloadDataType=Pdf` tải PDF (base64) theo `TransactionID` (mã tra cứu).

use std::sync::Mutex;

use feco_config::MisaConfig;
use serde_json::{json, Value};
use time::{Duration, OffsetDateTime};

#[derive(Debug, thiserror::Error)]
pub enum MisaError {
    #[error("không gọi được MISA meInvoice: {0}")]
    Http(String),
    #[error("MISA meInvoice từ chối: {0}")]
    Rejected(String),
}

/// Người mua trên hóa đơn.
#[derive(Debug, Clone)]
pub struct Buyer {
    pub company: bool,
    pub name: String,
    pub tax_code: Option<String>,
    pub address: String,
    pub email: String,
}

/// Hóa đơn gốc mà hóa đơn điều chỉnh tham chiếu.
#[derive(Debug, Clone)]
pub struct Original {
    pub series: String,
    pub number: String,
    pub date: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Published {
    /// Mã tra cứu của hóa đơn.
    pub transaction_id: String,
    pub series: String,
    pub number: String,
    pub date: String,
}

pub struct Misa {
    cfg: MisaConfig,
    http: reqwest::Client,
    token: Mutex<Option<(String, OffsetDateTime)>>,
}

fn num_words(n: i64) -> String {
    // Số tiền bằng chữ do MISA tự sinh khi để trống; F-Eco gửi kèm dạng số cho dễ đối chiếu.
    format!("{} đồng", crate::orders::vnd(n).trim_end_matches(" ₫"))
}

impl Misa {
    pub fn new(cfg: &MisaConfig) -> Self {
        Misa {
            cfg: cfg.clone(),
            http: reqwest::Client::builder()
                .timeout(std::time::Duration::from_secs(30))
                .build()
                .expect("tạo HTTP client"),
            token: Mutex::new(None),
        }
    }

    async fn token(&self, force: bool) -> Result<String, MisaError> {
        if !force {
            if let Some((t, until)) = self.token.lock().expect("khóa token").clone() {
                if OffsetDateTime::now_utc() < until {
                    return Ok(t);
                }
            }
        }
        let url = self
            .cfg
            .endpoint
            .join("invoice/token")
            .map_err(|e| MisaError::Http(e.to_string()))?;
        let v: Value = self
            .http
            .post(url)
            .header("ClientID", &self.cfg.client_id)
            .header("ClientSecret", self.cfg.client_secret.expose())
            .json(&json!({"taxcode": self.cfg.tax_code, "username": self.cfg.username, "password": self.cfg.password.expose()}))
            .send()
            .await
            .map_err(|e| MisaError::Http(e.without_url().to_string()))?
            .json()
            .await
            .map_err(|e| MisaError::Http(e.to_string()))?;
        let ok = v["Success"]
            .as_bool()
            .or(v["success"].as_bool())
            .unwrap_or(false);
        let token = v["Data"]
            .as_str()
            .or(v["data"].as_str())
            .unwrap_or_default()
            .to_owned();
        if !ok || token.is_empty() {
            return Err(MisaError::Rejected(format!(
                "token: {}",
                v["ErrorCode"]
                    .as_str()
                    .or(v["errorCode"].as_str())
                    .unwrap_or("không rõ")
            )));
        }
        // Token hạn 14 ngày; dùng 13 ngày cho an toàn.
        *self.token.lock().expect("khóa token") = Some((
            token.clone(),
            OffsetDateTime::now_utc() + Duration::days(13),
        ));
        Ok(token)
    }

    async fn call(&self, path: &str, body: &Value) -> Result<Value, MisaError> {
        let url = self
            .cfg
            .endpoint
            .join(path)
            .map_err(|e| MisaError::Http(e.to_string()))?;
        for attempt in 0..2 {
            let token = self.token(attempt > 0).await?;
            let v: Value = self
                .http
                .post(url.clone())
                .header("ClientID", &self.cfg.client_id)
                .bearer_auth(&token)
                .json(body)
                .send()
                .await
                .map_err(|e| MisaError::Http(e.without_url().to_string()))?
                .json()
                .await
                .map_err(|e| MisaError::Http(e.to_string()))?;
            let code = v["errorCode"]
                .as_str()
                .or(v["ErrorCode"].as_str())
                .unwrap_or_default();
            if attempt == 0
                && matches!(
                    code,
                    "TokenExpiredCode" | "InvalidTokenCode" | "UnAuthorize"
                )
            {
                continue;
            }
            return Ok(v);
        }
        unreachable!("vòng lặp luôn trả về ở lần thứ hai")
    }

    #[allow(clippy::too_many_arguments)]
    fn invoice_data(
        &self,
        ref_id: &str,
        buyer: &Buyer,
        line: &str,
        amount: i64,
        date: &str,
        original: Option<&Original>,
        note: &str,
    ) -> Value {
        let vat = &self.cfg.vat_rate;
        // Giá đã gồm thuế: tách tiền trước thuế theo thuế suất (KCT, 0% không có thuế).
        let rate: i64 = vat.trim_end_matches('%').parse().unwrap_or(0);
        let before = if rate > 0 {
            (amount * 100 + (100 + rate) / 2) / (100 + rate)
        } else {
            amount
        };
        let tax = amount - before;
        let mut d = json!({
            "RefID": ref_id,
            "InvSeries": self.cfg.inv_series,
            "InvDate": date,
            "CurrencyCode": "VND",
            "ExchangeRate": 1,
            "PaymentMethodName": "TM/CK",
            "IsInvoiceSummary": false,
            "IsSendEmail": true,
            "ReceiverName": buyer.name,
            "ReceiverEmail": buyer.email,
            "BuyerEmail": buyer.email,
        });
        for (k, v) in [
            ("TotalSaleAmountOC", before),
            ("TotalSaleAmount", before),
            ("TotalDiscountAmountOC", 0),
            ("TotalDiscountAmount", 0),
            ("TotalAmountWithoutVATOC", before),
            ("TotalAmountWithoutVAT", before),
            ("TotalVATAmountOC", tax),
            ("TotalVATAmount", tax),
            ("TotalAmountOC", amount),
            ("TotalAmount", amount),
        ] {
            d[k] = json!(v);
        }
        d["TotalAmountInWords"] = json!(num_words(amount));
        d["OriginalInvoiceDetail"] = json!([{
            "ItemType": 1, "SortOrder": 1, "LineNumber": 1,
            "ItemCode": "XAYAPP", "ItemName": line, "UnitName": "Gói",
            "Quantity": 1, "UnitPrice": before,
            "AmountOC": before, "Amount": before,
            "DiscountRate": 0, "DiscountAmountOC": 0, "DiscountAmount": 0,
            "AmountWithoutVATOC": before, "AmountWithoutVAT": before,
            "VATRateName": vat, "VATAmountOC": tax, "VATAmount": tax,
        }]);
        d["TaxRateInfo"] =
            json!([{"VATRateName": vat, "AmountWithoutVATOC": before, "VATAmountOC": tax}]);
        if buyer.company {
            d["BuyerLegalName"] = json!(buyer.name);
            d["BuyerTaxCode"] = json!(buyer.tax_code);
            d["BuyerAddress"] = json!(buyer.address);
        } else {
            d["BuyerFullName"] = json!(buyer.name);
            if !buyer.address.is_empty() {
                d["BuyerAddress"] = json!(buyer.address);
            }
        }
        if let Some(o) = original {
            let (template, series) = o.series.split_at(1.min(o.series.len()));
            d["ReferenceType"] = json!(2);
            d["OrgInvoiceType"] = json!(1);
            d["OrgInvTemplateNo"] = json!(template);
            d["OrgInvSeries"] = json!(series);
            d["OrgInvNo"] = json!(o.number);
            d["OrgInvDate"] = json!(o.date);
            d["InvoiceNote"] = json!(note);
        }
        d
    }

    /// Phát hành một hóa đơn (gốc, hoặc điều chỉnh giảm khi có `original`) bằng chữ ký HSM.
    #[allow(clippy::too_many_arguments)]
    pub async fn publish(
        &self,
        ref_id: &str,
        buyer: &Buyer,
        line: &str,
        amount: i64,
        now: OffsetDateTime,
        original: Option<&Original>,
        note: &str,
    ) -> Result<Published, MisaError> {
        let date = now.to_offset(time::macros::offset!(+7)).date();
        let date = format!(
            "{:04}-{:02}-{:02}",
            date.year(),
            u8::from(date.month()),
            date.day()
        );
        let body = json!({"SignType": 2, "InvoiceData": [self.invoice_data(ref_id, buyer, line, amount, &date, original, note)], "PublishInvoiceData": null});
        let v = self.call("invoice/publishing", &body).await?;
        let ok = v["success"]
            .as_bool()
            .or(v["Success"].as_bool())
            .unwrap_or(false);
        let raw = v["publishInvoiceResult"].as_str().unwrap_or("[]");
        let items: Vec<Value> = serde_json::from_str(raw).unwrap_or_default();
        let item = items.into_iter().find(|i| i["RefID"] == ref_id);
        match item {
            Some(i) if ok && i["ErrorCode"].as_str().is_none_or(str::is_empty) => Ok(Published {
                transaction_id: i["TransactionID"].as_str().unwrap_or_default().to_owned(),
                series: i["InvSeries"]
                    .as_str()
                    .unwrap_or(&self.cfg.inv_series)
                    .to_owned(),
                number: i["InvNo"].as_str().unwrap_or_default().to_owned(),
                date: i["InvDate"]
                    .as_str()
                    .map(|d| d.chars().take(10).collect())
                    .unwrap_or(date),
            }),
            Some(i) => Err(MisaError::Rejected(
                i["ErrorCode"].as_str().unwrap_or("không rõ").to_owned(),
            )),
            None => Err(MisaError::Rejected(
                v["errorCode"]
                    .as_str()
                    .or(v["descriptionErrorCode"].as_str())
                    .unwrap_or("không có kết quả phát hành")
                    .to_owned(),
            )),
        }
    }

    /// Tải PDF của hóa đơn theo mã tra cứu.
    pub async fn download_pdf(&self, transaction_id: &str) -> Result<Vec<u8>, MisaError> {
        let v = self
            .call(
                "invoice/Download?downloadDataType=Pdf",
                &json!([transaction_id]),
            )
            .await?;
        let data = match &v["data"] {
            Value::String(s) => s.clone(),
            Value::Array(a) => a
                .first()
                .and_then(|x| {
                    x["Data"]
                        .as_str()
                        .or(x["FileData"].as_str())
                        .or(x["data"].as_str())
                })
                .unwrap_or_default()
                .to_owned(),
            _ => String::new(),
        };
        use base64::Engine;
        base64::engine::general_purpose::STANDARD
            .decode(data.trim())
            .map_err(|e| MisaError::Rejected(format!("PDF không hợp lệ: {e}")))
    }
}

/// Máy chủ MISA meInvoice giả trên loopback cho kiểm thử.
pub mod testing {
    use std::sync::{Arc, Mutex};

    use axum::{extract::State, http::HeaderMap, routing::post, Json, Router};
    use serde_json::{json, Value};

    #[derive(Clone)]
    pub struct FakeMisa {
        pub client_id: String,
        pub client_secret: String,
        pub addr: std::net::SocketAddr,
        /// Hóa đơn đã phát hành (thân `InvoiceData` nhận được).
        pub published: Arc<Mutex<Vec<Value>>>,
        /// Số hóa đơn đã cấp, theo thứ tự (bắt đầu từ một số ngẫu nhiên mỗi lần chạy).
        pub numbers: Arc<Mutex<Vec<String>>>,
        pub tokens_issued: Arc<Mutex<usize>>,
        first: u64,
    }

    const TOKEN: &str = "fake-misa-token";

    async fn token(
        State(f): State<FakeMisa>,
        headers: HeaderMap,
        Json(b): Json<Value>,
    ) -> Json<Value> {
        let h = |k: &str| {
            headers
                .get(k)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        if h("ClientID") != f.client_id
            || h("ClientSecret") != f.client_secret
            || b["taxcode"].as_str().is_none_or(str::is_empty)
        {
            return Json(json!({"Success": false, "ErrorCode": "InvalidAppID"}));
        }
        *f.tokens_issued.lock().unwrap() += 1;
        Json(json!({"Success": true, "Data": TOKEN}))
    }

    fn authorized(f: &FakeMisa, headers: &HeaderMap) -> bool {
        let h = |k: &str| {
            headers
                .get(k)
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_owned()
        };
        h("ClientID") == f.client_id && h("Authorization") == format!("Bearer {TOKEN}")
    }

    async fn publish(
        State(f): State<FakeMisa>,
        headers: HeaderMap,
        Json(b): Json<Value>,
    ) -> Json<Value> {
        if !authorized(&f, &headers) {
            return Json(json!({"success": false, "errorCode": "InvalidTokenCode"}));
        }
        let mut published = f.published.lock().unwrap();
        let mut results = Vec::new();
        for inv in b["InvoiceData"].as_array().cloned().unwrap_or_default() {
            let company = inv["BuyerTaxCode"].is_string();
            if company
                && (inv["BuyerLegalName"].as_str().is_none_or(str::is_empty)
                    || inv["BuyerAddress"].as_str().is_none_or(str::is_empty))
            {
                results.push(json!({"RefID": inv["RefID"], "ErrorCode": "InvalidBuyerInfo"}));
                continue;
            }
            published.push(inv.clone());
            let no = format!("{:08}", f.first + published.len() as u64);
            f.numbers.lock().unwrap().push(no.clone());
            results.push(json!({"RefID": inv["RefID"], "TransactionID": format!("TRA{no}"), "InvTemplateNo": "1", "InvSeries": inv["InvSeries"], "InvNo": no, "InvDate": inv["InvDate"], "ErrorCode": null}));
        }
        Json(
            json!({"success": true, "publishInvoiceResult": serde_json::to_string(&results).unwrap()}),
        )
    }

    async fn download(
        State(f): State<FakeMisa>,
        headers: HeaderMap,
        Json(b): Json<Value>,
    ) -> Json<Value> {
        if !authorized(&f, &headers) {
            return Json(json!({"success": false, "errorCode": "InvalidTokenCode"}));
        }
        use base64::Engine;
        let id = b[0].as_str().unwrap_or_default();
        let pdf = format!("%PDF-1.4 hoa don {id}");
        Json(
            json!({"success": true, "data": base64::engine::general_purpose::STANDARD.encode(pdf)}),
        )
    }

    impl FakeMisa {
        pub async fn serve(client_id: &str, client_secret: &str) -> std::io::Result<FakeMisa> {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
            let fake = FakeMisa {
                client_id: client_id.to_owned(),
                client_secret: client_secret.to_owned(),
                addr: listener.local_addr()?,
                published: Arc::default(),
                numbers: Arc::default(),
                tokens_issued: Arc::default(),
                first: std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .unwrap_or_default()
                    .as_micros() as u64
                    % 90_000_000,
            };
            let app = Router::new()
                .route("/apis/itg/meinvoice/invoice/token", post(token))
                .route("/apis/itg/meinvoice/invoice/publishing", post(publish))
                .route("/apis/itg/meinvoice/invoice/Download", post(download))
                .with_state(fake.clone());
            tokio::spawn(async move {
                let _ = axum::serve(listener, app).await;
            });
            Ok(fake)
        }

        pub fn endpoint(&self) -> String {
            format!("http://{}/apis/itg/meinvoice/", self.addr)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use feco_config::Secret;

    fn misa(endpoint: &str) -> Misa {
        Misa::new(&MisaConfig {
            client_id: "cid".into(),
            client_secret: Secret::new("csecret"),
            tax_code: "0101234567".into(),
            username: "ketoan@f-eco.test".into(),
            password: Secret::new("mk"),
            inv_series: "1C26TFE".into(),
            vat_rate: "10%".into(),
            endpoint: url::Url::parse(endpoint).unwrap(),
        })
    }

    #[tokio::test]
    async fn publishes_original_and_adjustment_invoices() {
        let fake = testing::FakeMisa::serve("cid", "csecret").await.unwrap();
        let m = misa(&fake.endpoint());
        let company = Buyer {
            company: true,
            name: "Công ty TNHH Kho Vui".into(),
            tax_code: Some("0101234567".into()),
            address: "Hà Nội".into(),
            email: "kt@khovui.vn".into(),
        };
        let now = time::macros::datetime!(2026-11-12 03:00 UTC);
        let p = m
            .publish(
                "tx-1",
                &company,
                "Dịch vụ xây app, mốc 1",
                11_000_000,
                now,
                None,
                "",
            )
            .await
            .unwrap();
        assert_eq!(
            (p.series.as_str(), p.number.as_str(), p.date.as_str()),
            (
                "1C26TFE",
                fake.numbers.lock().unwrap()[0].as_str(),
                "2026-11-12"
            )
        );
        let inv = fake.published.lock().unwrap()[0].clone();
        assert_eq!(inv["BuyerTaxCode"], "0101234567");
        assert_eq!(inv["BuyerLegalName"], "Công ty TNHH Kho Vui");
        // Giá đã gồm thuế 10%: 10.000.000 trước thuế + 1.000.000 thuế.
        assert_eq!(
            (
                inv["TotalAmountWithoutVAT"].as_i64(),
                inv["TotalVATAmount"].as_i64(),
                inv["TotalAmount"].as_i64()
            ),
            (Some(10_000_000), Some(1_000_000), Some(11_000_000))
        );
        let original = Original {
            series: p.series.clone(),
            number: p.number.clone(),
            date: p.date.clone(),
        };
        let adj = m
            .publish(
                "rf-1",
                &company,
                "Điều chỉnh giảm do hoàn tiền",
                4_400_000,
                now,
                Some(&original),
                "Hoàn tiền tranh chấp",
            )
            .await
            .unwrap();
        assert_eq!(adj.number, fake.numbers.lock().unwrap()[1]);
        let inv = fake.published.lock().unwrap()[1].clone();
        assert_eq!(
            (
                inv["ReferenceType"].as_i64(),
                inv["OrgInvTemplateNo"].as_str(),
                inv["OrgInvSeries"].as_str(),
                inv["OrgInvNo"].as_str()
            ),
            (Some(2), Some("1"), Some("C26TFE"), Some(p.number.as_str()))
        );
        // Cá nhân: chỉ cần họ tên.
        let person = Buyer {
            company: false,
            name: "Nguyễn Văn An".into(),
            tax_code: None,
            address: String::new(),
            email: "an@example.com".into(),
        };
        m.publish("tx-2", &person, "Mốc 2", 5_000_000, now, None, "")
            .await
            .unwrap();
        assert_eq!(
            fake.published.lock().unwrap()[2]["BuyerFullName"],
            "Nguyễn Văn An"
        );
        assert_eq!(
            *fake.tokens_issued.lock().unwrap(),
            1,
            "token được dùng lại"
        );
        let pdf = m.download_pdf(&p.transaction_id).await.unwrap();
        assert!(pdf.starts_with(b"%PDF"));
        // Sai ClientSecret: không lấy được token.
        let mut bad = misa(&fake.endpoint());
        bad.cfg.client_secret = Secret::new("sai");
        assert!(bad
            .publish("tx-3", &person, "x", 1000, now, None, "")
            .await
            .is_err());
    }
}
