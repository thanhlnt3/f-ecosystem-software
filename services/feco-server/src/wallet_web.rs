//! Trang ví F-Coin (thay đổi OpenSpec fcoin-wallet-and-marketplace):
//! - `GET /wallet`: số dư, tỷ giá, nạp, rút, lệnh nạp, lệnh rút, lịch sử giao dịch (lọc, phân trang);
//! - `POST /wallet/topup`, `POST /wallet/withdraw`, `POST /wallet/withdrawals/{id}/cancel`;
//! - `GET /wallet/stats`: thống kê chi tiêu và thu nhập;
//! - `POST /store/{id}/buy`: mua app trên web;
//! - `GET /v1/wallet`, `POST /v1/wallet/purchases`: API cho F-Launcher;
//! - `GET/POST /admin/wallet`: trang Ví F-Coin của bảng quản trị.

use std::{collections::BTreeMap, sync::Arc};

use askama::Template;
use axum::{
    body::Bytes,
    extract::{Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
    Json,
};
use serde::Deserialize;
use serde_json::json;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    admin::{self, Role},
    api::api_error,
    connected::{current_subject, format_time},
    csrf, gateways,
    i18n::Lang,
    orders::{vnd, OrderError},
    views::Msg,
    wallet::{self, coin, signed_coin, Kind, Payout},
    AppState,
};

pub(crate) fn render(
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

pub(crate) fn see_other(to: &str) -> Response {
    let mut res = StatusCode::SEE_OTHER.into_response();
    if let Ok(v) = HeaderValue::from_str(to) {
        res.headers_mut().insert(header::LOCATION, v);
    }
    res
}

/// Thông báo lỗi nghiệp vụ theo ngôn ngữ (tiếng Việt, còn lại tiếng Anh).
pub(crate) fn explain(lang: Lang, e: OrderError) -> (StatusCode, Msg) {
    match e {
        OrderError::Problem { status, vi, en, .. } => {
            (status, Msg::error(if lang == Lang::Vi { vi } else { en }))
        }
        OrderError::Db(e) => {
            tracing::error!(error = %e, "lỗi cơ sở dữ liệu");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Msg::error(lang.t("error.unavailable")),
            )
        }
    }
}

pub(crate) fn form(body: &Bytes) -> BTreeMap<String, String> {
    form_urlencoded::parse(body).into_owned().collect()
}

pub(crate) fn get<'a>(f: &'a BTreeMap<String, String>, k: &str) -> &'a str {
    f.get(k).map(String::as_str).unwrap_or_default()
}

/// Email trong hồ sơ Kratos của người dùng (rỗng nếu không đọc được).
pub(crate) async fn identity_email(state: &AppState, subject: Uuid) -> String {
    match state.kratos.get_identity(&subject.to_string()).await {
        Ok(Some(i)) => i["traits"]["email"].as_str().unwrap_or_default().to_owned(),
        Ok(None) => String::new(),
        Err(e) => {
            tracing::warn!(error = %e, "không đọc được danh tính");
            String::new()
        }
    }
}

fn rate_label(lang: Lang, per_coin: i64) -> String {
    format!("1 F-Coin = {}", vnd(per_coin)).replace("₫", if lang == Lang::Vi { "₫" } else { "VND" })
}

// ---------- Trang ví ----------

pub struct SourceView {
    pub id: String,
    pub label: String,
}

pub struct TopupView {
    pub code: String,
    pub coins: String,
    pub amount: String,
    pub status: String,
    pub status_label: &'static str,
    pub pay: String,
    pub at: String,
}

pub struct WithdrawalView {
    pub id: String,
    pub code: String,
    pub coins: String,
    pub amount: String,
    pub method: String,
    pub status: String,
    pub status_label: &'static str,
    pub cancellable: bool,
    pub note: String,
    pub at: String,
}

pub struct LineView {
    pub code: String,
    pub at: String,
    pub label: &'static str,
    pub amount: String,
    pub positive: bool,
    pub balance: String,
    pub memo: String,
}

