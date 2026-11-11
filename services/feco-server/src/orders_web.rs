//! Trang web đơn xây app cho khách hàng (spec custom-build-orders):
//! - `GET /orders`, `POST /orders` (multipart: yêu cầu, tệp đính kèm tối đa 200 MB, chấp nhận
//!   Điều khoản dịch vụ xây app);
//! - `GET /orders/{id}`: trạng thái, lịch sử, báo giá, các mốc, thanh toán, tin nhắn;
//! - `POST /orders/{id}/messages` (multipart, có tệp), `POST /orders/{id}/{thao tác}`;
//! - `GET /orders/{id}/files/{file}`, `GET /orders/{id}/handover` (chỉ khi Hoàn tất và trả đủ);
//! - `GET /policy/custom-build`: Điều khoản dịch vụ xây app (bản nháp, chờ luật sư rà soát).
//!
//! Đội F-Eco báo giá, bàn giao, trả lời và giải quyết tranh chấp qua `feco-catalog order-*` cho
//! tới khi có bảng quản trị (nhóm 14).

use std::{path::PathBuf, sync::Arc};

use askama::Template;
use axum::{
    body::{Body, Bytes},
    extract::{Multipart, Path, Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Response},
};
use serde::Deserialize;
use sha2::Digest;
use time::OffsetDateTime;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

use crate::{
    connected::current_subject,
    csrf,
    i18n::Lang,
    orders::{self, OrderError, OrderResult, Status},
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
            tracing::error!(error = %e, "lỗi cơ sở dữ liệu ở trang đơn hàng");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Msg::error(lang.t("error.unavailable")),
            )
        }
    }
}

fn csrf_failed(lang: Lang) -> Response {
    (StatusCode::FORBIDDEN, lang.t("error.csrf")).into_response()
}

fn when(t: OffsetDateTime) -> String {
    t.format(&time::macros::format_description!(
        "[day]/[month]/[year] [hour]:[minute]"
    ))
    .unwrap_or_default()
}

/// Email đã xác minh của người dùng, hoặc lỗi "cần xác minh email" (spec "Gửi yêu cầu xây app").
async fn verified_email(state: &AppState, subject: Uuid) -> OrderResult<String> {
    let identity = state
        .kratos
        .get_identity(&subject.to_string())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "không đọc được danh tính");
            orders::problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Dịch vụ đang tạm gián đoạn.",
                "The service is temporarily unavailable.",
            )
        })?
        .unwrap_or_default();
    let email = identity["traits"]["email"]
        .as_str()
        .unwrap_or_default()
        .to_owned();
    let verified = identity["verifiable_addresses"]
        .as_array()
        .into_iter()
        .flatten()
        .any(|a| a["value"] == email.as_str() && a["verified"] == true);
    if !verified {
        return Err(orders::problem(
            StatusCode::FORBIDDEN,
            "email_unverified",
            "Bạn cần xác minh email trước khi gửi yêu cầu xây app.",
            "Verify your email before sending a custom-build request.",
        ));
    }
    Ok(email)
}

// ---------- Danh sách và gửi yêu cầu ----------

pub struct OrderItem {
    pub id: String,
    pub code: String,
    pub title: String,
    pub status: String,
    pub created: String,
}

#[derive(Template)]
#[template(path = "orders.html")]
struct OrdersPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    blocked: bool,
    orders: Vec<OrderItem>,
    tos_version: &'static str,
    title: String,
    description: String,
}

#[derive(Debug, Deserialize)]
pub struct OkQuery {
    pub ok: Option<String>,
}

async fn render_list(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    mut messages: Vec<Msg>,
    status: StatusCode,
    draft: (String, String),
) -> Response {
    let blocked = match verified_email(state, subject).await {
        Ok(_) => false,
        Err(e) => {
            let (_, m) = explain(lang, e);
            messages.push(m);
            true
        }
    };
    let list = match orders::list_for(&state.db, subject).await {
        Ok(l) => l,
        Err(e) => {
            let (status, m) = explain(lang, e);
            return (status, m.text).into_response();
        }
    };
    let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
    html(
        OrdersPage {
            i18n: lang,
            messages,
            csrf: token,
            blocked,
            orders: list
                .into_iter()
                .map(|o| OrderItem {
                    id: o.id.to_string(),
                    code: o.code,
                    title: o.title,
                    status: o.status.label(lang == Lang::Vi).to_owned(),
                    created: when(o.created_at),
                })
                .collect(),
            tos_version: orders::TOS_VERSION,
            title: draft.0,
            description: draft.1,
        },
        lang,
        status,
        set,
    )
}

/// GET /orders
pub async fn list(State(state): State<Arc<AppState>>, lang: Lang, headers: HeaderMap) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/orders").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_list(
        &state,
        lang,
        &headers,
        subject,
        Vec::new(),
        StatusCode::OK,
        Default::default(),
    )
    .await
}

