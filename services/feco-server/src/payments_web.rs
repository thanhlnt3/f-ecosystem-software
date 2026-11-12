//! Trang thanh toán (spec payments):
//! - `GET/POST /payments/{yêu cầu}/pay`: số tiền, nội dung mốc, chọn phương thức (QR chuyển khoản,
//!   thẻ ATM, ví điện tử, thẻ quốc tế) rồi chuyển sang trang của ZaloPay để trả tiền;
//! - `GET /payments/return`: trang ZaloPay chuyển về. Tham số trên URL không được tin: trang chỉ
//!   kích hoạt một lần truy vấn chủ động tới cổng;
//! - `POST /v1/payments/zalopay/callback`: callback đã ký của ZaloPay;
//! - `GET /payments`: lịch sử giao dịch, hoàn tiền, hóa đơn, và thông tin xuất hóa đơn;
//! - `GET /payments/invoices/{id}`: tải lại hóa đơn.

use std::sync::Arc;

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    connected::current_subject,
    csrf, gateways,
    i18n::Lang,
    orders::{self, OrderError},
    payments::{self, BillingProfile},
    views::Msg,
    AppState,
};

fn html(
    t: impl Template,
    lang: Lang,
    status: StatusCode,
    set_cookie: Option<HeaderValue>,
) -> Response {
    let mut res = match t.render() {
        Ok(h) => (status, Html(h)).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "lỗi dựng trang");
            (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
        }
    };
    if let Some(c) = set_cookie {
        res.headers_mut().append(header::SET_COOKIE, c);
    }
    res
}

fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

fn explain(lang: Lang, e: OrderError) -> (StatusCode, Msg) {
    match e {
        OrderError::Problem { status, vi, en, .. } => {
            (status, Msg::error(if lang == Lang::Vi { vi } else { en }))
        }
        OrderError::Db(e) => {
            tracing::error!(error = %e, "lỗi cơ sở dữ liệu ở trang thanh toán");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Msg::error(lang.t("error.unavailable")),
            )
        }
    }
}

fn when(t: OffsetDateTime) -> String {
    t.format(&time::macros::format_description!(
        "[day]/[month]/[year] [hour]:[minute]"
    ))
    .unwrap_or_default()
}

// ---------- Trang thanh toán một khoản ----------

pub struct MethodView {
    /// `cổng:phương thức`, ví dụ `vnpay:vietqr`.
    pub code: String,
    pub label: String,
}

#[derive(Template)]
#[template(path = "pay.html")]
struct PayPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    request: String,
    order_id: String,
    order_code: String,
    order_title: String,
    what: String,
    amount: String,
    payable: bool,
    paid: bool,
    gateway_ready: bool,
    methods: Vec<MethodView>,
    minutes: i64,
}