pub struct KindOption {
    pub code: &'static str,
    pub label: &'static str,
    pub selected: bool,
}

#[derive(Template)]
#[template(path = "wallet.html")]
struct WalletPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    balance: String,
    rate: String,
    rate_vnd: i64,
    frozen: Option<String>,
    gateway_ready: bool,
    withdrawals_enabled: bool,
    min_topup: i64,
    max_topup: i64,
    min_withdrawal: i64,
    sources: Vec<SourceView>,
    topups: Vec<TopupView>,
    withdrawals: Vec<WithdrawalView>,
    lines: Vec<LineView>,
    kinds: Vec<KindOption>,
    kind: String,
    prev: Option<i64>,
    next: Option<i64>,
}

fn topup_status(s: &str) -> &'static str {
    match s {
        "succeeded" => "wallet.topup.succeeded",
        "cancelled" => "wallet.topup.cancelled",
        _ => "wallet.topup.pending",
    }
}

pub fn withdrawal_status(s: &str) -> &'static str {
    match s {
        "approved" => "wallet.wd.approved",
        "processing" => "wallet.wd.processing",
        "paid" => "wallet.wd.paid",
        "rejected" => "wallet.wd.rejected",
        "cancelled" => "wallet.wd.cancelled",
        "failed" => "wallet.wd.failed",
        _ => "wallet.wd.pending",
    }
}

fn kind_label(k: Kind) -> &'static str {
    match k {
        Kind::Topup => "wallet.kind.topup",
        Kind::WithdrawalHold => "wallet.kind.withdrawal",
        Kind::WithdrawalRelease => "wallet.kind.withdrawal_release",
        Kind::WithdrawalPaid => "wallet.kind.withdrawal_paid",
        Kind::Purchase => "wallet.kind.purchases",
        Kind::EscrowFund => "wallet.kind.escrow_fund",
        Kind::EscrowRelease => "wallet.kind.escrow_release",
        Kind::EscrowRefund => "wallet.kind.escrow_refund",
    }
}

#[derive(Deserialize, Default)]
pub struct WalletQuery {
    pub kind: Option<String>,
    pub page: Option<i64>,
}

const PER_PAGE: i64 = 20;