/// Tệp đang nhận: ghi dần ra đĩa, băm SHA-256, chặn khi tổng vượt `limit`.
struct Incoming {
    path: PathBuf,
    file: tokio::fs::File,
    hasher: sha2::Sha256,
    size: u64,
}

impl Incoming {
    async fn create(state: &AppState) -> std::io::Result<Self> {
        let dir = state.package_dir.join("orders").join(".incoming");
        tokio::fs::create_dir_all(&dir).await?;
        let path = dir.join(Uuid::new_v4().to_string());
        let file = tokio::fs::File::create(&path).await?;
        Ok(Incoming {
            path,
            file,
            hasher: sha2::Sha256::new(),
            size: 0,
        })
    }

    async fn push(&mut self, chunk: &[u8]) -> std::io::Result<()> {
        self.size += chunk.len() as u64;
        self.hasher.update(chunk);
        self.file.write_all(chunk).await
    }

    async fn finish(mut self) -> std::io::Result<(PathBuf, String, u64)> {
        self.file.flush().await?;
        let sha = self
            .hasher
            .finalize()
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect();
        Ok((self.path, sha, self.size))
    }
}

/// Một tệp đã nhận: tên gốc, đường dẫn tạm, SHA-256, kích thước.
type Received = (String, PathBuf, String, u64);

/// Đọc các trường của biểu mẫu multipart. Tệp được ghi ra thư mục tạm; tổng vượt `limit` thì dừng
/// và xóa mọi tệp đã nhận (spec "Tệp quá lớn").
async fn read_multipart(
    state: &AppState,
    headers: &HeaderMap,
    multipart: &mut Multipart,
    limit: u64,
) -> OrderResult<(Vec<(String, String)>, Vec<Received>)> {
    let bad = |e: axum::extract::multipart::MultipartError| {
        orders::problem(
            StatusCode::BAD_REQUEST,
            "invalid_request",
            format!("Biểu mẫu không hợp lệ: {e}"),
            format!("Invalid form: {e}"),
        )
    };
    let io = |e: std::io::Error| {
        tracing::error!(error = %e, "không lưu được tệp đính kèm");
        orders::problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "Không lưu được tệp đính kèm.",
            "Couldn't store the attachment.",
        )
    };
    let mut fields = Vec::new();
    let mut files: Vec<Received> = Vec::new();
    let mut csrf_ok = false;
    let mut total = 0u64;
    let mut over = false;
    let result: OrderResult<()> = async {
        loop {
            let next = multipart.next_field().await;
            // Đã vượt giới hạn: đọc bỏ phần còn lại để trình duyệt nhận được trang báo lỗi thay vì
            // kết nối bị cắt; lỗi đọc (kể cả quá giới hạn thân yêu cầu) vẫn báo "tệp quá lớn".
            if over {
                match next {
                    Ok(Some(mut field)) => {
                        while let Ok(Some(_)) = field.chunk().await {}
                        continue;
                    }
                    _ => return Err(orders::too_large()),
                }
            }
            let Some(mut field) = next.map_err(bad)? else {
                break;
            };
            let name = field.name().unwrap_or_default().to_owned();
            if name == csrf::FIELD {
                csrf_ok = csrf::verify(headers, Some(&field.text().await.map_err(bad)?));
                continue;
            }
            // Biểu mẫu đặt token CSRF đầu tiên: chưa hợp lệ thì không nhận gì thêm.
            if !csrf_ok {
                return Err(orders::problem(
                    StatusCode::FORBIDDEN,
                    "csrf",
                    "Phiên biểu mẫu hết hạn, hãy tải lại trang.",
                    "The form expired; reload the page.",
                ));
            }
            match field.file_name().map(str::to_owned) {
                Some(file_name) if name == "files" => {
                    if file_name.is_empty() {
                        continue;
                    }
                    let mut incoming = Incoming::create(state).await.map_err(io)?;
                    let mut failed = None;
                    loop {
                        let chunk = match field.chunk().await {
                            Ok(Some(c)) => c,
                            Ok(None) => break,
                            Err(e) => {
                                failed = Some(bad(e));
                                break;
                            }
                        };
                        total += chunk.len() as u64;
                        if total > limit {
                            over = true;
                            while let Ok(Some(_)) = field.chunk().await {}
                            break;
                        }
                        if let Err(e) = incoming.push(&chunk).await {
                            failed = Some(io(e));
                            break;
                        }
                    }
                    if over || failed.is_some() {
                        // Đóng tệp trước khi xóa (Windows không xóa được tệp đang mở).
                        let path = incoming.path.clone();
                        drop(incoming);
                        let _ = tokio::fs::remove_file(&path).await;
                        match failed {
                            Some(e) => return Err(e),
                            None => continue,
                        }
                    }
                    let (path, sha, size) = incoming.finish().await.map_err(io)?;
                    if size == 0 {
                        let _ = tokio::fs::remove_file(&path).await;
                        continue;
                    }
                    files.push((file_name, path, sha, size));
                }
                _ => {
                    let text = field.text().await.map_err(bad)?;
                    fields.push((name, text));
                }
            }
        }
        if !csrf_ok {
            return Err(orders::problem(
                StatusCode::FORBIDDEN,
                "csrf",
                "Phiên biểu mẫu hết hạn, hãy tải lại trang.",
                "The form expired; reload the page.",
            ));
        }
        Ok(())
    }
    .await;
    if let Err(e) = result {
        for (_, path, _, _) in &files {
            let _ = tokio::fs::remove_file(path).await;
        }
        return Err(e);
    }
    Ok((fields, files))
}