async fn render_pay(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    request: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let vi = lang == Lang::Vi;
    let c = match payments::charge_of(state, subject, request).await {
        Ok(c) => c,
        Err(e) => {
            let (status, m) = explain(lang, e);
            return (status, m.text).into_response();
        }
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    html(
        PayPage {
            i18n: lang,
            messages,
            csrf: token,
            request: request.to_string(),
            order_id: c.order_id.to_string(),
            what: match c.milestone {
                Some(p) => format!(
                    "{} {p}: {}",
                    if vi { "Mốc" } else { "Milestone" },
                    c.milestone_title
                ),
                None => c.reason.clone(),
            },
            order_code: c.order_code,
            order_title: c.order_title,
            amount: orders::vnd(c.amount_vnd),
            payable: c.status == "pending",
            paid: c.status == "succeeded",
            gateway_ready: gateways::any(state),
            methods: gateways::choices(state)
                .into_iter()
                .map(|(p, m)| MethodView {
                    code: format!("{p}:{m}"),
                    label: format!(
                        "{} ({})",
                        payments::method_label(m, vi),
                        gateways::provider_label(p)
                    ),
                })
                .collect(),
            minutes: payments::LINK_MINUTES,
        },
        lang,
        status,
        set,
    )
}

/// GET /payments/{request}/pay
pub async fn pay_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(request): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let subject =
        match current_subject(&state, lang, &headers, &format!("/payments/{request}/pay")).await {
            Ok(s) => s,
            Err(r) => return *r,
        };
    render_pay(
        &state,
        lang,
        &headers,
        subject,
        request,
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /payments/{request}/pay (feco_csrf, method): tạo hoặc dùng lại giao dịch, chuyển sang ZaloPay.
pub async fn pay(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(request): Path<Uuid>,
    client: crate::kratos::ClientInfo,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let get = |k: &str| {
        form.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.as_str())
            .unwrap_or_default()
    };
    if !csrf::verify(&headers, Some(get(csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject =
        match current_subject(&state, lang, &headers, &format!("/payments/{request}/pay")).await {
            Ok(s) => s,
            Err(r) => return *r,
        };
    match payments::start(
        &state,
        subject,
        request,
        get("method"),
        client.ip.as_deref().unwrap_or("127.0.0.1"),
        OffsetDateTime::now_utc(),
    )
    .await
    {
        Ok(link) => see_other(&link.order_url),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_pay(&state, lang, &headers, subject, request, vec![msg], status).await
        }
    }
}

// ---------- Trang chuyển về và callback ----------

#[derive(Debug, Deserialize)]
pub struct ReturnQuery {
    /// ZaloPay.
    pub apptransid: Option<String>,
    /// VNPAY.
    #[serde(rename = "vnp_TxnRef")]
    pub vnp_txn_ref: Option<String>,
    /// MoMo.
    #[serde(rename = "orderId")]
    pub order_id: Option<String>,
}

#[derive(Template)]
#[template(path = "payment_return.html")]
struct ReturnPage {
    i18n: Lang,
    status: &'static str,
    label: String,
    order_id: String,
}

/// GET /payments/return: không tin `status` hay `checksum` trên URL; truy vấn cổng rồi hiện kết quả.
pub async fn return_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Query(q): Query<ReturnQuery>,
) -> Response {
    let vi = lang == Lang::Vi;
    let Some(gateway_ref) = q
        .apptransid
        .or(q.vnp_txn_ref)
        .or(q.order_id)
        .filter(|r| !r.is_empty() && r.len() <= 64)
    else {
        return (StatusCode::BAD_REQUEST, lang.t("pay.unknown")).into_response();
    };
    let now = OffsetDateTime::now_utc();
    let t = match payments::by_ref(&state, &gateway_ref).await {
        Ok(t) => t,
        Err(_) => return (StatusCode::NOT_FOUND, lang.t("pay.unknown")).into_response(),
    };
    if let Err(e) = payments::confirm(&state, &gateway_ref, "redirect", now).await {
        tracing::warn!(error = %e, gateway_ref, "không truy vấn được giao dịch khi khách quay về");
    }
    let t = payments::transaction(&state, t.id).await.unwrap_or(t);
    let order_id: Option<Uuid> =
        sqlx::query_scalar("SELECT order_id FROM payment_requests WHERE id = $1")
            .bind(t.request_id)
            .fetch_optional(&state.db)
            .await
            .ok()
            .flatten();
    html(
        ReturnPage {
            i18n: lang,
            status: t.status.as_str(),
            label: t.label(vi).to_owned(),
            order_id: order_id.map(|o| o.to_string()).unwrap_or_default(),
        },
        lang,
        StatusCode::OK,
        None,
    )
}

/// GET /v1/payments/vnpay/ipn (VNPAY gọi, tham số `vnp_*` có chữ ký).
pub async fn vnpay_ipn(
    State(state): State<Arc<AppState>>,
    Query(params): Query<std::collections::BTreeMap<String, String>>,
) -> Response {
    Json(payments::handle_vnpay_ipn(&state, &params, OffsetDateTime::now_utc()).await)
        .into_response()
}

/// POST /v1/payments/momo/ipn: 204 khi đã nhận, 400 khi sai chữ ký.
pub async fn momo_ipn(State(state): State<Arc<AppState>>, Json(body): Json<Value>) -> Response {
    if payments::handle_momo_ipn(&state, &body, OffsetDateTime::now_utc()).await {
        StatusCode::NO_CONTENT.into_response()
    } else {
        StatusCode::BAD_REQUEST.into_response()
    }
}

/// POST /v1/payments/zalopay/callback
pub async fn zalopay_callback(
    State(state): State<Arc<AppState>>,
    Json(body): Json<Value>,
) -> Response {
    Json(payments::handle_callback(&state, &body, OffsetDateTime::now_utc()).await).into_response()
}

// ---------- Lịch sử thanh toán và hóa đơn ----------

pub struct InvoiceView {
    pub id: String,
    pub number: String,
    pub kind: String,
    pub amount: String,
}

pub struct RefundView {
    pub amount: String,
    pub status: String,
    pub reason: String,
}

pub struct HistoryView {
    pub order_id: String,
    pub order_code: String,
    pub what: String,
    pub method: String,
    pub amount: String,
    pub status: String,
    pub status_code: &'static str,
    pub reference: String,
    pub at: String,
    pub refunds: Vec<RefundView>,
    pub invoices: Vec<InvoiceView>,
}

#[derive(Template)]
#[template(path = "payments.html")]
struct HistoryPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    items: Vec<HistoryView>,
    company: bool,
    name: String,
    tax_code: String,
    address: String,
    email: String,
}

async fn render_history(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
    form: Option<BillingProfile>,
) -> Response {
    let vi = lang == Lang::Vi;
    let result: Result<(Vec<payments::HistoryItem>, Option<BillingProfile>), OrderError> = async {
        Ok((
            payments::history(state, subject).await?,
            payments::billing_profile(state, subject).await?,
        ))
    }
    .await;
    let (items, profile) = match result {
        Ok(x) => x,
        Err(e) => {
            let (status, m) = explain(lang, e);
            return (status, m.text).into_response();
        }
    };
    let email = match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(i)) => i["traits"]["email"].as_str().unwrap_or_default().to_owned(),
        _ => String::new(),
    };
    let p = form.or(profile).unwrap_or(BillingProfile {
        kind: "individual".into(),
        email: email.clone(),
        ..Default::default()
    });
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    html(
        HistoryPage {
            i18n: lang,
            messages,
            csrf: token,
            items: items
                .into_iter()
                .map(|h| HistoryView {
                    order_id: h.order_id.to_string(),
                    order_code: h.order_code,
                    what: h.what,
                    method: payments::method_label(&h.transaction.method, vi).to_owned(),
                    amount: orders::vnd(h.transaction.amount_vnd),
                    status: h.transaction.label(vi).to_owned(),
                    status_code: h.transaction.status.as_str(),
                    reference: h.transaction.gateway_ref.clone(),
                    at: when(h.transaction.created_at),
                    refunds: h
                        .refunds
                        .iter()
                        .map(|r| RefundView {
                            amount: orders::vnd(r.amount_vnd),
                            status: r.label(vi).to_owned(),
                            reason: r.reason.clone(),
                        })
                        .collect(),
                    invoices: h
                        .invoices
                        .iter()
                        .map(|i| InvoiceView {
                            id: i.id.to_string(),
                            number: i.number.clone(),
                            kind: if i.kind == "adjustment" {
                                lang.t("pay.invoice_adjustment").to_owned()
                            } else {
                                lang.t("pay.invoice").to_owned()
                            },
                            amount: orders::vnd(i.amount_vnd),
                        })
                        .collect(),
                })
                .collect(),
            company: p.kind == "company",
            name: p.name,
            tax_code: p.tax_code.unwrap_or_default(),
            address: p.address,
            email: if p.email.is_empty() { email } else { p.email },
        },
        lang,
        status,
        set,
    )
}

