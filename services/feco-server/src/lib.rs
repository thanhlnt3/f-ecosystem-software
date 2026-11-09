//! Dịch vụ HTTP của F-Eco. Hiện có endpoint kiểm tra sức khỏe; các API
//! (tài khoản, danh mục, đơn hàng, thanh toán) được thêm theo từng nhóm task.

use std::sync::Arc;

use axum::{extract::State, routing::get, Json, Router};
use feco_config::Config;
use feco_core::Environment;
use serde::Serialize;

#[derive(Clone)]
pub struct AppState {
    pub environment: Environment,
}

impl AppState {
    pub fn from_config(config: &Config) -> Self {
        AppState {
            environment: config.environment,
        }
    }
}

#[derive(Debug, Serialize, PartialEq, Eq)]
pub struct Health {
    pub status: &'static str,
    pub environment: Environment,
    pub version: &'static str,
}

async fn healthz(State(state): State<Arc<AppState>>) -> Json<Health> {
    Json(Health {
        status: "ok",
        environment: state.environment,
        version: env!("CARGO_PKG_VERSION"),
    })
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .with_state(Arc::new(state))
}

#[cfg(test)]
mod tests {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    #[tokio::test]
    async fn healthz_reports_environment_and_version() {
        let app = router(AppState {
            environment: Environment::Staging,
        });
        let res = app
            .oneshot(Request::get("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 200);
        let body = res.into_body().collect().await.unwrap().to_bytes();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["status"], "ok");
        assert_eq!(json["environment"], "staging");
        assert_eq!(json["version"], env!("CARGO_PKG_VERSION"));
    }

    #[tokio::test]
    async fn unknown_path_is_404() {
        let app = router(AppState {
            environment: Environment::Dev,
        });
        let res = app
            .oneshot(Request::get("/khong-co").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), 404);
    }
}
