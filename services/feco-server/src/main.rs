use std::{process::ExitCode, sync::Arc, time::Duration};

use feco_config::Config;
use feco_server::{account, router, store, webhooks, AppState};
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
    let state = match AppState::from_config(&config) {
        Ok(s) => Arc::new(s),
        Err(e) => {
            eprintln!("Cấu hình không hợp lệ, dịch vụ không khởi động: {e}");
            return ExitCode::from(78);
        }
    };
    if let Err(e) = store::MIGRATOR.run(&state.db).await {
        eprintln!("Không cập nhật được lược đồ cơ sở dữ liệu: {e}");
        return ExitCode::FAILURE;
    }

    let listener = match tokio::net::TcpListener::bind(config.http_addr).await {
        Ok(l) => l,
        Err(e) => {
            eprintln!("Không mở được cổng {}: {e}", config.http_addr);
            return ExitCode::FAILURE;
        }
    };
    tracing::info!(environment = %config.environment, addr = %config.http_addr, "feco-server đang chạy");

    spawn_background_jobs(state.clone());
    let shutdown = async {
        let _ = tokio::signal::ctrl_c().await;
        tracing::info!("nhận tín hiệu dừng");
    };
    if let Err(e) = axum::serve(
        listener,
        router(state).into_make_service_with_connect_info::<std::net::SocketAddr>(),
    )
    .with_graceful_shutdown(shutdown)
    .await
    {
        eprintln!("Lỗi máy chủ: {e}");
        return ExitCode::FAILURE;
    }
    ExitCode::SUCCESS
}

fn spawn_background_jobs(state: Arc<AppState>) {
    // Gửi webhook cho app con mỗi 10 giây (spec app-sdk, "Sự kiện cho app").
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        loop {
            tick.tick().await;
            if let Err(e) = webhooks::deliver_due(&s.db, &s.http, &s.data_key).await {
                tracing::error!(error = %e, "không gửi được webhook");
            }
        }
    });
    // Mỗi giờ: xóa tài khoản đã chờ đủ 30 ngày, nhật ký truy cập quá 90 ngày, file xuất
    // dữ liệu đã hết hạn.
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(3600));
        loop {
            tick.tick().await;
            match account::purge_due(&state, std::time::SystemTime::now()).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(deleted = n, "đã xóa tài khoản hết hạn chờ"),
                Err(e) => tracing::error!(error = %e, "không dọn được tài khoản chờ xóa"),
            }
            if let Err(e) = store::purge_old_access_log(&state.db).await {
                tracing::error!(error = %e, "không dọn được nhật ký truy cập cũ");
            }
            if let Err(e) = store::purge_expired_exports(&state.db).await {
                tracing::error!(error = %e, "không dọn được file xuất hết hạn");
            }
            if let Err(e) = feco_server::audit::purge_older_than_one_year(&state.db).await {
                tracing::error!(error = %e, "không dọn được nhật ký kiểm toán quá 1 năm");
            }
            if let Err(e) = feco_server::launch::purge_expired(&state.db).await {
                tracing::error!(error = %e, "không dọn được mã khởi chạy hết hạn");
            }
        }
    });
}