async fn render_wallet(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    q: &WalletQuery,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let db = &state.db;
    let kind = q.kind.as_deref().and_then(Kind::parse);
    let page = q.page.unwrap_or(1).max(1);
    let (summary, sources, topups, withdrawals, history) = tokio::join!(
        wallet::summary(db, subject),
        wallet::refundable_sources(db, subject),
        wallet::topups_of(db, subject, 10),
        wallet::withdrawals_of(db, subject, 10),
        wallet::history(db, subject, kind, page, PER_PAGE),
    );
    let summary = match summary {
        Ok(s) => s,
        Err(e) => {
            tracing::error!(error = %e, "không đọc được ví");
            return (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response();
        }
    };
    let (lines, more) = history.unwrap_or_default();
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        WalletPage {
            i18n: lang,
            messages,
            csrf: token,
            balance: coin(summary.balance, lang),
            rate: rate_label(lang, summary.rate),
            rate_vnd: summary.rate,
            frozen: summary.frozen,
            gateway_ready: gateways::any(state),
            withdrawals_enabled: state.withdrawals_enabled,
            min_topup: wallet::MIN_TOPUP,
            max_topup: wallet::MAX_TOPUP,
            min_withdrawal: wallet::MIN_WITHDRAWAL,
            sources: sources
                .unwrap_or_default()
                .into_iter()
                .map(|s| SourceView {
                    id: s.transaction.to_string(),
                    label: format!(
                        "{} · {} · {} {}",
                        s.topup_code,
                        gateways::provider_label(&s.provider),
                        lang.t("wallet.refundable"),
                        vnd(s.refundable_vnd)
                    ),
                })
                .collect(),
            topups: topups
                .unwrap_or_default()
                .into_iter()
                .map(|t| TopupView {
                    pay: match (t.status.as_str(), t.request) {
                        ("pending", Some(r)) => format!("/payments/{r}/pay"),
                        _ => String::new(),
                    },
                    code: t.code,
                    coins: coin(t.coins, lang),
                    amount: vnd(t.amount_vnd),
                    status_label: topup_status(&t.status),
                    status: t.status,
                    at: format_time(t.created_at),
                })
                .collect(),
            withdrawals: withdrawals
                .unwrap_or_default()
                .into_iter()
                .map(|w| WithdrawalView {
                    id: w.id.to_string(),
                    code: w.code,
                    coins: coin(w.coins, lang),
                    amount: vnd(w.amount_vnd),
                    method: match (w.method.as_str(), &w.bank_name, &w.account_last4) {
                        ("bank", Some(b), Some(l)) => format!("{b} ···{l}"),
                        _ => lang.t("wallet.method.source").to_owned(),
                    },
                    cancellable: w.status == "pending",
                    status_label: withdrawal_status(&w.status),
                    status: w.status,
                    note: w.decision_note.unwrap_or_default(),
                    at: format_time(w.created_at),
                })
                .collect(),
            lines: lines
                .into_iter()
                .map(|l| LineView {
                    code: l.code,
                    at: format_time(l.at),
                    label: wallet::line_key(l.kind, l.amount),
                    amount: signed_coin(l.amount, lang),
                    positive: l.amount > 0,
                    balance: coin(l.balance_after, lang),
                    memo: l.memo,
                })
                .collect(),
            kinds: Kind::ALL
                .into_iter()
                .filter(|k| *k != Kind::WithdrawalPaid)
                .map(|k| KindOption {
                    code: k.as_str(),
                    label: kind_label(k),
                    selected: Some(k) == kind,
                })
                .collect(),
            kind: kind.map(|k| k.as_str().to_owned()).unwrap_or_default(),
            prev: (page > 1).then_some(page - 1),
            next: more.then_some(page + 1),
        },
        lang,
        status,
        set,
    )
}

/// GET /wallet
pub async fn page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<WalletQuery>,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/wallet").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_wallet(
        &state,
        lang,
        &headers,
        subject,
        &q,
        Vec::new(),
        StatusCode::OK,
    )
    .await
}

/// POST /wallet/topup: tạo lệnh nạp rồi chuyển tới trang thanh toán.
pub async fn topup(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/wallet").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let coins: i64 = get(&f, "coins").trim().parse().unwrap_or(0);
    let email = identity_email(&state, subject).await;
    match wallet::create_topup(&state, subject, &email, coins).await {
        Ok(t) => see_other(&format!("/payments/{}/pay", t.request)),
        Err(e) => {
            let (status, m) = explain(lang, e);
            let status = if status.is_server_error() {
                status
            } else {
                StatusCode::BAD_REQUEST
            };
            render_wallet(
                &state,
                lang,
                &headers,
                subject,
                &WalletQuery::default(),
                vec![m],
                status,
            )
            .await
        }
    }
}

/// POST /wallet/withdraw
pub async fn withdraw(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/wallet").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let coins: i64 = get(&f, "coins").trim().parse().unwrap_or(0);
    let payout = if get(&f, "method") == "source" {
        match get(&f, "source").parse::<Uuid>() {
            Ok(t) => Payout::Source(t),
            Err(_) => {
                return render_wallet(
                    &state,
                    lang,
                    &headers,
                    subject,
                    &WalletQuery::default(),
                    vec![Msg::error(lang.t("wallet.choose_source"))],
                    StatusCode::BAD_REQUEST,
                )
                .await
            }
        }
    } else {
        Payout::Bank {
            bank: get(&f, "bank").to_owned(),
            holder: get(&f, "holder").to_owned(),
            number: get(&f, "number").to_owned(),
        }
    };
    let email = identity_email(&state, subject).await;
    let (messages, status) =
        match wallet::create_withdrawal(&state, subject, &email, coins, payout, get(&f, "note"))
            .await
        {
            Ok(_) => (
                vec![Msg::success(lang.t("wallet.withdraw_created"))],
                StatusCode::OK,
            ),
            Err(e) => {
                let (s, m) = explain(lang, e);
                (vec![m], s)
            }
        };
    render_wallet(
        &state,
        lang,
        &headers,
        subject,
        &WalletQuery::default(),
        messages,
        status,
    )
    .await
}

