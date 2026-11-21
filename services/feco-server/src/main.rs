use std::{process::ExitCode, sync::Arc, time::Duration};

use feco_config::Config;
use feco_server::{account, bootstrap, router, store, webhooks, AppState};
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
    // Tài khoản quản trị viên mặc định (production-readiness): tệp mật khẩu phải đọc được (dev tự
    // sinh); thiếu thì không khởi động.
    let admin_cfg = config.admin_bootstrap.clone();
    if let Err(e) = bootstrap::load_password(&admin_cfg) {
        eprintln!("Tài khoản quản trị viên mặc định: {e}");
        return ExitCode::from(78);
    }
    if let Err(e) = bootstrap::ensure_admin(&state, &admin_cfg).await {
        tracing::error!(error = %e, "chưa bảo đảm được tài khoản quản trị viên mặc định, thử lại sau 10 phút");
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
    {
        let state = state.clone();
        tokio::spawn(async move {
            let mut tick = tokio::time::interval(Duration::from_secs(600));
            tick.tick().await;
            loop {
                tick.tick().await;
                if let Err(e) = bootstrap::ensure_admin(&state, &admin_cfg).await {
                    tracing::error!(error = %e, "không bảo đảm được tài khoản quản trị viên mặc định");
                }
            }
        });
    }
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
    // Trang trạng thái: kiểm tra các thành phần mỗi phút, dọn số liệu cũ mỗi ngày (tasks.md 15.3).
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        let mut n: u64 = 0;
        loop {
            tick.tick().await;
            feco_server::status::run_once(&s).await;
            if n % (24 * 60) == 0 {
                if let Err(e) =
                    feco_server::status::prune(&s.db, time::OffsetDateTime::now_utc()).await
                {
                    tracing::error!(error = %e, "không dọn được số liệu trạng thái");
                }
            }
            n += 1;
        }
    });
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
    // Kiểm duyệt: chạy hàng đợi kiểm tra tự động mỗi 10 giây, xếp lịch quét lại mỗi giờ
    // (spec app-review, "Kiểm tra tự động", "Kiểm tra lại định kỳ").
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(10));
        let mut last_rescan = std::time::Instant::now() - Duration::from_secs(3600);
        loop {
            tick.tick().await;
            if last_rescan.elapsed() >= Duration::from_secs(3600) {
                last_rescan = std::time::Instant::now();
                if let Err(e) = feco_server::review::enqueue_rescans(&s.db).await {
                    tracing::error!(error = %e, "không xếp được lịch quét lại");
                }
            }
            if let Err(e) = feco_server::review::process_due(&s).await {
                tracing::error!(error = %e, "không chạy được hàng đợi kiểm duyệt");
            }
        }
    });
    // Chat: nghe `LISTEN feco_chat` để phát sự kiện tin mới; mỗi phút quét lại tệp đính kèm chưa
    // quét xong (thay đổi chat-and-source-scan).
    feco_server::chat::start_listener(&state);
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        loop {
            tick.tick().await;
            if let Err(e) = feco_server::chat::rescan_due(&s).await {
                tracing::error!(error = %e, "không quét lại được tệp đính kèm của chat");
            }
        }
    });
    // Mỗi 10 phút: báo bản cập nhật chưa báo, gồm bản phát hành bằng `feco-catalog release`
    // (thay đổi app-update-notices, design U1).
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(600));
        loop {
            tick.tick().await;
            match feco_server::notifications::announce_updates(&s, None).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(sent = n, "đã báo bản cập nhật app"),
                Err(e) => tracing::error!(error = %e, "không báo được bản cập nhật app"),
            }
        }
    });
    // Thanh toán: mỗi phút truy vấn chủ động giao dịch quá 15 phút và lệnh hoàn đang xử lý; mỗi
    // giờ kiểm xem đã tới lúc đối soát ngày hôm trước chưa.
    let s = state.clone();
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(Duration::from_secs(60));
        let mut n: u64 = 0;
        loop {
            tick.tick().await;
            let now = time::OffsetDateTime::now_utc();
            if let Err(e) = feco_server::payments::process_due(&s, now).await {
                tracing::error!(error = %e, "không truy vấn được giao dịch đang chờ");
            }
            if n % 60 == 0 {
                match feco_server::payments::reconcile_if_due(&s, now).await {
                    Ok(Some(k)) => tracing::info!(mismatches = k, "đã đối soát ngày hôm trước"),
                    Ok(None) => {}
                    Err(e) => tracing::error!(error = %e, "đối soát thất bại"),
                }
            }
            n += 1;
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
            // Đơn xây app: nhắc nghiệm thu sau 7 ngày, tự nghiệm thu sau 14 ngày.
            match feco_server::orders::process_due(&state, time::OffsetDateTime::now_utc()).await {
                Ok(0) => {}
                Ok(n) => tracing::info!(accepted = n, "đã tự nghiệm thu mốc quá 14 ngày"),
                Err(e) => {
                    tracing::error!(error = %e, "không chạy được việc theo thời gian của đơn hàng")
                }
            }
            if let Err(e) = store::purge_old_access_log(&state.db).await {
                tracing::error!(error = %e, "không dọn được nhật ký truy cập cũ");
            }
            if let Err(e) = store::purge_expired_exports(&state.db).await {
                tracing::error!(error = %e, "không dọn được file xuất hết hạn");
            }
            if let Err(e) = feco_server::token_proxy::purge_old(&state.db).await {
                tracing::error!(error = %e, "không dọn được băm refresh token cũ");
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