/// GET /payments
pub async fn history(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<crate::orders_web::OkQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/payments").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let messages = if q.ok.as_deref() == Some("billing") {
        vec![Msg::success(lang.t("pay.billing_saved"))]
    } else {
        Vec::new()
    };
    render_history(
        &state,
        lang,
        &headers,
        subject,
        messages,
        StatusCode::OK,
        None,
    )
    .await
}

/// POST /payments/billing: thông tin xuất hóa đơn (cá nhân, hoặc doanh nghiệp có mã số thuế).
pub async fn save_billing(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    let get = |k: &str| {
        form.iter()
            .find(|(n, _)| n == k)
            .map(|(_, v)| v.clone())
            .unwrap_or_default()
    };
    if !csrf::verify(&headers, Some(&get(csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/payments").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let p = BillingProfile {
        kind: get("kind"),
        name: get("name"),
        tax_code: Some(get("tax_code")).filter(|t| !t.trim().is_empty()),
        address: get("address"),
        email: get("email"),
    };
    let email_ok = p.email.trim().contains('@');
    let result = if email_ok {
        payments::save_billing_profile(&state, subject, &p).await
    } else {
        Err(orders::problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            "Email nhận hóa đơn không hợp lệ.",
            "The invoice email is not valid.",
        ))
    };
    match result {
        Ok(()) => see_other("/payments?ok=billing"),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_history(&state, lang, &headers, subject, vec![msg], status, Some(p)).await
        }
    }
}

/// GET /payments/invoices/{id}: tải lại hóa đơn (chỉ chủ hóa đơn).
pub async fn invoice(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/payments").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    match payments::invoice_document(&state, subject, id).await {
        Ok(file) => {
            let mut res = file.bytes.into_response();
            let h = res.headers_mut();
            if let Ok(v) = HeaderValue::from_str(&file.content_type) {
                h.insert(header::CONTENT_TYPE, v);
            }
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{}\"", file.name))
            {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            res
        }
        Err(e) => {
            let (status, m) = explain(lang, e);
            (status, m.text).into_response()
        }
    }
}
