//! Trang trạng thái công khai (spec platform-support, "Độ sẵn sàng"; tasks.md 15.3).
//!
//! - Mỗi phút máy chủ kiểm tra cơ sở dữ liệu, dịch vụ tài khoản (Kratos) và dịch vụ đăng nhập cho
//!   app (Hydra, cũng là nền của `GET /v1/me`), ghi vào `status_checks`.
//! - Hai lần lỗi liên tiếp mở một sự cố; lần kiểm tra tốt đầu tiên sau đó đóng sự cố. Như vậy sự cố
//!   hiện trên trang trong khoảng 2 phút (spec yêu cầu trong vòng 10 phút).
//! - `/status` (HTML) và `/status.json` (cho công cụ giám sát) hiện tình trạng từng thành phần, tỉ
//!   lệ sẵn sàng 24 giờ và 30 ngày, các sự cố trong 30 ngày.
//!
//! Trang chạy trong chính feco-server nên không hiện được khi máy chủ hay cơ sở dữ liệu ngừng hẳn;
//! production cần thêm giám sát từ bên ngoài (docs/ops/status-and-backup.md).

use std::sync::Arc;
use std::time::Instant;

use askama::Template;
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse, Json, Response},
};
use serde::Serialize;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::{i18n::Lang, store::Db, AppState};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Component {
    Database,
    Accounts,
    Oauth,
}

impl Component {
    pub const ALL: [Component; 3] = [Component::Accounts, Component::Oauth, Component::Database];

    pub fn as_str(self) -> &'static str {
        match self {
            Component::Database => "database",
            Component::Accounts => "accounts",
            Component::Oauth => "oauth",
        }
    }

    fn label(self, lang: Lang) -> &'static str {
        lang.t(match self {
            Component::Database => "status.component.database",
            Component::Accounts => "status.component.accounts",
            Component::Oauth => "status.component.oauth",
        })
    }
}

/// Số lần lỗi liên tiếp thì mở sự cố.
pub const FAILURES_TO_OPEN: i64 = 2;

/// Kiểm tra từng thành phần một lần: (thành phần, đạt, thời gian ms, chi tiết lỗi).
pub async fn probe(state: &AppState) -> Vec<(Component, bool, i32, String)> {
    let mut out = Vec::new();
    let t = Instant::now();
    let db = sqlx::query_scalar::<_, i32>("SELECT 1")
        .fetch_one(&state.db)
        .await;
    out.push((
        Component::Database,
        db.is_ok(),
        ms(t),
        db.err().map(|e| e.to_string()).unwrap_or_default(),
    ));
    let t = Instant::now();
    let k = state.kratos.alive().await;
    out.push((
        Component::Accounts,
        k.is_ok(),
        ms(t),
        k.err().unwrap_or_default(),
    ));
    let t = Instant::now();
    let h = state.hydra.alive().await;
    out.push((
        Component::Oauth,
        h.is_ok(),
        ms(t),
        h.err().unwrap_or_default(),
    ));
    out
}

fn ms(t: Instant) -> i32 {
    i32::try_from(t.elapsed().as_millis()).unwrap_or(i32::MAX)
}