/// POST /wallet/withdrawals/{id}/cancel
pub async fn cancel_withdrawal(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let subject = match current_subject(&state, lang, &headers, "/wallet").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let (messages, status) = match wallet::cancel_withdrawal(&state, subject, id).await {
        Ok(()) => (
            vec![Msg::success(lang.t("wallet.withdraw_cancelled"))],
            StatusCode::OK,
        ),
        Err(e) => {
            let (s, m) = explain(lang, e);
            (vec![m], s)
        }
    };
    render_wallet(
        &state,
        lang,
        &headers,
        subject,
        &WalletQuery::default(),
        messages,
        status,
    )
    .await
}

// ---------- Thống kê ----------

pub struct BarView {
    pub label: String,
    pub spend: String,
    pub income: String,
    pub spend_pct: i64,
    pub income_pct: i64,
}

pub struct CatView {
    pub label: &'static str,
    pub amount: String,
}

#[derive(Template)]
#[template(path = "wallet_stats.html")]
struct StatsPage {
    i18n: Lang,
    balance: String,
    month_spend: String,
    month_income: String,
    bars: Vec<BarView>,
    spend: Vec<CatView>,
    income: Vec<CatView>,
}

/// Phần trăm chiều cao cột (tối thiểu 0, cột lớn nhất 100).
pub fn pct(v: i64, max: i64) -> i64 {
    if max <= 0 {
        0
    } else {
        (v.max(0) * 100 / max).clamp(0, 100)
    }
}

/// GET /wallet/stats
pub async fn stats(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/wallet/stats").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let now = OffsetDateTime::now_utc();
    let (s, summary) = tokio::join!(
        wallet::stats(&state.db, subject, now),
        wallet::summary(&state.db, subject)
    );
    let (Ok(s), Ok(summary)) = (s, summary) else {
        return (StatusCode::SERVICE_UNAVAILABLE, lang.t("error.unavailable")).into_response();
    };
    let max = s
        .months
        .iter()
        .map(|m| m.spend.max(m.income))
        .max()
        .unwrap_or(0);
    let last = s.months.last().cloned().unwrap_or_default();
    render(
        StatsPage {
            i18n: lang,
            balance: coin(summary.balance, lang),
            month_spend: coin(last.spend, lang),
            month_income: coin(last.income, lang),
            bars: s
                .months
                .iter()
                .map(|m| BarView {
                    label: format!("{:02}/{}", m.month, m.year % 100),
                    spend: coin(m.spend, lang),
                    income: coin(m.income, lang),
                    spend_pct: pct(m.spend, max),
                    income_pct: pct(m.income, max),
                })
                .collect(),
            spend: s
                .spend
                .iter()
                .map(|(k, v)| CatView {
                    label: k,
                    amount: coin(*v, lang),
                })
                .collect(),
            income: s
                .income
                .iter()
                .map(|(k, v)| CatView {
                    label: k,
                    amount: coin(*v, lang),
                })
                .collect(),
        },
        lang,
        StatusCode::OK,
        None,
    )
}

// ---------- Mua app trên web ----------

/// POST /store/{id}/buy: mua bằng F-Coin rồi quay lại trang app kèm kết quả.
pub async fn buy(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(app): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let back = format!("/store/{app}");
    let subject = match current_subject(&state, lang, &headers, &back).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let expected = get(&f, "expected_price").parse::<i64>().ok();
    let email = identity_email(&state, subject).await;
    match wallet::purchase(&state, subject, &email, &app, expected).await {
        Ok(_) => see_other(&format!("{back}?buy=ok")),
        Err(OrderError::Problem { code, .. }) => see_other(&format!("{back}?buy={code}")),
        Err(e) => {
            let (status, m) = explain(lang, e);
            (status, m.text).into_response()
        }
    }
}

