use std::process::ExitCode;

use feco_config::Config;
use feco_server::{router, AppState};
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> ExitCode {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    // Kiểm tra cấu hình trước khi mở cổng: cấu hình sai thì không chạy (tasks.md 1.5).
    let config = match Config::from_env() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("Cấu hình không hợp lệ, dịch vụ không khởi động: {e}");
            return ExitCode::from(78); // EX_CONFIG
        }
    };

    let listener = match tokio::net::TcpListener::bind(config.http_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Không mở được cổng {}: {e}", config.http_addr);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(environment = %config.environment, addr = %config.http_addr, "feco-server đang chạy");

    let state = match AppState::from_config(&config) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Cấu hình không hợp lệ, dịch vụ không khởi động: {e}");
            return ExitCode::from(78);
        }
    };
    // Xóa hẳn các tài khoản đã chờ đủ 30 ngày, mỗi giờ một lần (spec account-identity,
    // "Xóa tài khoản").
    let purge_kratos = state.kratos.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(std::time::Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match feco_server::account::purge_due(&purge_kratos, std::time::SystemTime::now()).await
            {
                Ok(0) => {}
                Ok(n) => tracing::info!(deleted = n, "đã xóa tài khoản hết hạn chờ"),
                Err(e) => tracing::error!(error = %e, "không dọn được tài khoản chờ xóa"),
            }
        }
    });
    let app = router(state);
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("nhận tín hiệu dừng");
    };
    if let Err(e) = axum::serve(
        listener,
        app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    {
        eprintln!("Lỗi máy chủ: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}