/// Ghi một lần kiểm tra; mở hoặc đóng sự cố khi cần.
pub async fn record(
    db: &Db,
    component: Component,
    ok: bool,
    latency_ms: i32,
    detail: &str,
    now: OffsetDateTime,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO status_checks (component, ok, latency_ms, checked_at) VALUES ($1, $2, $3, $4)",
    )
    .bind(component.as_str())
    .bind(ok)
    .bind(latency_ms)
    .bind(now)
    .execute(db)
    .await?;
    if ok {
        sqlx::query(
            "UPDATE status_incidents SET resolved_at = $2 WHERE component = $1 AND resolved_at IS NULL",
        )
        .bind(component.as_str())
        .bind(now)
        .execute(db)
        .await?;
        return Ok(());
    }
    let recent: Vec<bool> = sqlx::query_scalar(
        "SELECT ok FROM status_checks WHERE component = $1 ORDER BY checked_at DESC LIMIT $2",
    )
    .bind(component.as_str())
    .bind(FAILURES_TO_OPEN)
    .fetch_all(db)
    .await?;
    if recent.len() as i64 == FAILURES_TO_OPEN && recent.iter().all(|ok| !ok) {
        sqlx::query(
            "INSERT INTO status_incidents (id, component, started_at, detail) VALUES ($1, $2, $3, $4)
             ON CONFLICT (component) WHERE resolved_at IS NULL DO NOTHING",
        )
        .bind(Uuid::new_v4())
        .bind(component.as_str())
        .bind(now)
        .bind(detail.chars().take(500).collect::<String>())
        .execute(db)
        .await?;
    }
    Ok(())
}

/// Một vòng kiểm tra (gọi mỗi phút từ feco-server).
pub async fn run_once(state: &AppState) {
    let now = OffsetDateTime::now_utc();
    for (c, ok, latency, detail) in probe(state).await {
        if !ok {
            tracing::warn!(component = c.as_str(), detail, "thành phần không trả lời");
        }
        if let Err(e) = record(&state.db, c, ok, latency, &detail, now).await {
            tracing::error!(error = %e, "không ghi được kết quả kiểm tra trạng thái");
        }
    }
}

/// Xóa kết quả kiểm tra quá 90 ngày.
pub async fn prune(db: &Db, now: OffsetDateTime) -> sqlx::Result<u64> {
    Ok(
        sqlx::query("DELETE FROM status_checks WHERE checked_at < $1")
            .bind(now - Duration::days(90))
            .execute(db)
            .await?
            .rows_affected(),
    )
}

/// Tỉ lệ sẵn sàng (phần trăm, 2 chữ số thập phân); `None` khi chưa có số liệu.
pub fn uptime(ok: i64, total: i64) -> Option<f64> {
    (total > 0).then(|| (ok as f64 * 10000.0 / total as f64).floor() / 100.0)
}

fn pct(v: Option<f64>) -> String {
    v.map_or_else(|| "—".into(), |p| format!("{p:.2}%"))
}

#[derive(Serialize)]
pub struct ComponentStatus {
    pub component: Component,
    #[serde(skip)]
    pub label: &'static str,
    /// `operational`, `outage` hoặc `unknown`.
    pub state: &'static str,
    #[serde(skip)]
    pub state_label: &'static str,
    pub uptime_24h: Option<f64>,
    pub uptime_30d: Option<f64>,
    #[serde(skip)]
    pub uptime_24h_text: String,
    #[serde(skip)]
    pub uptime_30d_text: String,
    #[serde(skip)]
    pub checked: String,
}

#[derive(Serialize)]
pub struct IncidentView {
    pub component: String,
    #[serde(skip)]
    pub label: &'static str,
    pub started_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Serialize)]
pub struct Overview {
    /// `operational` khi mọi thành phần bình thường.
    pub state: &'static str,
    pub components: Vec<ComponentStatus>,
    pub incidents: Vec<IncidentView>,
}

fn when(t: OffsetDateTime) -> String {
    t.to_offset(time::macros::offset!(+7))
        .format(&time::macros::format_description!(
            "[day]/[month]/[year] [hour]:[minute] (UTC+7)"
        ))
        .unwrap_or_default()
}