// ---------- API cho F-Launcher ----------

fn api_problem(e: OrderError) -> Response {
    match e {
        OrderError::Problem {
            status,
            code,
            vi,
            en,
        } => api_error(status, code, &en, &vi),
        OrderError::Db(e) => {
            tracing::error!(error = %e, "lỗi cơ sở dữ liệu ở API ví");
            api_error(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "The service is temporarily unavailable.",
                "Dịch vụ đang tạm gián đoạn.",
            )
        }
    }
}

/// GET /v1/wallet (chỉ token của F-Launcher).
pub async fn api_wallet(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    let subject = match crate::launch::launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let (summary, owned, prices) = tokio::join!(
        wallet::summary(&state.db, subject),
        wallet::owned_apps(&state.db, subject),
        wallet::priced_apps(&state.db)
    );
    match (summary, owned, prices) {
        (Ok(s), Ok(owned), Ok(prices)) => Json(json!({
            "balance": s.balance,
            "rate_vnd": s.rate,
            "frozen": s.frozen.is_some(),
            "owned": owned,
            "prices": prices.into_iter().collect::<BTreeMap<_, _>>(),
            "wallet_url": format!("{}wallet", state.public_url),
        }))
        .into_response(),
        _ => api_problem(OrderError::Db(sqlx::Error::PoolTimedOut)),
    }
}

#[derive(Deserialize)]
pub struct PurchaseRequest {
    pub app_id: String,
    pub expected_price: Option<i64>,
}

/// POST /v1/wallet/purchases (chỉ token của F-Launcher).
pub async fn api_purchase(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<PurchaseRequest>,
) -> Response {
    let subject = match crate::launch::launcher_subject(&state, &headers).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let email = identity_email(&state, subject).await;
    match wallet::purchase(&state, subject, &email, &req.app_id, req.expected_price).await {
        Ok(b) => (
            StatusCode::CREATED,
            Json(json!({"balance": b.balance, "price": b.price, "transaction": b.code})),
        )
            .into_response(),
        Err(e) => api_problem(e),
    }
}

// ---------- Bảng quản trị: Ví F-Coin ----------

pub struct AdminWithdrawalView {
    pub id: String,
    pub code: String,
    pub email: String,
    pub coins: String,
    pub amount: String,
    pub method: String,
    pub bank: bool,
    pub status: String,
    pub status_label: &'static str,
    pub approvals: String,
    pub needed: usize,
    pub note: String,
    pub at: String,
}

pub struct AdminLineView {
    pub code: String,
    pub at: String,
    pub kind: &'static str,
    pub account: String,
    pub amount: String,
    pub memo: String,
}

pub struct RateView {
    pub rate: String,
    pub by: String,
    pub reason: String,
    pub at: String,
}

pub struct DayBar {
    pub label: String,
    pub topup: String,
    pub withdraw: String,
    pub topup_pct: i64,
    pub withdraw_pct: i64,
}

#[derive(Template)]
#[template(path = "admin_wallet.html")]
struct AdminWalletPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    me: String,
    roles: String,
    nav: Vec<admin::NavItem>,
    dev_no_passkey: bool,
    finance: bool,
    circulation: String,
    escrow: String,
    pending: String,
    fees: String,
    open_withdrawals: i64,
    topup: String,
    withdraw: String,
    rate: String,
    rate_vnd: i64,
    app_fee: i64,
    project_fee: i64,
    rates: Vec<RateView>,
    days: Vec<DayBar>,
    withdrawals: Vec<AdminWithdrawalView>,
    query: String,
    lines: Vec<AdminLineView>,
    revealed: Option<(String, String)>,
}

#[derive(Deserialize, Default)]
pub struct AdminWalletQuery {
    pub q: Option<String>,
}

