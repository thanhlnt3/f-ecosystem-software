//! Công cụ quản trị danh mục và kiểm duyệt tạm thời, cho tới khi có bảng quản trị (nhóm 14). Đọc cấu hình giống `feco-server` (`FECO_ENV`,
//! `FECO_DATABASE_URL`...).

use std::process::ExitCode;

use feco_config::Config;
use feco_server::{catalog, review, store, store_web, AppState};

const USAGE: &str = "\
feco-catalog: quản trị danh mục F-Store (tạm thời)

  feco-catalog add-app <app.json>                 thêm hoặc cập nhật app (xem docs/catalog-app.example.json)
  feco-catalog release <app> <version> <platform> <size-bytes> <tuf-target> [changelog]
  feco-catalog remove <app> <lý do>               gỡ app khỏi danh mục (không đụng tới TUF)
  feco-catalog reports                            hàng chờ báo cáo vi phạm
  feco-catalog resolve <report-id> upheld|rejected <ghi chú>

Kiểm duyệt (người duyệt ghi tên bằng FECO_REVIEWER, mặc định admin):
  feco-catalog scan                               chạy hàng đợi kiểm tra tự động, xếp lịch quét lại 7 ngày
  feco-catalog queue                              hàng chờ duyệt thủ công
  feco-catalog approve <app> <version> [ghi chú]
  feco-catalog reject <app> <version> <điều khoản> <ghi chú>   điều khoản: malware, undeclared_data,
                                                  hidden_mining, drm_copyright, brand_impersonation,
                                                  illegal_content, misleading, technical
  feco-catalog appeals                            kháng nghị đang mở
  feco-catalog appeal <app> <version> uphold|overturn <ghi chú>
  feco-catalog emergency <app> <version|all> <lý do>   thu hồi khẩn cấp (danh mục, TUF, email)
  feco-catalog alerts                             cảnh báo kiểm duyệt chưa xử lý
  feco-catalog add-signature <sha256> <tên>       thêm vào cơ sở dữ liệu mã độc

Quyền sử dụng (tạm, tới nhóm 12-13):
  feco-catalog grant <app> <email> owner|license|subscription <nguồn>
  feco-catalog ungrant <app> <email> owner|license|subscription
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
    let reviewer = std::env::var("FECO_REVIEWER").unwrap_or_else(|_| "admin".into());
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
        "scan" => {
            let queued = review::enqueue_rescans(&state.db).await.map_err(e)?;
            let done = review::process_due(&state).await.map_err(e)?;
            println!("đã xếp lịch quét lại {queued} phiên bản; đã chạy {done} job");
        }
        "queue" => {
            for q in review::manual_queue(&state.db).await.map_err(e)? {
                println!("{} {} — {}", q.app_id, q.version, q.reason);
            }
        }
        "approve" | "reject" => {
            let (app, version) = (arg(&args, 1)?, arg(&args, 2)?);
            let id = review::version_id(&state.db, app, version)
                .await
                .map_err(e)?
                .ok_or("không có phiên bản này")?;
            if command == "approve" {
                let note = args.get(3).cloned().unwrap_or_default();
                review::approve(&state, id, &reviewer, &note)
                    .await
                    .map_err(|e| e.to_string())?;
                println!("đã duyệt {app} {version}");
            } else {
                review::reject(&state, id, &reviewer, arg(&args, 3)?, arg(&args, 4)?)
                    .await
                    .map_err(|e| e.to_string())?;
                println!("đã từ chối {app} {version} và gửi email cho nhà phát triển");
            }
        }
        "appeals" => {
            for a in review::open_appeals(&state.db).await.map_err(e)? {
                println!(
                    "{} {} (điều khoản {}) — {}",
                    a.app_id,
                    a.version,
                    a.reject_code.unwrap_or_default(),
                    a.message
                );
            }
        }
        "appeal" => {
            let (app, version) = (arg(&args, 1)?, arg(&args, 2)?);
            let id = review::version_id(&state.db, app, version)
                .await
                .map_err(e)?
                .ok_or("không có phiên bản này")?;
            let overturn = match arg(&args, 3)? {
                "overturn" => true,
                "uphold" => false,
                other => {
                    return Err(format!(
                        "kết quả phải là uphold hoặc overturn, không phải {other}"
                    ))
                }
            };
            review::resolve_appeal(&state, id, &reviewer, overturn, arg(&args, 4)?)
                .await
                .map_err(|e| e.to_string())?;
            println!("đã xử lý kháng nghị và gửi email cho nhà phát triển");
        }
        "emergency" => {
            let version = Some(arg(&args, 2)?).filter(|v| *v != "all");
            let n = review::revoke(&state, arg(&args, 1)?, version, &reviewer, arg(&args, 3)?)
                .await
                .map_err(|e| e.to_string())?;
            println!("đã thu hồi {n} phiên bản và báo cho nhà phát triển");
        }
        "alerts" => {
            for a in review::open_alerts(&state.db).await.map_err(e)? {
                println!(
                    "#{} {} [{}] {}\n  {}",
                    a.id, a.created_at, a.kind, a.message, a.details
                );
            }
        }
        "grant" | "ungrant" => {
            let app = arg(&args, 1)?;
            let client_id: String = sqlx::query_scalar(
                "SELECT client_id FROM catalog_apps WHERE id = $1
                 UNION SELECT client_id FROM developer_apps WHERE app_id = $1 LIMIT 1",
            )
            .bind(app)
            .fetch_optional(&state.db)
            .await
            .map_err(e)?
            .ok_or("không có app này")?;
            let email = arg(&args, 2)?;
            let subject = state
                .kratos
                .list_identities()
                .await
                .map_err(|e| e.to_string())?
                .into_iter()
                .find(|i| i["traits"]["email"] == email)
                .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
                .ok_or("không có người dùng với email này")?;
            let kind = arg(&args, 3)?;
            let done = if command == "grant" {
                feco_server::entitlements::grant(
                    &state,
                    &client_id,
                    subject,
                    kind,
                    arg(&args, 4)?,
                    None,
                )
                .await
                .map_err(e)?
            } else {
                feco_server::entitlements::revoke(&state, &client_id, subject, kind)
                    .await
                    .map_err(e)?
            };
            println!(
                "{}",
                if done {
                    "đã cập nhật quyền và báo cho app (entitlement.changed)"
                } else {
                    "không có thay đổi"
                }
            );
        }
        "add-signature" => {
            sqlx::query("INSERT INTO malware_signatures (sha256, name) VALUES ($1, $2) ON CONFLICT DO NOTHING")
                .bind(arg(&args, 1)?.to_ascii_lowercase())
                .bind(arg(&args, 2)?)
                .execute(&state.db)
                .await
                .map_err(e)?;
            println!("đã thêm chữ ký mã độc");
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
