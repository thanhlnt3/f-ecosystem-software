//! Công cụ quản trị danh mục và kiểm duyệt tạm thời, cho tới khi có bảng quản trị (nhóm 14). Đọc cấu hình giống `feco-server` (`FECO_ENV`,
//! `FECO_DATABASE_URL`...).

use std::process::ExitCode;

use feco_config::Config;
use feco_server::{catalog, orders, payments, review, store, store_web, AppState};

fn o_err(e: orders::OrderError) -> String {
    e.to_string()
}

async fn subject_by_email(state: &AppState, email: &str) -> Result<uuid::Uuid, String> {
    state
        .kratos
        .list_identities()
        .await
        .map_err(|e| e.to_string())?
        .into_iter()
        .find(|i| i["traits"]["email"] == email)
        .and_then(|i| i["id"].as_str().and_then(|s| s.parse().ok()))
        .ok_or_else(|| "không có người dùng với email này".to_owned())
}

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
  feco-catalog grant <app|client ID> <email> owner|license|subscription <nguồn>
  feco-catalog ungrant <app|client ID> <email> owner|license|subscription

Đơn xây app (đội F-Eco ghi tên bằng FECO_REVIEWER; mã đơn dạng DH-000001):
  feco-catalog orders [trạng thái]                 danh sách đơn (submitted, quoted, in_progress...)
  feco-catalog order <mã đơn>                      chi tiết: mốc, yêu cầu thanh toán, tin nhắn, lịch sử
  feco-catalog order-quote <mã đơn> <quote.json>   gửi báo giá (xem docs/order-quote.example.json)
  feco-catalog order-paid <payment-request-id>     ghi nhận đã trả bằng tay (chỉ dev/staging, khi chưa nối cổng)
  feco-catalog private <app> <email|none>          đặt app thành app riêng tư của một khách hàng
  feco-catalog order-deliver <mã đơn> <mốc> <app> <phiên bản> <kết-quả-kiểm-tra.txt> [ghi chú]
  feco-catalog order-message <mã đơn> <nội dung>
  feco-catalog order-resolve <mã đơn> continue|full|partial <số tiền>   giải quyết tranh chấp
  feco-catalog order-handover <mã đơn> <bàn-giao.zip>   gói bàn giao cuối (mã nguồn, spec, tài liệu, build)
  feco-catalog order-due                           chạy ngay việc theo thời gian (nhắc, tự nghiệm thu)

Email:
  feco-catalog mail-dns <tên miền> <selector DKIM> [include SPF] [email nhận báo cáo DMARC]
                                                  kiểm tra SPF, DKIM, DMARC và gợi ý bản ghi

Bảng quản trị:
  feco-catalog admin-set <email> <vai trò,...|none>   vai trò: reviewer, support, project_manager,
                                                  finance, sysadmin (cần passkey khi đăng nhập /admin)