fn field<'a>(fields: &'a [(String, String)], k: &str) -> &'a str {
    fields
        .iter()
        .find(|(n, _)| n == k)
        .map(|(_, v)| v.as_str())
        .unwrap_or_default()
}

/// Chuyển các tệp đã nhận vào thư mục của đơn và ghi vào cơ sở dữ liệu.
async fn keep_files(
    state: &AppState,
    order: Uuid,
    message: Option<i64>,
    files: Vec<Received>,
) -> OrderResult<()> {
    let dir = state
        .package_dir
        .join("orders")
        .join(order.to_string())
        .join("files");
    let mut rest = files.into_iter();
    let result: OrderResult<()> = async {
        tokio::fs::create_dir_all(&dir).await.map_err(|e| {
            orders::problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                format!("Không lưu được tệp: {e}"),
                format!("Couldn't store the file: {e}"),
            )
        })?;
        for (name, tmp, sha, size) in rest.by_ref() {
            let to = dir.join(Uuid::new_v4().to_string());
            if let Err(e) = tokio::fs::rename(&tmp, &to).await {
                let _ = tokio::fs::remove_file(&tmp).await;
                return Err(orders::problem(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "unavailable",
                    format!("Không lưu được tệp: {e}"),
                    format!("Couldn't store the file: {e}"),
                ));
            }
            if let Err(e) =
                orders::add_file(&state.db, order, message, &name, &to, size, &sha).await
            {
                let _ = tokio::fs::remove_file(&to).await;
                return Err(e);
            }
        }
        Ok(())
    }
    .await;
    for (_, tmp, _, _) in rest {
        let _ = tokio::fs::remove_file(&tmp).await;
    }
    result
}

/// POST /orders (multipart): tạo đơn, lưu tệp, gửi yêu cầu.
pub async fn create(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, "/orders").await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let mut draft = (String::new(), String::new());
    let result: OrderResult<Uuid> = async {
        let email = verified_email(&state, subject).await?;
        let (fields, files) = read_multipart(
            &state,
            &headers,
            &mut multipart,
            orders::MAX_ATTACHMENTS_BYTES,
        )
        .await?;
        draft = (
            field(&fields, "title").to_owned(),
            field(&fields, "description").to_owned(),
        );
        let platforms: Vec<String> = fields
            .iter()
            .filter(|(k, _)| k == "platform")
            .map(|(_, v)| v.clone())
            .collect();
        let wanted_by = match field(&fields, "wanted_by").trim() {
            "" => None,
            s => Some(
                time::Date::parse(
                    s,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .map_err(|_| {
                    orders::problem(
                        StatusCode::BAD_REQUEST,
                        "invalid_request",
                        "Thời hạn mong muốn không hợp lệ.",
                        "The wanted-by date is not valid.",
                    )
                })?,
            ),
        };
        let budget_vnd = match field(&fields, "budget_vnd")
            .replace(['.', ',', ' '], "")
            .as_str()
        {
            "" => None,
            s => Some(s.parse::<i64>().ok().filter(|n| *n >= 0).ok_or_else(|| {
                orders::problem(
                    StatusCode::BAD_REQUEST,
                    "invalid_request",
                    "Ngân sách phải là số tiền VND.",
                    "The budget must be an amount in VND.",
                )
            })?),
        };
        let accept = field(&fields, "accept_tos") == "on";
        if !accept {
            for (_, path, _, _) in &files {
                let _ = tokio::fs::remove_file(path).await;
            }
            return Err(orders::problem(
                StatusCode::BAD_REQUEST,
                "tos_required",
                "Bạn cần chấp nhận Điều khoản dịch vụ xây app trước khi gửi.",
                "Accept the custom-build Terms of Service before submitting.",
            ));
        }
        let new = orders::NewOrder {
            title: draft.0.clone(),
            description: draft.1.clone(),
            platforms,
            wanted_by,
            budget_vnd,
        };
        let order = match orders::create_draft(&state, subject, &email, &new).await {
            Ok(o) => o,
            Err(e) => {
                for (_, path, _, _) in &files {
                    let _ = tokio::fs::remove_file(path).await;
                }
                return Err(e);
            }
        };
        keep_files(&state, order.id, None, files).await?;
        orders::submit(&state, subject, order.id, accept, OffsetDateTime::now_utc()).await?;
        Ok(order.id)
    }
    .await;
    match result {
        Ok(id) => see_other(&format!("/orders/{id}?ok=submitted")),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            render_list(&state, lang, &headers, subject, vec![msg], status, draft).await
        }
    }
}

