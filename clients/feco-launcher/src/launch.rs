//! Mở app con (spec launcher, "Mở app con và truyền phiên đăng nhập"; design D4).
//!
//! App nhận qua biến môi trường:
//! - `FECO_LAUNCH_CODE`: mã dùng một lần, sống 60 giây, đổi lấy token tại
//!   `POST {FECO_SERVER_URL}v1/launch-codes/exchange`.
//! - `FECO_CLIENT_ID`, `FECO_SERVER_URL`, `FECO_ISSUER_URL`, `FECO_APP_DATA_DIR` (F-SDK đọc các
//!   biến này).
//! - `FECO_LAUNCHER_URL`, `FECO_LAUNCHER_SECRET`: API cục bộ để xin mã mới (xem [`crate::broker`]).
//!
//! Mọi biến `FECO_*` khác của launcher bị gỡ khỏi môi trường của app.

use crate::install::LaunchPlan;
use std::process::Command;

#[derive(Clone, Debug, Default)]
pub struct LaunchEnv {
    pub launch_code: Option<String>,
    pub server_url: String,
    /// Máy chủ OAuth 2.0 / OIDC (`FECO_ISSUER_URL`).
    pub issuer_url: String,
    pub broker_url: Option<String>,
    pub broker_secret: Option<String>,
}

pub fn command(plan: &LaunchPlan, env: &LaunchEnv) -> Command {
    let mut cmd = Command::new(&plan.program);
    cmd.current_dir(&plan.working_dir);
    for (key, _) in std::env::vars_os() {
        if key.to_string_lossy().starts_with("FECO_") {
            cmd.env_remove(key);
        }
    }
    cmd.env("FECO_APP_DATA_DIR", &plan.data_dir);
    cmd.env("FECO_SERVER_URL", &env.server_url);
    cmd.env("FECO_ISSUER_URL", &env.issuer_url);
    if let Some(client_id) = &plan.client_id {
        cmd.env("FECO_CLIENT_ID", client_id);
    }
    if let Some(code) = &env.launch_code {
        cmd.env("FECO_LAUNCH_CODE", code);
    }
    if let (Some(url), Some(secret)) = (&env.broker_url, &env.broker_secret) {
        cmd.env("FECO_LAUNCHER_URL", url);
        cmd.env("FECO_LAUNCHER_SECRET", secret);
    }
    cmd
}