Thanh toán (quản trị viên ghi tên bằng FECO_REVIEWER):
  feco-catalog payments <mã đơn>                   giao dịch, lệnh hoàn của một đơn
  feco-catalog payment-check <mã giao dịch>        truy vấn chủ động cổng thanh toán
  feco-catalog payment-link <payment-request-id> <cổng:phương thức>   tạo liên kết trả thay khách (chỉ dev/staging)
  feco-catalog refund <transaction-id> <số tiền> <lý do>   hoàn toàn bộ hoặc một phần
  feco-catalog refunds                             lệnh hoàn chờ phê duyệt hoặc đang xử lý
  feco-catalog refund-approve <refund-id>          phê duyệt (từ 10.000.000 ₫ cần hai người khác nhau)
  feco-catalog payments-due                        chạy ngay truy vấn giao dịch quá 15 phút, lệnh hoàn
  feco-catalog reconcile [yyyy-mm-dd]              đối soát một ngày (mặc định hôm qua)
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
            // App chưa có trong danh mục hay cổng (ví dụ app mẫu F-SDK): nhận thẳng client ID.
            .or_else(|| app.parse::<uuid::Uuid>().ok().map(|_| app.to_owned()))
            .ok_or("không có app này (nhập mã app hoặc client ID)")?;
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
        "orders" => {
            let status = match args.get(1) {
                Some(s) => Some(orders::Status::parse(s).ok_or("trạng thái không hợp lệ")?),
                None => None,
            };
            for o in orders::list_by_status(&state.db, status)
                .await
                .map_err(o_err)?
            {
                println!(
                    "{} {:<12} {} — {} ({})",
                    o.code,
                    o.status.as_str(),
                    o.created_at.date(),
                    o.title,
                    o.customer_email
                );
            }
        }
        "order" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            println!(
                "{} {} [{}] khách: {}",
                o.code,
                o.title,
                o.status.as_str(),
                o.customer_email
            );
            println!("{}\n", o.description);
            if let Some(q) = orders::current_quote(&state.db, o.id)
                .await
                .map_err(o_err)?
            {
                println!("Báo giá {} ({}), tổng {}", q.number, q.status, q.total());
                for m in &q.milestones {
                    println!(
                        "  mốc {} {} — {} [{}] trả={} sửa còn {}/{} build={} {}",
                        m.position,
                        m.title,
                        m.amount_vnd,
                        m.status,
                        m.paid,
                        m.revisions_left(),
                        m.free_revisions,
                        m.app_id.as_deref().unwrap_or("-"),
                        m.app_version.as_deref().unwrap_or("")
                    );
                }
            }
            for p in orders::payment_requests(&state.db, o.id)
                .await
                .map_err(o_err)?
            {
                println!(
                    "  thanh toán {} {} {} [{}] {}",
                    p.id, p.kind, p.amount_vnd, p.status, p.reason
                );
            }
            for f in orders::files(&state.db, o.id).await.map_err(o_err)? {
                println!("  tệp {} {} ({} byte)", f.id, f.name, f.size);
            }
            for m in orders::messages(&state.db, o.id).await.map_err(o_err)? {
                println!("  [{}] {} {}: {}", m.at, m.author, m.kind, m.body);
            }
            for ev in orders::history(&state.db, o.id).await.map_err(o_err)? {
                println!(
                    "  {} {} -> {} bởi {} {}",
                    ev.at,
                    ev.from.unwrap_or_default(),
                    ev.to,
                    ev.actor,
                    ev.note
                );
            }
        }
        "order-quote" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            let json = std::fs::read_to_string(arg(&args, 2)?).map_err(|e| e.to_string())?;
            let q: orders::QuoteInput =
                serde_json::from_str(&json).map_err(|e| format!("quote.json không hợp lệ: {e}"))?;
            orders::create_quote(&state, o.id, &reviewer, &q, time::OffsetDateTime::now_utc())
                .await
                .map_err(o_err)?;
            println!("đã gửi báo giá cho {} và email cho khách hàng", o.code);
        }
        "order-paid" => {
            let id = arg(&args, 1)?
                .parse()
                .map_err(|_| "payment-request-id không hợp lệ")?;
            let outcome = payments::record_manual(&state, id, time::OffsetDateTime::now_utc())
                .await
                .map_err(o_err)?;
            println!("đã ghi nhận thanh toán thủ công: {outcome:?}");
        }
        "payments" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            for r in orders::payment_requests(&state.db, o.id)
                .await
                .map_err(o_err)?
            {
                println!(
                    "yêu cầu {} {} {} [{}] {}",
                    r.id, r.kind, r.amount_vnd, r.status, r.reason
                );
                for t in payments::transactions_of_request(&state, r.id)
                    .await
                    .map_err(o_err)?
                {
                    println!(
                        "  giao dịch {} {} lần {} {} {} [{}] hoàn {} {}",
                        t.id,
                        t.gateway_ref,
                        t.attempt,
                        t.method,
                        t.amount_vnd,
                        t.status.as_str(),
                        t.refunded_vnd,
                        t.gateway_trans_id.as_deref().unwrap_or("")
                    );
                }
            }
        }
        "mail-dns" => {
            let (domain, selector) = (arg(&args, 1)?, arg(&args, 2)?);
            let servers = if state.dns_servers.is_empty() {
                vec!["1.1.1.1:53".parse().unwrap(), "8.8.8.8:53".parse().unwrap()]
            } else {
                state.dns_servers.clone()
            };
            let report = feco_server::maildns::check(&servers, domain, selector)
                .await
                .map_err(|e| e.to_string())?;
            println!("SPF:   {}", report.spf.as_deref().unwrap_or("(không có)"));
            println!("DKIM:  {}", report.dkim.as_deref().unwrap_or("(không có)"));
            println!("DMARC: {}", report.dmarc.as_deref().unwrap_or("(không có)"));
            if report.ok() {
                println!("đạt");
            } else {
                for p in &report.problems {
                    println!("- {p}");
                }
                println!(
                    "
Bản ghi gợi ý:"
                );
                let include = args
                    .get(3)
                    .map(String::as_str)
                    .unwrap_or("<include của máy chủ gửi>");
                let rua = args
                    .get(4)
                    .cloned()
                    .unwrap_or_else(|| format!("dmarc@{domain}"));
                for (name, value) in
                    feco_server::maildns::recommended(domain, selector, include, &rua)
                {
                    println!("  {name}  TXT  \"{value}\"");
                }
                return Err("tên miền chưa đủ SPF, DKIM, DMARC".into());
            }
        }
        "admin-set" => {
            let email = arg(&args, 1)?;
            let roles: Vec<feco_server::admin::Role> = match arg(&args, 2)? {
                "none" => Vec::new(),
                list => list
                    .split(',')
                    .map(|r| {
                        feco_server::admin::Role::parse(r.trim())
                            .ok_or(format!("vai trò không hợp lệ: {r}"))
                    })
                    .collect::<Result<_, _>>()?,
            };
            feco_server::admin::set_member(&state, &reviewer, email, &roles).await?;
            println!("đã cập nhật vai trò quản trị của {email}");
        }
        "payment-link" => {
            if state.environment.is_production() {
                return Err("chỉ dùng ngoài production".into());
            }
            let request = arg(&args, 1)?
                .parse()
                .map_err(|_| "payment-request-id không hợp lệ")?;
            let charge = payments::charge(&state, request).await.map_err(o_err)?;
            let link = payments::start(
                &state,
                charge.customer,
                request,
                arg(&args, 2)?,
                "127.0.0.1",
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(o_err)?;
            println!(
                "giao dịch {} ({})
{}",
                link.gateway_ref, link.transaction, link.order_url
            );
        }
        "payment-check" => {
            let status = payments::confirm(
                &state,
                arg(&args, 1)?,
                "query",
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(o_err)?;
            println!("trạng thái: {}", status.as_str());
        }
        "refund" => {
            let tx = arg(&args, 1)?
                .parse()
                .map_err(|_| "transaction-id không hợp lệ")?;
            let amount: i64 = arg(&args, 2)?
                .replace(['.', ','], "")
                .parse()
                .map_err(|_| "số tiền phải là số")?;
            let id = payments::request_refund(
                &state,
                tx,
                amount,
                &args[3..].join(" "),
                payments::Requester::Admin(reviewer.clone()),
                None,
            )
            .await
            .map_err(o_err)?;
            let r = payments::refund(&state, id).await.map_err(o_err)?;
            println!("lệnh hoàn {id}: {}", r.label(true));
        }
        "refunds" => {
            for r in payments::open_refunds(&state).await.map_err(o_err)? {
                println!(
                    "{} giao dịch {} {} [{}] cần {} duyệt, đã duyệt {:?} — {}",
                    r.id,
                    r.transaction_id,
                    r.amount_vnd,
                    r.status,
                    r.approvals_needed(),
                    r.approvals,
                    r.reason
                );
            }
        }
        "refund-approve" => {
            let id = arg(&args, 1)?
                .parse()
                .map_err(|_| "refund-id không hợp lệ")?;
            let r = payments::approve_refund(&state, id, &reviewer)
                .await
                .map_err(o_err)?;
            println!("lệnh hoàn {id}: {}", r.label(true));
        }
        "payments-due" => {
            let r = payments::process_due(&state, time::OffsetDateTime::now_utc())
                .await
                .map_err(o_err)?;
            println!(
                "đã truy vấn {} giao dịch, {} lệnh hoàn",
                r.checked, r.refunds_checked
            );
        }
        "reconcile" => {
            let now = time::OffsetDateTime::now_utc();
            let day = match args.get(1) {
                Some(d) => time::Date::parse(
                    d,
                    &time::macros::format_description!("[year]-[month]-[day]"),
                )
                .map_err(|_| "ngày phải dạng yyyy-mm-dd")?,
                None => now
                    .to_offset(time::macros::offset!(+7))
                    .date()
                    .previous_day()
                    .ok_or("không có ngày hôm trước")?,
            };
            let m = payments::reconcile(&state, day, now).await.map_err(o_err)?;
            println!("đối soát {day}: {} chênh lệch", m.len());
            for x in m {
                println!(
                    "  {} {} F-Eco {} / cổng {}",
                    x.gateway_ref, x.kind, x.local_amount, x.gateway_amount
                );
            }
        }
        "private" => {
            let app = arg(&args, 1)?;
            let owner = match arg(&args, 2)? {
                "none" => None,
                email => Some(subject_by_email(&state, email).await?),
            };
            if catalog::set_private(&state.db, &state.catalog, app, owner)
                .await
                .map_err(e)?
            {
                println!("đã cập nhật");
            } else {
                println!("không có app này trong danh mục");
            }
        }
        "order-deliver" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            let position: i32 = arg(&args, 2)?.parse().map_err(|_| "mốc phải là số")?;
            let d = orders::Delivery {
                app_id: arg(&args, 3)?.to_owned(),
                app_version: arg(&args, 4)?.to_owned(),
                check_report: std::fs::read_to_string(arg(&args, 5)?).map_err(|e| e.to_string())?,
                notes: args.get(6).cloned().unwrap_or_default(),
            };
            orders::deliver(
                &state,
                o.id,
                position,
                &reviewer,
                &d,
                time::OffsetDateTime::now_utc(),
            )
            .await
            .map_err(o_err)?;
            println!("đã bàn giao mốc {position} của {}", o.code);
        }
        "order-message" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            orders::post_message(&state, o.id, Some(&reviewer), &args[2..].join(" "))
                .await
                .map_err(o_err)?;
            println!("đã gửi tin nhắn");
        }
        "order-resolve" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            let r = match arg(&args, 2)? {
                "continue" => orders::Resolution::Continue,
                "full" => orders::Resolution::FullRefund,
                "partial" => orders::Resolution::PartialRefund(
                    arg(&args, 3)?.parse().map_err(|_| "số tiền phải là số")?,
                ),
                other => {
                    return Err(format!(
                        "kết quả phải là continue, full hoặc partial, không phải {other}"
                    ))
                }
            };
            orders::resolve_dispute(&state, o.id, &reviewer, r)
                .await
                .map_err(o_err)?;
            println!("đã giải quyết tranh chấp và báo cho khách hàng");
        }
        "order-handover" => {
            let o = orders::by_code(&state.db, arg(&args, 1)?)
                .await
                .map_err(o_err)?;
            let to = orders::handover_path(&state, &o);
            if let Some(dir) = to.parent() {
                std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
            }
            std::fs::copy(arg(&args, 2)?, &to).map_err(|e| e.to_string())?;
            println!("đã lưu gói bàn giao cuối của {}", o.code);
        }
        "order-due" => {
            let n = orders::process_due(&state, time::OffsetDateTime::now_utc())
                .await
                .map_err(o_err)?;
            println!("đã tự nghiệm thu {n} mốc");
        }
        other => return Err(format!("lệnh không hợp lệ: {other}\n\n{USAGE}")),
    }
    Ok(())
}