async fn admin_gate(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
) -> Result<admin::Admin, Box<Response>> {
    let a = admin::gate(state, lang, headers, "/admin/wallet", None).await?;
    if !(a.has(Role::Finance) || a.has(Role::Sysadmin)) {
        return Err(Box::new(
            (StatusCode::FORBIDDEN, lang.t("admin.forbidden")).into_response(),
        ));
    }
    Ok(a)
}

#[allow(clippy::too_many_arguments)]
async fn render_admin(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    a: &admin::Admin,
    query: &str,
    messages: Vec<Msg>,
    revealed: Option<(String, String)>,
    status: StatusCode,
) -> Response {
    let now = OffsetDateTime::now_utc();
    let (o, rates, open, lines) = tokio::join!(
        wallet::overview(&state.db, now),
        wallet::rate_history(&state.db, 10),
        wallet::open_withdrawals(&state.db),
        wallet::search(&state.db, query)
    );
    let o = o.unwrap_or_default();
    let max = o
        .days
        .iter()
        .map(|d| d.topup.max(d.withdraw))
        .max()
        .unwrap_or(0);
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    render(
        AdminWalletPage {
            i18n: lang,
            messages,
            csrf: token,
            me: a.email.clone(),
            roles: a
                .roles
                .iter()
                .map(|r| r.label(lang))
                .collect::<Vec<_>>()
                .join(", "),
            nav: admin::nav(lang, a, "wallet"),
            dev_no_passkey: state.admin_passkey_optional,
            finance: a.has(Role::Finance),
            circulation: coin(o.circulation, lang),
            escrow: coin(o.escrow, lang),
            pending: coin(o.withdrawals_pending, lang),
            fees: coin(o.fees, lang),
            open_withdrawals: o.open_withdrawals,
            topup: format!("{} · {}", coin(o.topup_coin, lang), vnd(o.topup_vnd)),
            withdraw: format!("{} · {}", coin(o.withdraw_coin, lang), vnd(o.withdraw_vnd)),
            rate: rate_label(lang, o.rate),
            rate_vnd: o.rate,
            app_fee: o.fee_settings.app_bps,
            project_fee: o.fee_settings.project_bps,
            rates: rates
                .unwrap_or_default()
                .into_iter()
                .map(|r| RateView {
                    rate: vnd(r.vnd_per_coin),
                    by: r.set_by,
                    reason: r.reason,
                    at: format_time(r.set_at),
                })
                .collect(),
            days: o
                .days
                .iter()
                .map(|d| DayBar {
                    label: d.label.clone(),
                    topup: coin(d.topup, lang),
                    withdraw: coin(d.withdraw, lang),
                    topup_pct: pct(d.topup, max),
                    withdraw_pct: pct(d.withdraw, max),
                })
                .collect(),
            withdrawals: open
                .unwrap_or_default()
                .into_iter()
                .map(|w| AdminWithdrawalView {
                    id: w.id.to_string(),
                    needed: w.approvals_needed(),
                    code: w.code,
                    email: w.email,
                    coins: coin(w.coins, lang),
                    amount: vnd(w.amount_vnd),
                    bank: w.method == "bank",
                    method: match (
                        w.method.as_str(),
                        &w.bank_name,
                        &w.account_holder,
                        &w.account_last4,
                    ) {
                        ("bank", Some(b), Some(h), Some(l)) => format!("{b} · {h} · ···{l}"),
                        _ => lang.t("wallet.method.source").to_owned(),
                    },
                    status_label: withdrawal_status(&w.status),
                    status: w.status,
                    approvals: w.approvals.join(", "),
                    note: w.note,
                    at: format_time(w.created_at),
                })
                .collect(),
            query: query.to_owned(),
            lines: lines
                .unwrap_or_default()
                .into_iter()
                .map(|l| AdminLineView {
                    code: l.code,
                    at: format_time(l.at),
                    kind: kind_label(l.kind),
                    account: l.account,
                    amount: signed_coin(l.amount, lang),
                    memo: l.memo,
                })
                .collect(),
            revealed,
        },
        lang,
        status,
        set,
    )
}