// ---------- Chi tiết đơn ----------

pub struct EventView {
    pub at: String,
    pub from: String,
    pub to: String,
    pub actor: String,
    pub note: String,
}

pub struct MilestoneView {
    pub position: i32,
    pub title: String,
    pub deliverable: String,
    pub amount: String,
    pub days: i32,
    pub due: String,
    pub status: String,
    pub paid: bool,
    pub free_revisions: i32,
    pub revisions_left: i32,
    pub build: String,
    pub notes: String,
    pub check_report: String,
    pub delivered: String,
    pub can_review: bool,
    pub can_dispute: bool,
}

pub struct QuoteView {
    pub number: i32,
    pub scope: String,
    pub exclusions: String,
    pub ownership: String,
    pub valid_until: String,
    pub total: String,
    pub deposit: String,
    pub open: bool,
    pub expired: bool,
    pub accepted: bool,
    pub milestones: Vec<MilestoneView>,
}

pub struct PaymentView {
    pub kind: String,
    pub amount: String,
    pub status: String,
    pub reason: String,
}

pub struct FileView {
    pub id: String,
    pub name: String,
    pub size: String,
}

pub struct MessageView {
    pub author: String,
    pub team: bool,
    pub kind: String,
    pub body: String,
    pub at: String,
    pub files: Vec<FileView>,
}

#[derive(Template)]
#[template(path = "order.html")]
struct OrderPage {
    i18n: Lang,
    messages: Vec<Msg>,
    csrf: String,
    id: String,
    code: String,
    title: String,
    description: String,
    status: String,
    status_code: &'static str,
    platforms: String,
    wanted_by: String,
    budget: String,
    quote_due: String,
    tos_version: String,
    history: Vec<EventView>,
    quote: Option<QuoteView>,
    payments: Vec<PaymentView>,
    files: Vec<FileView>,
    thread: Vec<MessageView>,
    can_cancel: bool,
    can_adjust: bool,
    /// Báo giá mở đã hết hạn: chỉ còn nút "Yêu cầu báo giá lại".
    quote_expired: bool,
    handover_ready: bool,
}

fn actor_label(lang: Lang, actor: &str) -> String {
    let vi = lang == Lang::Vi;
    if actor.starts_with("customer:") {
        (if vi { "Bạn" } else { "You" }).into()
    } else if let Some(name) = actor.strip_prefix("team:") {
        format!("{} ({name})", if vi { "Đội F-Eco" } else { "F-Eco team" })
    } else {
        (if vi { "Hệ thống" } else { "System" }).into()
    }
}

fn milestone_status(lang: Lang, s: &str) -> String {
    let vi = lang == Lang::Vi;
    match s {
        "pending" => {
            if vi {
                "Chưa tới"
            } else {
                "Not started"
            }
        }
        "awaiting_payment" => {
            if vi {
                "Chờ thanh toán"
            } else {
                "Awaiting payment"
            }
        }
        "in_progress" => {
            if vi {
                "Đang thực hiện"
            } else {
                "In progress"
            }
        }
        "delivered" => {
            if vi {
                "Chờ nghiệm thu"
            } else {
                "Awaiting acceptance"
            }
        }
        "accepted" => {
            if vi {
                "Đã nghiệm thu"
            } else {
                "Accepted"
            }
        }
        "refunded" => {
            if vi {
                "Đã hoàn tiền"
            } else {
                "Refunded"
            }
        }
        other => other,
    }
    .to_owned()
}

fn payment_status(lang: Lang, s: &str) -> String {
    let vi = lang == Lang::Vi;
    match s {
        "pending" => {
            if vi {
                "Chờ xử lý"
            } else {
                "Pending"
            }
        }
        "succeeded" => {
            if vi {
                "Thành công"
            } else {
                "Succeeded"
            }
        }
        "cancelled" => {
            if vi {
                "Đã hủy"
            } else {
                "Cancelled"
            }
        }
        "failed" => {
            if vi {
                "Thất bại"
            } else {
                "Failed"
            }
        }
        other => other,
    }
    .to_owned()
}

fn size(n: i64) -> String {
    match n {
        n if n >= 1024 * 1024 => format!("{:.1} MB", n as f64 / 1024.0 / 1024.0),
        n if n >= 1024 => format!("{} KB", n / 1024),
        n => format!("{n} B"),
    }
}