pub async fn overview(db: &Db, lang: Lang, now: OffsetDateTime) -> sqlx::Result<Overview> {
    let mut components = Vec::new();
    let open: Vec<String> =
        sqlx::query_scalar("SELECT component FROM status_incidents WHERE resolved_at IS NULL")
            .fetch_all(db)
            .await?;
    for c in Component::ALL {
        let counts = |since: OffsetDateTime| {
            sqlx::query_as::<_, (i64, i64)>(
                "SELECT count(*) FILTER (WHERE ok), count(*) FROM status_checks
                 WHERE component = $1 AND checked_at >= $2",
            )
            .bind(c.as_str())
            .bind(since)
            .fetch_one(db)
        };
        let (ok24, n24) = counts(now - Duration::hours(24)).await?;
        let (ok30, n30) = counts(now - Duration::days(30)).await?;
        let last: Option<(bool, OffsetDateTime)> = sqlx::query_as(
            "SELECT ok, checked_at FROM status_checks WHERE component = $1 ORDER BY checked_at DESC LIMIT 1",
        )
        .bind(c.as_str())
        .fetch_optional(db)
        .await?;
        let state = if open.iter().any(|o| o == c.as_str()) {
            "outage"
        } else if last.is_some() {
            "operational"
        } else {
            "unknown"
        };
        components.push(ComponentStatus {
            component: c,
            label: c.label(lang),
            state,
            state_label: lang.t(match state {
                "outage" => "status.down",
                "operational" => "status.ok",
                _ => "status.no_data",
            }),
            uptime_24h: uptime(ok24, n24),
            uptime_30d: uptime(ok30, n30),
            uptime_24h_text: pct(uptime(ok24, n24)),
            uptime_30d_text: pct(uptime(ok30, n30)),
            checked: last.map(|(_, t)| when(t)).unwrap_or_default(),
        });
    }
    let rows: Vec<(String, OffsetDateTime, Option<OffsetDateTime>)> = sqlx::query_as(
        "SELECT component, started_at, resolved_at FROM status_incidents
         WHERE started_at >= $1 OR resolved_at IS NULL ORDER BY started_at DESC LIMIT 50",
    )
    .bind(now - Duration::days(30))
    .fetch_all(db)
    .await?;
    let incidents = rows
        .into_iter()
        .map(|(component, started, resolved)| IncidentView {
            label: Component::ALL
                .into_iter()
                .find(|c| c.as_str() == component)
                .map_or("?", |c| c.label(lang)),
            component,
            started_at: when(started),
            resolved_at: resolved.map(when),
        })
        .collect();
    let state = if components.iter().any(|c| c.state == "outage") {
        "outage"
    } else {
        "operational"
    };
    Ok(Overview {
        state,
        components,
        incidents,
    })
}

#[derive(Template)]
#[template(path = "status.html")]
struct StatusPage {
    i18n: Lang,
    overview: Overview,
}

/// GET /status
pub async fn page(State(state): State<Arc<AppState>>, lang: Lang) -> Response {
    match overview(&state.db, lang, OffsetDateTime::now_utc()).await {
        Ok(overview) => match (StatusPage {
            i18n: lang,
            overview,
        })
        .render()
        {
            Ok(h) => Html(h).into_response(),
            Err(e) => {
                tracing::error!(error = %e, "lỗi dựng trang");
                (StatusCode::INTERNAL_SERVER_ERROR, lang.t("error.render")).into_response()
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "không đọc được trạng thái");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                lang.t("status.unavailable"),
            )
                .into_response()
        }
    }
}

/// GET /status.json
pub async fn json(State(state): State<Arc<AppState>>) -> Response {
    match overview(&state.db, Lang::En, OffsetDateTime::now_utc()).await {
        Ok(o) => Json(o).into_response(),
        Err(e) => {
            tracing::error!(error = %e, "không đọc được trạng thái");
            (
                StatusCode::SERVICE_UNAVAILABLE,
                Json(serde_json::json!({"state": "unknown"})),
            )
                .into_response()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uptime_math() {
        assert_eq!(uptime(0, 0), None);
        assert_eq!(uptime(999, 1000), Some(99.9));
        assert_eq!(uptime(2, 3), Some(66.66));
        assert_eq!(pct(Some(100.0)), "100.00%");
        assert_eq!(pct(None), "—");
        assert_eq!(
            when(time::macros::datetime!(2026-11-15 01:00 UTC)),
            "15/11/2026 08:00 (UTC+7)"
        );
    }
}