/// GET /admin/wallet
pub async fn admin_page(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    Query(q): Query<AdminWalletQuery>,
) -> Response {
    let a = match admin_gate(&state, lang, &headers).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    render_admin(
        &state,
        lang,
        &headers,
        &a,
        q.q.as_deref().unwrap_or_default(),
        Vec::new(),
        None,
        StatusCode::OK,
    )
    .await
}

/// POST /admin/wallet/{action}: rate, fees, freeze, unfreeze.
/// POST /admin/wallet/withdrawals/{id}/{action}: approve, reject, paid, reveal.
pub async fn admin_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(action): Path<String>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    admin_do(state, lang, None, action, headers, body).await
}

pub async fn admin_withdrawal_action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    admin_do(state, lang, Some(id), action, headers, body).await
}

async fn admin_do(
    state: Arc<AppState>,
    lang: Lang,
    id: Option<Uuid>,
    action: String,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let f = form(&body);
    if !csrf::verify(&headers, Some(get(&f, csrf::FIELD))) {
        return (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response();
    }
    let a = match admin_gate(&state, lang, &headers).await {
        Ok(a) => a,
        Err(r) => return *r,
    };
    // Mọi thao tác trừ khóa/mở ví cần vai trò con Tài chính.
    let needs_finance = !matches!(action.as_str(), "freeze" | "unfreeze");
    if needs_finance && !a.has(Role::Finance) {
        return (StatusCode::FORBIDDEN, lang.t("admin.forbidden")).into_response();
    }
    let me = a.email.clone();
    let mut revealed = None;
    let result = match (id, action.as_str()) {
        (None, "rate") => {
            wallet::set_rate(
                &state,
                &me,
                get(&f, "rate").trim().parse().unwrap_or(0),
                get(&f, "reason"),
            )
            .await
        }
        (None, "fees") => {
            wallet::set_fees(
                &state,
                &me,
                wallet::Fees {
                    app_bps: get(&f, "app_bps").trim().parse().unwrap_or(-1),
                    project_bps: get(&f, "project_bps").trim().parse().unwrap_or(-1),
                },
            )
            .await
        }
        (None, "freeze") | (None, "unfreeze") => {
            wallet::set_frozen(
                &state,
                &me,
                get(&f, "email"),
                action == "freeze",
                get(&f, "reason"),
            )
            .await
        }
        (Some(id), "approve") => wallet::approve_withdrawal(&state, id, &me)
            .await
            .map(|_| ()),
        (Some(id), "reject") => wallet::reject_withdrawal(&state, id, &me, get(&f, "reason")).await,
        (Some(id), "paid") => wallet::mark_paid(&state, id, &me, get(&f, "reference")).await,
        (Some(id), "reveal") => match wallet::reveal_account(&state, id, &me).await {
            Ok(n) => {
                let code = wallet::withdrawal(&state.db, id)
                    .await
                    .map(|w| w.code)
                    .unwrap_or_default();
                revealed = Some((code, n));
                Ok(())
            }
            Err(e) => Err(e),
        },
        _ => return (StatusCode::NOT_FOUND, lang.t("error.not_found")).into_response(),
    };
    let (messages, status) = match result {
        Ok(()) => (vec![Msg::success(lang.t("admin.saved"))], StatusCode::OK),
        Err(e) => {
            let (s, m) = explain(lang, e);
            (vec![m], s)
        }
    };
    render_admin(&state, lang, &headers, &a, "", messages, revealed, status).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bar_percentages() {
        assert_eq!(pct(50, 100), 50);
        assert_eq!(pct(0, 0), 0);
        assert_eq!(pct(200, 100), 100);
        assert_eq!(pct(-5, 100), 0);
    }

    #[test]
    fn rate_reads_per_language() {
        assert_eq!(rate_label(Lang::Vi, 1000), "1 F-Coin = 1.000 ₫");
        assert_eq!(rate_label(Lang::En, 1200), "1 F-Coin = 1.200 VND");
    }
}