async fn render_order(
    state: &AppState,
    lang: Lang,
    headers: &HeaderMap,
    subject: Uuid,
    id: Uuid,
    messages: Vec<Msg>,
    status: StatusCode,
) -> Response {
    let vi = lang == Lang::Vi;
    let now = OffsetDateTime::now_utc();
    let result: OrderResult<OrderPage> = async {
        let o = orders::of_customer(&state.db, subject, id).await?;
        let history = orders::history(&state.db, id)
            .await?
            .into_iter()
            .map(|e| EventView {
                at: when(e.at),
                from: e
                    .from
                    .as_deref()
                    .and_then(Status::parse)
                    .map(|s| s.label(vi).to_owned())
                    .unwrap_or_default(),
                to: Status::parse(&e.to)
                    .map(|s| s.label(vi).to_owned())
                    .unwrap_or(e.to),
                actor: actor_label(lang, &e.actor),
                note: e.note,
            })
            .collect();
        let quote = orders::current_quote(&state.db, id).await?.map(|q| {
            let accepted = q.status == "accepted";
            QuoteView {
                number: q.number,
                total: orders::vnd(q.total()),
                deposit: q
                    .milestones
                    .first()
                    .map(|m| orders::vnd(m.amount_vnd))
                    .unwrap_or_default(),
                open: q.status == "open" && o.status == Status::Quoted,
                expired: q.expired(now),
                accepted,
                valid_until: orders::date(q.valid_until),
                milestones: q
                    .milestones
                    .iter()
                    .map(|m| MilestoneView {
                        position: m.position,
                        title: m.title.clone(),
                        deliverable: m.deliverable.clone(),
                        amount: orders::vnd(m.amount_vnd),
                        days: m.days,
                        due: m.due_at.map(orders::date).unwrap_or_default(),
                        status: milestone_status(lang, &m.status),
                        paid: m.paid,
                        free_revisions: m.free_revisions,
                        revisions_left: m.revisions_left(),
                        build: match (&m.app_id, &m.app_version) {
                            (Some(a), Some(v)) => format!("{a} {v}"),
                            _ => String::new(),
                        },
                        notes: m.notes.clone(),
                        check_report: m.check_report.clone(),
                        delivered: m.delivered_at.map(when).unwrap_or_default(),
                        can_review: accepted
                            && m.status == "delivered"
                            && o.status == Status::Delivered,
                        can_dispute: accepted
                            && matches!(
                                o.status,
                                Status::InProgress | Status::Delivered | Status::Completed
                            )
                            && m.delivered_at.is_some_and(|d| {
                                now - d <= time::Duration::days(orders::DISPUTE_WINDOW_DAYS)
                            }),
                    })
                    .collect(),
                scope: q.scope,
                exclusions: q.exclusions,
                ownership: q.ownership,
            }
        });
        let payments = orders::payment_requests(&state.db, id)
            .await?
            .into_iter()
            .map(|p| PaymentView {
                kind: if p.kind == "refund" {
                    (if vi { "Hoàn tiền" } else { "Refund" }).into()
                } else {
                    (if vi { "Thu tiền" } else { "Charge" }).into()
                },
                amount: orders::vnd(p.amount_vnd),
                status: payment_status(lang, &p.status),
                reason: p.reason,
            })
            .collect();
        let all_files = orders::files(&state.db, id).await?;
        let view = |f: &orders::FileInfo| FileView {
            id: f.id.to_string(),
            name: f.name.clone(),
            size: size(f.size),
        };
        let thread = orders::messages(&state.db, id)
            .await?
            .into_iter()
            .map(|m| MessageView {
                author: match m.author.as_str() {
                    "team" => format!(
                        "{}{}",
                        if vi { "Đội F-Eco" } else { "F-Eco team" },
                        if m.author_name.is_empty() {
                            String::new()
                        } else {
                            format!(" ({})", m.author_name)
                        }
                    ),
                    "customer" => (if vi { "Bạn" } else { "You" }).into(),
                    _ => (if vi { "Hệ thống" } else { "System" }).into(),
                },
                team: m.author == "team",
                kind: match (m.kind.as_str(), vi) {
                    ("adjustment", true) => "Đề nghị điều chỉnh",
                    ("adjustment", false) => "Adjustment request",
                    ("revision", true) => "Yêu cầu sửa",
                    ("revision", false) => "Revision request",
                    ("dispute", true) => "Tranh chấp",
                    ("dispute", false) => "Dispute",
                    _ => "",
                }
                .into(),
                files: all_files
                    .iter()
                    .filter(|f| f.message_id == Some(m.id))
                    .map(view)
                    .collect(),
                body: m.body,
                at: when(m.at),
            })
            .collect();
        Ok(OrderPage {
            i18n: lang,
            messages: Vec::new(),
            csrf: String::new(),
            id: o.id.to_string(),
            title: o.title.clone(),
            description: o.description.clone(),
            status: o.status.label(vi).to_owned(),
            status_code: o.status.as_str(),
            platforms: o
                .platforms
                .iter()
                .map(|p| if p == "macos" { "macOS" } else { "Windows" })
                .collect::<Vec<_>>()
                .join(", "),
            wanted_by: o
                .wanted_by
                .map(|d| format!("{:02}/{:02}/{}", d.day(), u8::from(d.month()), d.year()))
                .unwrap_or_default(),
            budget: o.budget_vnd.map(orders::vnd).unwrap_or_default(),
            quote_due: if o.status == Status::Submitted {
                o.quote_due_at.map(orders::date).unwrap_or_default()
            } else {
                String::new()
            },
            tos_version: o.tos_version.clone().unwrap_or_default(),
            history,
            can_adjust: o.status == Status::Quoted,
            quote_expired: quote.as_ref().is_some_and(|q| q.open && q.expired),
            quote,
            payments,
            files: all_files
                .iter()
                .filter(|f| f.message_id.is_none())
                .map(view)
                .collect(),
            thread,
            can_cancel: !matches!(o.status, Status::Completed | Status::Cancelled),
            handover_ready: orders::handover_ready(&state.db, &o).await?,
            code: o.code,
        })
    }
    .await;
    match result {
        Ok(mut page) => {
            let (token, set) = csrf::issue(headers, state.public_url.scheme() == "https");
            page.csrf = token;
            page.messages = messages;
            html(page, lang, status, set)
        }
        Err(e) => {
            let (status, msg) = explain(lang, e);
            (status, msg.text).into_response()
        }
    }
}

