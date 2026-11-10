//! Công cụ quản trị danh mục tạm thời, cho tới khi có cổng nhà phát triển (nhóm 8), kiểm duyệt
//! (nhóm 9) và bảng quản trị (nhóm 14). Đọc cấu hình giống `feco-server` (`FECO_ENV`,
//! `FECO_DATABASE_URL`...).

use std::process::ExitCode;

use feco_config::Config;
use feco_server::{catalog, store, store_web, AppState};

const USAGE: &str = "\
feco-catalog: quản trị danh mục F-Store (tạm thời)

  feco-catalog add-app <app.json>                 thêm hoặc cập nhật app (xem docs/catalog-app.example.json)
  feco-catalog release <app> <version> <platform> <size-bytes> <tuf-target> [changelog]
  feco-catalog revoke <app> <version>             thu hồi một phiên bản khỏi danh mục
  feco-catalog remove <app> <lý do>               gỡ app khỏi danh mục
  feco-catalog review <app> <version> approve|reject [ghi chú]   kết quả kiểm duyệt (tạm, tới nhóm 9)
  feco-catalog reports                            hàng chờ báo cáo vi phạm
  feco-catalog resolve <report-id> upheld|rejected <ghi chú>
";

#[tokio::main]
async fn main() -> ExitCode {
    match run(std::env::args().skip(1).collect()).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("lỗi: {e}");
            ExitCode::FAILURE
        }
    }
}

fn arg(args: &[String], i: usize) -> Result<&str, String> {
    args.get(i)
        .map(String::as_str)
        .ok_or_else(|| format!("thiếu tham số\n\n{USAGE}"))
}

async fn run(args: Vec<String>) -> Result<(), String> {
    let e = |e: sqlx::Error| e.to_string();
    let command = arg(&args, 0)?.to_owned();
    if command == "help" || command == "--help" {
        print!("{USAGE}");
        return Ok(());
    }
    let config = Config::from_env().map_err(|e| e.to_string())?;
    let state = AppState::from_config(&config).map_err(|e| e.to_string())?;
    store::MIGRATOR
        .run(&state.db)
        .await
        .map_err(|e| e.to_string())?;
    match command.as_str() {
        "add-app" => {
            let json = std::fs::read_to_string(arg(&args, 1)?).map_err(|e| e.to_string())?;
            let input: catalog::AppInput =
                serde_json::from_str(&json).map_err(|e| format!("app.json không hợp lệ: {e}"))?;
            catalog::upsert_app(&state.db, &state.catalog, &input)
                .await
                .map_err(e)?;
            println!("đã lưu app {}", input.id);
        }
        "release" => {
            let app = arg(&args, 1)?;
            let v = catalog::VersionInput {
                version: arg(&args, 2)?.to_owned(),
                platform: arg(&args, 3)?.to_owned(),
                size_bytes: arg(&args, 4)?
                    .parse()
                    .map_err(|_| "size-bytes phải là số")?,
                tuf_target: arg(&args, 5)?.to_owned(),
                changelog: args.get(6).cloned().unwrap_or_default(),
            };
            catalog::release_version(&state.db, &state.catalog, app, &v)
                .await
                .map_err(e)?;
            println!("đã phát hành {app} {} ({})", v.version, v.platform);
        }
        "revoke" => {
            let n =
                catalog::revoke_version(&state.db, &state.catalog, arg(&args, 1)?, arg(&args, 2)?)
                    .await
                    .map_err(e)?;
            println!("đã thu hồi {n} bản ghi phiên bản");
        }
        "remove" => {
            let removed =
                catalog::remove_app(&state.db, &state.catalog, arg(&args, 1)?, arg(&args, 2)?)
                    .await
                    .map_err(e)?;
            println!(
                "{}",
                if removed {
                    "đã gỡ app khỏi danh mục"
                } else {
                    "app không có hoặc đã bị gỡ"
                }
            );
        }
        "review" => {
            let approve = match arg(&args, 3)? {
                "approve" => true,
                "reject" => false,
                other => {
                    return Err(format!(
                        "kết quả phải là approve hoặc reject, không phải {other}"
                    ))
                }
            };
            let note = args.get(4).cloned().unwrap_or_default();
            let done = feco_server::developer::review(
                &state.db,
                arg(&args, 1)?,
                arg(&args, 2)?,
                approve,
                &note,
            )
            .await
            .map_err(|e| e.to_string())?;
            println!(
                "{}",
                if done {
                    "đã ghi kết quả kiểm duyệt"
                } else {
                    "không có phiên bản đang chờ duyệt"
                }
            );
        }
        "reports" => {
            for r in catalog::open_reports(&state.db).await.map_err(e)? {
                println!(
                    "{} {} app={} review={} lý do={} — {}",
                    r.id,
                    r.created_at,
                    r.app_id,
                    r.review_id
                        .map(|x| x.to_string())
                        .unwrap_or_else(|| "-".into()),
                    r.reason,
                    r.details
                );
            }
        }
        "resolve" => {
            let id = arg(&args, 1)?
                .parse()
                .map_err(|_| "report-id không hợp lệ")?;
            let upheld = match arg(&args, 2)? {
                "upheld" => true,
                "rejected" => false,
                other => {
                    return Err(format!(
                        "kết quả phải là upheld hoặc rejected, không phải {other}"
                    ))
                }
            };
            let note = args.get(3).cloned().unwrap_or_default();
            if store_web::resolve_and_notify(&state, id, upheld, &note).await? {
                println!("đã xử lý báo cáo và gửi email cho người báo cáo");
            } else {
                println!("không có báo cáo đang mở với id này");
            }
        }
        other => return Err(format!("lệnh không hợp lệ: {other}\n\n{USAGE}")),
    }
    Ok(())
}
