//! Dữ liệu danh mục F-Store (spec app-catalog).
//!
//! - Chỉ hiện app ở trạng thái `listed` có ít nhất một phiên bản `released` (đã qua kiểm duyệt).
//! - Tìm theo tên, mô tả, nhà phát triển, không phân biệt dấu tiếng Việt và hoa thường
//!   ([`normalize`]); lọc theo danh mục, hệ điều hành, miễn phí/trả phí, nhà phát triển đã
//!   xác minh.
//! - Kết quả tìm kiếm được giữ trong bộ nhớ tối đa [`CACHE_TTL`]; mọi thao tác ghi xóa bộ nhớ
//!   đệm ngay. Khi chạy nhiều bản feco-server, app bị gỡ biến khỏi danh mục chậm nhất sau
//!   `CACHE_TTL` (dưới mức 5 phút của spec).

use std::{
    collections::{BTreeMap, HashMap},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde::{Deserialize, Serialize};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::store::Db;

/// Thời gian giữ kết quả tìm kiếm trong bộ nhớ.
pub const CACHE_TTL: Duration = Duration::from_secs(60);

/// Lý do báo cáo vi phạm (spec app-catalog, "Báo cáo vi phạm").
pub const REPORT_REASONS: [&str; 5] = ["malware", "fraud", "copyright", "inappropriate", "other"];

/// Viết thường, bỏ dấu tiếng Việt (cả dạng dựng sẵn lẫn dạng tổ hợp), `đ` → `d`, gom ký tự
/// không phải chữ/số thành một khoảng trắng.
pub fn normalize(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut space = true;
    for c in s.chars().flat_map(char::to_lowercase) {
        // Dấu tổ hợp (NFD): bỏ.
        if ('\u{0300}'..='\u{036f}').contains(&c) {
            continue;
        }
        let base = fold(c);
        if base.is_alphanumeric() {
            out.push(base);
            space = false;
        } else if !space {
            out.push(' ');
            space = true;
        }
    }
    out.trim_end().to_owned()
}

fn fold(c: char) -> char {
    const GROUPS: [(&str, char); 7] = [
        ("àáảãạăằắẳẵặâầấẩẫậ", 'a'),
        ("èéẻẽẹêềếểễệ", 'e'),
        ("ìíỉĩị", 'i'),
        ("òóỏõọôồốổỗộơờớởỡợ", 'o'),
        ("ùúủũụưừứửữự", 'u'),
        ("ỳýỷỹỵ", 'y'),
        ("đ", 'd'),
    ];
    GROUPS
        .iter()
        .find(|(set, _)| set.contains(c))
        .map(|(_, base)| *base)
        .unwrap_or(c)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Price {
    Free,
    Paid,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct Filters {
    pub q: Option<String>,
    pub category: Option<String>,
    /// Tiền tố nền tảng: `windows`, `macos`.
    pub os: Option<String>,
    pub price: Option<Price>,
    pub verified_only: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct Listing {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub developer: String,
    pub developer_verified: bool,
    pub first_party: bool,
    pub summary: String,
    pub category: String,
    pub price_vnd: u64,
    pub platforms: Vec<String>,
    pub latest_version: String,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
    pub rating_avg: Option<f64>,
    pub rating_count: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct Version {
    pub version: String,
    pub platform: String,
    pub size_bytes: i64,
    pub changelog: String,
    #[serde(with = "time::serde::rfc3339")]
    pub released_at: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct Detail {
    #[serde(flatten)]
    pub listing: Listing,
    pub description: String,
    pub license: String,
    pub screenshots: Vec<String>,
    /// Mọi phiên bản đã phát hành, mới nhất trước (lịch sử thay đổi).
    pub versions: Vec<Version>,
    #[serde(skip)]
    pub owner_subject: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Review {
    pub id: Uuid,
    pub author_name: String,
    pub rating: i16,
    pub body: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    pub reply_body: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub reply_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct Report {
    pub id: Uuid,
    pub reporter: Uuid,
    pub app_id: String,
    pub review_id: Option<Uuid>,
    pub reason: String,
    pub details: String,
    pub status: String,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

/// App mới hoặc cập nhật thông tin app (cổng nhà phát triển và kiểm duyệt sẽ gọi ở nhóm 8, 9).
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppInput {
    pub id: String,
    pub client_id: String,
    pub name: String,
    pub developer: String,
    pub developer_verified: bool,
    pub owner_subject: Option<Uuid>,
    pub first_party: bool,
    pub summary: String,
    pub description: String,
    pub category: String,
    pub price_vnd: u64,
    pub license: String,
    pub screenshots: Vec<String>,
}

#[derive(Debug, Clone, Default)]
pub struct VersionInput {
    pub version: String,
    pub platform: String,
    pub size_bytes: u64,
    pub tuf_target: String,
    pub changelog: String,
}

/// Bộ nhớ đệm kết quả tìm kiếm.
#[derive(Default)]
pub struct Cache {
    entries: Mutex<HashMap<Filters, (Instant, Vec<Listing>)>>,
}

impl Cache {
    pub fn get(&self, f: &Filters, now: Instant, ttl: Duration) -> Option<Vec<Listing>> {
        let entries = self.entries.lock().expect("khóa bộ nhớ đệm");
        entries
            .get(f)
            .filter(|(at, _)| now.duration_since(*at) < ttl)
            .map(|(_, v)| v.clone())
    }

    pub fn put(&self, f: Filters, now: Instant, v: Vec<Listing>) {
        let mut entries = self.entries.lock().expect("khóa bộ nhớ đệm");
        if entries.len() > 1000 {
            entries.clear();
        }
        entries.insert(f, (now, v));
    }

    pub fn clear(&self) {
        self.entries.lock().expect("khóa bộ nhớ đệm").clear();
    }
}

type AppRow = (
    String,
    String,
    String,
    String,
    bool,
    bool,
    String,
    String,
    i64,
    OffsetDateTime,
);

type ReviewRow = (
    Uuid,
    String,
    i16,
    String,
    OffsetDateTime,
    Option<String>,
    Option<OffsetDateTime>,
);

type ReportRow = (
    Uuid,
    Uuid,
    String,
    Option<Uuid>,
    String,
    String,
    String,
    OffsetDateTime,
);

const APP_COLUMNS: &str = "a.id, a.client_id, a.name, a.developer, a.developer_verified, \
     a.first_party, a.summary, a.category, a.price_vnd, a.updated_at";

/// Điều kiện "đang hiện trong danh mục". App riêng tư (bản build bàn giao của đơn xây app, spec
/// custom-build-orders) không hiện công khai: chỉ khách hàng của đơn thấy qua [`private_for`].
const VISIBLE: &str =
    "a.status = 'listed' AND a.private_to IS NULL AND EXISTS (SELECT 1 FROM catalog_versions v \
     WHERE v.app_id = a.id AND v.status = 'released')";

async fn decorate(db: &Db, rows: Vec<AppRow>) -> sqlx::Result<Vec<Listing>> {
    let ids: Vec<String> = rows.iter().map(|r| r.0.clone()).collect();
    let versions: Vec<(String, String, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT app_id, version, platform, released_at FROM catalog_versions
         WHERE app_id = ANY($1) AND status = 'released' ORDER BY released_at DESC",
    )
    .bind(&ids)
    .fetch_all(db)
    .await?;
    let ratings: Vec<(String, Option<f64>, i64)> = sqlx::query_as(
        "SELECT app_id, avg(rating)::float8, count(*) FROM catalog_reviews
         WHERE app_id = ANY($1) AND NOT hidden GROUP BY app_id",
    )
    .bind(&ids)
    .fetch_all(db)
    .await?;
    let mut by_app: BTreeMap<String, Vec<(String, String, OffsetDateTime)>> = BTreeMap::new();
    for (app, version, platform, at) in versions {
        by_app.entry(app).or_default().push((version, platform, at));
    }
    Ok(rows
        .into_iter()
        .map(|r| {
            let vs = by_app.remove(&r.0).unwrap_or_default();
            let mut platforms: Vec<String> = vs.iter().map(|v| v.1.clone()).collect();
            platforms.sort();
            platforms.dedup();
            let (rating_avg, rating_count) = ratings
                .iter()
                .find(|x| x.0 == r.0)
                .map(|x| (x.1, x.2))
                .unwrap_or((None, 0));
            Listing {
                latest_version: vs.first().map(|v| v.0.clone()).unwrap_or_default(),
                updated_at: vs.first().map(|v| v.2).unwrap_or(r.9).max(r.9),
                id: r.0,
                client_id: r.1,
                name: r.2,
                developer: r.3,
                developer_verified: r.4,
                first_party: r.5,
                summary: r.6,
                category: r.7,
                price_vnd: r.8.max(0) as u64,
                platforms,
                rating_avg,
                rating_count,
            }
        })
        .collect())
}

/// Tìm và lọc trong danh mục (không qua bộ nhớ đệm).
pub async fn search(db: &Db, f: &Filters) -> sqlx::Result<Vec<Listing>> {
    let patterns: Vec<String> =
        f.q.as_deref()
            .map(normalize)
            .unwrap_or_default()
            .split(' ')
            .filter(|w| !w.is_empty())
            .map(|w| format!("%{w}%"))
            .collect();
    let sql = format!(
        "SELECT {APP_COLUMNS} FROM catalog_apps a WHERE {VISIBLE}
           AND (cardinality($1::text[]) = 0 OR a.search_text LIKE ALL ($1))
           AND ($2::text IS NULL OR a.category = $2)
           AND ($3::text IS NULL OR EXISTS (SELECT 1 FROM catalog_versions v
                WHERE v.app_id = a.id AND v.status = 'released' AND v.platform LIKE $3 || '%'))
           AND ($4::bool IS NULL OR (a.price_vnd = 0) = $4)
           AND (NOT $5 OR a.developer_verified)
         ORDER BY a.first_party DESC, a.name"
    );
    let rows: Vec<AppRow> = sqlx::query_as(&sql)
        .bind(&patterns)
        .bind(f.category.as_deref().filter(|c| !c.is_empty()))
        .bind(f.os.as_deref().filter(|c| !c.is_empty()))
        .bind(f.price.map(|p| p == Price::Free))
        .bind(f.verified_only)
        .fetch_all(db)
        .await?;
    decorate(db, rows).await
}

/// Tìm qua bộ nhớ đệm.
pub async fn search_cached(db: &Db, cache: &Cache, f: &Filters) -> sqlx::Result<Vec<Listing>> {
    let now = Instant::now();
    if let Some(hit) = cache.get(f, now, CACHE_TTL) {
        return Ok(hit);
    }
    let result = search(db, f).await?;
    cache.put(f.clone(), now, result.clone());
    Ok(result)
}

/// Các danh mục đang có app hiện.
pub async fn categories(db: &Db) -> sqlx::Result<Vec<String>> {
    sqlx::query_scalar(&format!(
        "SELECT DISTINCT a.category FROM catalog_apps a WHERE {VISIBLE} ORDER BY 1"
    ))
    .fetch_all(db)
    .await
}

/// Trang chi tiết; `None` nếu app không có hoặc không hiện trong danh mục.
pub async fn detail(db: &Db, id: &str) -> sqlx::Result<Option<Detail>> {
    let row: Option<AppRow> = sqlx::query_as(&format!(
        "SELECT {APP_COLUMNS} FROM catalog_apps a WHERE a.id = $1 AND {VISIBLE}"
    ))
    .bind(id)
    .fetch_optional(db)
    .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let (description, license, screenshots, owner_subject): (
        String,
        String,
        Vec<String>,
        Option<Uuid>,
    ) = sqlx::query_as(
        "SELECT description, license, screenshots, owner_subject FROM catalog_apps WHERE id = $1",
    )
    .bind(id)
    .fetch_one(db)
    .await?;
    let versions: Vec<(String, String, i64, String, OffsetDateTime)> = sqlx::query_as(
        "SELECT version, platform, size_bytes, changelog, released_at FROM catalog_versions
         WHERE app_id = $1 AND status = 'released' ORDER BY released_at DESC, platform",
    )
    .bind(id)
    .fetch_all(db)
    .await?;
    let listing = decorate(db, vec![row]).await?.remove(0);
    Ok(Some(Detail {
        listing,
        description,
        license,
        screenshots,
        versions: versions
            .into_iter()
            .map(|v| Version {
                version: v.0,
                platform: v.1,
                size_bytes: v.2,
                changelog: v.3,
                released_at: v.4,
            })
            .collect(),
        owner_subject,
    }))
}

/// Thêm hoặc cập nhật app (trạng thái giữ nguyên nếu đã có).
pub async fn upsert_app(db: &Db, cache: &Cache, a: &AppInput) -> sqlx::Result<()> {
    let search_text = normalize(&format!(
        "{} {} {} {}",
        a.name, a.summary, a.description, a.developer
    ));
    sqlx::query(
        "INSERT INTO catalog_apps (id, client_id, name, developer, developer_verified, owner_subject,
             first_party, summary, description, category, price_vnd, license, screenshots, search_text)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14)
         ON CONFLICT (id) DO UPDATE SET client_id = $2, name = $3, developer = $4,
             developer_verified = $5, owner_subject = $6, first_party = $7, summary = $8,
             description = $9, category = $10, price_vnd = $11, license = $12, screenshots = $13,
             search_text = $14, updated_at = now()",
    )
    .bind(&a.id)
    .bind(&a.client_id)
    .bind(&a.name)
    .bind(&a.developer)
    .bind(a.developer_verified)
    .bind(a.owner_subject)
    .bind(a.first_party)
    .bind(&a.summary)
    .bind(&a.description)
    .bind(&a.category)
    .bind(a.price_vnd as i64)
    .bind(&a.license)
    .bind(&a.screenshots)
    .bind(search_text)
    .execute(db)
    .await?;
    cache.clear();
    Ok(())
}

/// Ghi một phiên bản đã qua kiểm duyệt.
pub async fn release_version(
    db: &Db,
    cache: &Cache,
    app: &str,
    v: &VersionInput,
) -> sqlx::Result<()> {
    sqlx::query(
        "INSERT INTO catalog_versions (app_id, version, platform, size_bytes, tuf_target, changelog)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (app_id, version, platform) DO UPDATE SET status = 'released',
             size_bytes = $4, tuf_target = $5, changelog = $6",
    )
    .bind(app)
    .bind(&v.version)
    .bind(&v.platform)
    .bind(v.size_bytes as i64)
    .bind(&v.tuf_target)
    .bind(&v.changelog)
    .execute(db)
    .await?;
    cache.clear();
    Ok(())
}

/// Thu hồi một phiên bản (nhóm 9/14 làm cùng lúc với thu hồi trong TUF).
pub async fn revoke_version(db: &Db, cache: &Cache, app: &str, version: &str) -> sqlx::Result<u64> {
    let n = sqlx::query(
        "UPDATE catalog_versions SET status = 'revoked' WHERE app_id = $1 AND version = $2",
    )
    .bind(app)
    .bind(version)
    .execute(db)
    .await?
    .rows_affected();
    cache.clear();
    Ok(n)
}

/// Gỡ app khỏi danh mục (scenario "App bị gỡ").
pub async fn remove_app(db: &Db, cache: &Cache, app: &str, reason: &str) -> sqlx::Result<bool> {
    let n = sqlx::query(
        "UPDATE catalog_apps SET status = 'removed', removed_reason = $2, removed_at = now(),
             updated_at = now()
         WHERE id = $1 AND status = 'listed'",
    )
    .bind(app)
    .bind(reason)
    .execute(db)
    .await?
    .rows_affected();
    cache.clear();
    Ok(n > 0)
}

/// Ghi nhận người dùng đã cài app (launcher báo sau khi cài).
/// App riêng tư của một người dùng (đã phát hành ít nhất một phiên bản).
pub async fn private_for(db: &Db, subject: Uuid) -> sqlx::Result<Vec<Listing>> {
    let rows: Vec<AppRow> = sqlx::query_as(&format!(
        "SELECT {APP_COLUMNS} FROM catalog_apps a WHERE a.private_to = $1 AND a.status = 'listed'
           AND EXISTS (SELECT 1 FROM catalog_versions v WHERE v.app_id = a.id AND v.status = 'released')
         ORDER BY a.name"
    ))
    .bind(subject)
    .fetch_all(db)
    .await?;
    decorate(db, rows).await
}

/// Đặt (hoặc bỏ, với `None`) người dùng duy nhất thấy một app trong danh mục.
pub async fn set_private(
    db: &Db,
    cache: &Cache,
    app: &str,
    owner: Option<Uuid>,
) -> sqlx::Result<bool> {
    let n = sqlx::query("UPDATE catalog_apps SET private_to = $2 WHERE id = $1")
        .bind(app)
        .bind(owner)
        .execute(db)
        .await?
        .rows_affected();
    cache.clear();
    Ok(n > 0)
}

pub async fn record_install(
    db: &Db,
    subject: Uuid,
    app: &str,
    version: Option<&str>,
) -> sqlx::Result<bool> {
    let n = sqlx::query(
        "INSERT INTO catalog_installs (subject, app_id, version)
         SELECT $1, id, $3 FROM catalog_apps WHERE id = $2
         ON CONFLICT (subject, app_id) DO UPDATE SET version = COALESCE($3, catalog_installs.version)",
    )
    .bind(subject)
    .bind(app)
    .bind(version.filter(|v| v.len() <= 64))
    .execute(db)
    .await?
    .rows_affected();
    Ok(n > 0 || has_installed(db, subject, app).await?)
}

pub async fn has_installed(db: &Db, subject: Uuid, app: &str) -> sqlx::Result<bool> {
    sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM catalog_installs WHERE subject = $1 AND app_id = $2)",
    )
    .bind(subject)
    .bind(app)
    .fetch_one(db)
    .await
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReviewOutcome {
    Saved(Uuid),
    NotInstalled,
    Invalid,
}

/// Chấm điểm và nhận xét; chỉ người đã cài app (spec: "Bạn cần cài app trước khi đánh giá").
pub async fn upsert_review(
    db: &Db,
    cache: &Cache,
    app: &str,
    subject: Uuid,
    author_name: &str,
    rating: i16,
    body: &str,
) -> sqlx::Result<ReviewOutcome> {
    let body = body.trim();
    if !(1..=5).contains(&rating) || body.chars().count() > 4000 {
        return Ok(ReviewOutcome::Invalid);
    }
    if !has_installed(db, subject, app).await? {
        return Ok(ReviewOutcome::NotInstalled);
    }
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO catalog_reviews (id, app_id, subject, author_name, rating, body)
         VALUES ($1, $2, $3, $4, $5, $6)
         ON CONFLICT (app_id, subject) DO UPDATE SET author_name = $4, rating = $5, body = $6,
             updated_at = now()
         RETURNING id",
    )
    .bind(Uuid::new_v4())
    .bind(app)
    .bind(subject)
    .bind(author_name)
    .bind(rating)
    .bind(body)
    .fetch_one(db)
    .await?;
    cache.clear();
    Ok(ReviewOutcome::Saved(id))
}

/// Nhận xét đang hiện của một app, mới nhất trước.
pub async fn reviews(db: &Db, app: &str) -> sqlx::Result<Vec<Review>> {
    let rows: Vec<ReviewRow> = sqlx::query_as(
        "SELECT id, author_name, rating, body, created_at, reply_body, reply_at
             FROM catalog_reviews WHERE app_id = $1 AND NOT hidden ORDER BY created_at DESC",
    )
    .bind(app)
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Review {
            id: r.0,
            author_name: r.1,
            rating: r.2,
            body: r.3,
            created_at: r.4,
            reply_body: r.5,
            reply_at: r.6,
        })
        .collect())
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReplyOutcome {
    Saved,
    NotOwner,
    NotFound,
}

/// Nhà phát triển (chủ app) trả lời một nhận xét.
pub async fn reply(
    db: &Db,
    app: &str,
    review: Uuid,
    developer: Uuid,
    body: &str,
) -> sqlx::Result<ReplyOutcome> {
    let owner: Option<Option<Uuid>> =
        sqlx::query_scalar("SELECT owner_subject FROM catalog_apps WHERE id = $1")
            .bind(app)
            .fetch_optional(db)
            .await?;
    match owner {
        None => return Ok(ReplyOutcome::NotFound),
        Some(o) if o != Some(developer) => return Ok(ReplyOutcome::NotOwner),
        Some(_) => {}
    }
    let n = sqlx::query(
        "UPDATE catalog_reviews SET reply_body = $3, reply_at = now()
         WHERE id = $1 AND app_id = $2 AND NOT hidden",
    )
    .bind(review)
    .bind(app)
    .bind(body.trim())
    .execute(db)
    .await?
    .rows_affected();
    Ok(if n > 0 {
        ReplyOutcome::Saved
    } else {
        ReplyOutcome::NotFound
    })
}

/// Báo cáo một app hoặc một nhận xét; vào hàng chờ của quản trị viên.
pub async fn report(
    db: &Db,
    reporter: Uuid,
    app: &str,
    review: Option<Uuid>,
    reason: &str,
    details: &str,
) -> sqlx::Result<Option<Uuid>> {
    if !REPORT_REASONS.contains(&reason) || details.chars().count() > 4000 {
        return Ok(None);
    }
    let id = Uuid::new_v4();
    let n = sqlx::query(
        "INSERT INTO catalog_reports (id, reporter, app_id, review_id, reason, details)
         SELECT $1, $2, a.id, $4, $5, $6 FROM catalog_apps a WHERE a.id = $3
           AND ($4::uuid IS NULL OR EXISTS (SELECT 1 FROM catalog_reviews r
                WHERE r.id = $4 AND r.app_id = a.id))",
    )
    .bind(id)
    .bind(reporter)
    .bind(app)
    .bind(review)
    .bind(reason)
    .bind(details.trim())
    .execute(db)
    .await?
    .rows_affected();
    Ok((n > 0).then_some(id))
}

/// Hàng chờ báo cáo chưa xử lý, cũ nhất trước.
pub async fn open_reports(db: &Db) -> sqlx::Result<Vec<Report>> {
    let rows: Vec<ReportRow> = sqlx::query_as(
        "SELECT id, reporter, app_id, review_id, reason, details, status, created_at
             FROM catalog_reports WHERE status = 'open' ORDER BY created_at",
    )
    .fetch_all(db)
    .await?;
    Ok(rows
        .into_iter()
        .map(|r| Report {
            id: r.0,
            reporter: r.1,
            app_id: r.2,
            review_id: r.3,
            reason: r.4,
            details: r.5,
            status: r.6,
            created_at: r.7,
        })
        .collect())
}

/// Xử lý báo cáo. Báo cáo nhận xét được xác nhận thì nhận xét bị ẩn. Trả báo cáo đã xử lý để
/// gửi email cho người báo cáo; `None` nếu không có báo cáo đang mở với id đó.
pub async fn resolve_report(
    db: &Db,
    cache: &Cache,
    id: Uuid,
    upheld: bool,
    resolution: &str,
) -> sqlx::Result<Option<Report>> {
    let mut tx = db.begin().await?;
    let row: Option<ReportRow> = sqlx::query_as(
        "UPDATE catalog_reports SET status = $2, resolution = $3, resolved_at = now()
             WHERE id = $1 AND status = 'open'
             RETURNING id, reporter, app_id, review_id, reason, details, status, created_at",
    )
    .bind(id)
    .bind(if upheld { "upheld" } else { "rejected" })
    .bind(resolution)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some((_, _, _, Some(review), ..)) = &row {
        if upheld {
            sqlx::query("UPDATE catalog_reviews SET hidden = true WHERE id = $1")
                .bind(review)
                .execute(&mut *tx)
                .await?;
        }
    }
    tx.commit().await?;
    cache.clear();
    Ok(row.map(|r| Report {
        id: r.0,
        reporter: r.1,
        app_id: r.2,
        review_id: r.3,
        reason: r.4,
        details: r.5,
        status: r.6,
        created_at: r.7,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_ignores_vietnamese_accents_and_case() {
        assert_eq!(normalize("Sổ tay Học tập"), "so tay hoc tap");
        assert_eq!(normalize("SO TAY hoc tap"), "so tay hoc tap");
        assert_eq!(normalize("Đàn ĐIỆN TỬ — Âm nhạc!"), "dan dien tu am nhac");
        // Dạng tổ hợp (NFD): "ố" = "o" + U+0302 + U+0301.
        assert_eq!(normalize("So\u{0302}\u{0301} tay"), "so tay");
        assert_eq!(normalize("  Ứng   dụng   "), "ung dung");
        assert_eq!(normalize("Ỷ Ỵ ỹ ữ ợ"), "y y y u o");
    }

    #[test]
    fn cache_expires_and_clears() {
        let cache = Cache::default();
        let f = Filters::default();
        let t0 = Instant::now();
        cache.put(f.clone(), t0, vec![]);
        assert!(cache.get(&f, t0, CACHE_TTL).is_some());
        assert!(cache.get(&f, t0 + CACHE_TTL, CACHE_TTL).is_none());
        cache.clear();
        assert!(cache.get(&f, t0, CACHE_TTL).is_none());
        assert!(CACHE_TTL < Duration::from_secs(5 * 60));
    }
}