fn ok_message(lang: Lang, key: Option<&str>) -> Vec<Msg> {
    let k = match key {
        Some("submitted") => "order.ok.submitted",
        Some("accepted") => "order.ok.accepted",
        Some("adjust") => "order.ok.adjust",
        Some("milestone") => "order.ok.milestone",
        Some("revision") => "order.ok.revision",
        Some("dispute") => "order.ok.dispute",
        Some("cancel") => "order.ok.cancel",
        Some("message") => "order.ok.message",
        _ => return Vec::new(),
    };
    vec![Msg::success(lang.t(k))]
}

/// GET /orders/{id}
pub async fn show(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    Query(q): Query<OkQuery>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/orders/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    render_order(
        &state,
        lang,
        &headers,
        subject,
        id,
        ok_message(lang, q.ok.as_deref()),
        StatusCode::OK,
    )
    .await
}

/// POST /orders/{id}/{action}: chấp nhận báo giá, đề nghị điều chỉnh, nghiệm thu, yêu cầu sửa,
/// tranh chấp, hủy đơn.
pub async fn action(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, action)): Path<(Uuid, String)>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    let form: Vec<(String, String)> = form_urlencoded::parse(&body).into_owned().collect();
    if !csrf::verify(&headers, Some(field(&form, csrf::FIELD))) {
        return csrf_failed(lang);
    }
    let subject = match current_subject(&state, lang, &headers, &format!("/orders/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let now = OffsetDateTime::now_utc();
    let position = field(&form, "milestone").parse::<i32>().unwrap_or(0);
    let note = field(&form, "note");
    let result: OrderResult<&'static str> = async {
        // Đơn của người khác: "không có" trước mọi thao tác.
        orders::of_customer(&state.db, subject, id).await?;
        match action.as_str() {
            "accept-quote" => orders::accept_quote(&state, subject, id, now)
                .await
                .map(|_| "accepted"),
            "adjust" => orders::ask_adjustment(&state, subject, id, note)
                .await
                .map(|_| "adjust"),
            "accept-milestone" => orders::accept_milestone(
                &state,
                id,
                position,
                &orders::customer_actor(subject),
                now,
            )
            .await
            .map(|_| "milestone"),
            "revise" => orders::request_revision(&state, subject, id, position, note)
                .await
                .map(|_| "revision"),
            "dispute" => orders::open_dispute(&state, subject, id, position, note, now)
                .await
                .map(|_| "dispute"),
            "cancel" => orders::cancel(&state, subject, id, note)
                .await
                .map(|_| "cancel"),
            _ => Err(orders::problem(
                StatusCode::NOT_FOUND,
                "not_found",
                "Không có thao tác này.",
                "No such action.",
            )),
        }
    }
    .await;
    match result {
        Ok(ok) => see_other(&format!("/orders/{id}?ok={ok}")),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            if status == StatusCode::NOT_FOUND {
                return (status, msg.text).into_response();
            }
            render_order(&state, lang, &headers, subject, id, vec![msg], status).await
        }
    }
}

/// Giới hạn tệp kèm một tin nhắn.
const MAX_MESSAGE_FILES_BYTES: u64 = 50 * 1024 * 1024;

/// POST /orders/{id}/messages (multipart: feco_csrf, body, files)
pub async fn post_message(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
    mut multipart: Multipart,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/orders/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let result: OrderResult<()> = async {
        orders::of_customer(&state.db, subject, id).await?;
        let (fields, files) =
            read_multipart(&state, &headers, &mut multipart, MAX_MESSAGE_FILES_BYTES).await?;
        let message = match orders::post_message(&state, id, None, field(&fields, "body")).await {
            Ok(m) => m,
            Err(e) => {
                for (_, path, _, _) in &files {
                    let _ = tokio::fs::remove_file(path).await;
                }
                return Err(e);
            }
        };
        keep_files(&state, id, Some(message), files).await
    }
    .await;
    match result {
        Ok(()) => see_other(&format!("/orders/{id}?ok=message#thread")),
        Err(e) => {
            let (status, msg) = explain(lang, e);
            if status == StatusCode::NOT_FOUND || status == StatusCode::FORBIDDEN {
                return (status, msg.text).into_response();
            }
            render_order(&state, lang, &headers, subject, id, vec![msg], status).await
        }
    }
}

async fn send_file(path: &std::path::Path, name: &str, content_type: &str) -> Response {
    match tokio::fs::File::open(path).await {
        Ok(f) => {
            // Đọc từng khối 64 KB, không nạp cả tệp (tới 200 MB) vào bộ nhớ.
            let stream = futures_util::stream::unfold(Some(f), |file| async move {
                let mut f = file?;
                let mut buf = vec![0u8; 64 * 1024];
                match f.read(&mut buf).await {
                    Ok(0) => None,
                    Ok(n) => {
                        buf.truncate(n);
                        Some((Ok::<_, std::io::Error>(Bytes::from(buf)), Some(f)))
                    }
                    Err(e) => Some((Err(e), None)),
                }
            });
            let mut res = Body::from_stream(stream).into_response();
            let h = res.headers_mut();
            h.insert(
                header::CONTENT_TYPE,
                HeaderValue::from_str(content_type)
                    .unwrap_or(HeaderValue::from_static("application/octet-stream")),
            );
            let safe: String = name
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_') {
                        c
                    } else {
                        '_'
                    }
                })
                .collect();
            if let Ok(v) = HeaderValue::from_str(&format!("attachment; filename=\"{safe}\"")) {
                h.insert(header::CONTENT_DISPOSITION, v);
            }
            h.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            res
        }
        Err(_) => StatusCode::NOT_FOUND.into_response(),
    }
}

/// GET /orders/{id}/files/{file}: tệp của đơn, chỉ khách hàng của đơn tải được.
pub async fn download_file(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path((id, file)): Path<(Uuid, Uuid)>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/orders/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    if orders::of_customer(&state.db, subject, id).await.is_err() {
        return StatusCode::NOT_FOUND.into_response();
    }
    let row: Option<(String, String)> =
        sqlx::query_as("SELECT name, path FROM order_files WHERE id = $1 AND order_id = $2")
            .bind(file)
            .bind(id)
            .fetch_optional(&state.db)
            .await
            .unwrap_or_default();
    match row {
        Some((name, path)) => {
            send_file(
                std::path::Path::new(&path),
                &name,
                "application/octet-stream",
            )
            .await
        }
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

/// GET /orders/{id}/handover: bàn giao cuối (ZIP), chỉ khi đơn Hoàn tất và đã thanh toán đủ.
pub async fn handover(
    State(state): State<Arc<AppState>>,
    lang: Lang,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Response {
    let subject = match current_subject(&state, lang, &headers, &format!("/orders/{id}")).await {
        Ok(s) => s,
        Err(r) => return *r,
    };
    let order = match orders::of_customer(&state.db, subject, id).await {
        Ok(o) => o,
        Err(e) => return (explain(lang, e).0).into_response(),
    };
    match orders::handover_ready(&state.db, &order).await {
        Ok(true) => {}
        Ok(false) => {
            return (StatusCode::FORBIDDEN, lang.t("order.handover_locked")).into_response()
        }
        Err(e) => return (explain(lang, e).0).into_response(),
    }
    let path = orders::handover_path(&state, &order);
    if !path.exists() {
        return (StatusCode::NOT_FOUND, lang.t("order.handover_missing")).into_response();
    }
    send_file(&path, &format!("{}.zip", order.code), "application/zip").await
}

// ---------- Điều khoản dịch vụ xây app ----------

pub struct TermView {
    pub title: &'static str,
    pub body: &'static str,
}

/// Bản nháp Điều khoản dịch vụ xây app (tasks.md 12.6: cần luật sư rà soát trước khi dùng thật).
/// (tiêu đề Việt, nội dung Việt, tiêu đề Anh, nội dung Anh)
pub const TERMS: &[(&str, &str, &str, &str)] = &[
    (
        "Phạm vi công việc",
        "Đội F-Eco chỉ làm những gì ghi trong phần \"Phạm vi\" của báo giá đã được chấp nhận. Những gì ghi trong \"Không bao gồm\", và mọi yêu cầu mới sau khi chấp nhận, cần báo giá bổ sung.",
        "Scope of work",
        "The F-Eco team builds only what the accepted quote lists under \"Scope\". Anything under \"Not included\", and any new request after acceptance, needs an additional quote.",
    ),
    (
        "Báo giá, mốc và thanh toán",
        "Báo giá có hiệu lực 7 ngày (trừ khi ghi khác). Mốc đầu là tiền đặt cọc, không quá 30% tổng giá; công việc bắt đầu khi đặt cọc thành công và thời hạn các mốc tính từ ngày đó. Mỗi mốc sau được thanh toán khi mốc trước được nghiệm thu. Giá bằng VND, đã gồm thuế.",
        "Quotes, milestones and payment",
        "A quote is valid for 7 days unless it says otherwise. The first milestone is the deposit, at most 30% of the total; work starts when the deposit is paid and milestone deadlines count from that day. Each later milestone is paid when the one before it is accepted. Prices are in VND, tax included.",
    ),
    (
        "Bàn giao, nghiệm thu và sửa lỗi",
        "Mỗi mốc được bàn giao qua F-Launcher dưới dạng app riêng tư chỉ bạn thấy, kèm ghi chú thay đổi và kết quả kiểm tra. Bạn nghiệm thu hoặc yêu cầu sửa trong 7 ngày; không phản hồi thì nền tảng nhắc, và mốc tự nghiệm thu sau 14 ngày. Mỗi mốc có số lượt sửa miễn phí ghi trong báo giá (mặc định 2); sửa thêm cần báo giá bổ sung.",
        "Delivery, acceptance and fixes",
        "Each milestone is delivered through F-Launcher as a private app only you can see, with change notes and test results. You accept it or ask for changes within 7 days; without an answer the platform reminds you, and the milestone is accepted automatically after 14 days. Each milestone has the free revisions the quote states (2 by default); more changes need an additional quote.",
    ),
    (
        "Quyền sở hữu mã nguồn",
        "Quyền sở hữu hoặc quyền sử dụng mã nguồn theo đúng nội dung ghi trong báo giá bạn đã chấp nhận. Mã nguồn, spec, tài liệu và bản build chỉ được tải về khi đơn hoàn tất và đã thanh toán đủ; trước đó bạn chỉ chạy thử bản build qua F-Launcher. Thư viện bên thứ ba giữ giấy phép của chúng.",
        "Source code ownership",
        "Ownership of, or the right to use, the source code is exactly as stated in the quote you accepted. The source, spec, documents and builds can be downloaded only once the order is completed and fully paid; before that you can only run the builds through F-Launcher. Third-party libraries keep their own licenses.",
    ),
    (
        "Hủy đơn và hoàn tiền",
        "Bạn hủy được đơn ở mọi trạng thái trước Hoàn tất. Hủy trước khi đặt cọc không mất phí. Hủy sau khi đặt cọc: bạn được hoàn các khoản đã trả cho mốc chưa bàn giao; tiền đặt cọc không được hoàn khi công việc đã bắt đầu.",
        "Cancellation and refunds",
        "You can cancel an order in any state before Completed. Cancelling before the deposit costs nothing. After the deposit, you get back what you paid for milestones not yet delivered; the deposit is not refunded once work has started.",
    ),
    (
        "Tranh chấp",
        "Bạn mở được tranh chấp cho một mốc trong 14 ngày sau khi bàn giao. Quản trị viên F-Eco phản hồi trong 2 ngày làm việc, căn cứ vào báo giá, spec và tin nhắn trong đơn. Kết quả là một trong: tiếp tục sửa, hoàn tiền một phần, hoặc hoàn toàn bộ tiền của mốc.",
        "Disputes",
        "You can open a dispute about a milestone within 14 days of its delivery. An F-Eco administrator answers within 2 business days, based on the quote, the spec and the order's messages. The outcome is one of: continue fixing, a partial refund, or a full refund of the milestone.",
    ),
];

#[derive(Template)]
#[template(path = "custom_build_terms.html")]
struct TermsPage {
    i18n: Lang,
    version: &'static str,
    terms: Vec<TermView>,
}

/// GET /policy/custom-build (công khai)
pub async fn terms(lang: Lang) -> Response {
    let terms = TERMS
        .iter()
        .map(|(tv, bv, te, be)| match lang {
            Lang::Vi => TermView {
                title: tv,
                body: bv,
            },
            Lang::En => TermView {
                title: te,
                body: be,
            },
        })
        .collect();
    html(
        TermsPage {
            i18n: lang,
            version: orders::TOS_VERSION,
            terms,
        },
        lang,
        StatusCode::OK,
        None,
    )
}
