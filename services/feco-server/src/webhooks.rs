//! Gửi sự kiện cho app con (spec user-data-consent, "Trách nhiệm của app con với dữ
//! liệu"; spec app-sdk, "Sự kiện cho app"; spec platform-security, "Bảo mật webhook").
//!
//! - Mỗi yêu cầu có header `F-Eco-Signature: t=<giây unix>,v1=<hex>`, trong đó
//!   `v1 = HMAC-SHA256(khóa của app, "<t>.<thân yêu cầu>")`.
//! - App không trả 2xx: gửi lại sau 1, 5, 30 phút rồi giãn dần, trong tối đa 72 giờ.

use std::time::Duration;

use hmac::{Hmac, KeyInit, Mac};
use sha2::Sha256;
use time::OffsetDateTime;

use crate::{
    crypto::DataKey,
    store::{self, Db, PendingDelivery},
};

pub const MAX_AGE: Duration = Duration::from_secs(72 * 3600);

/// Thời gian chờ trước lần gửi thứ `attempts + 1` (sau khi đã thất bại `attempts` lần).
pub fn backoff(attempts: i32) -> Duration {
    const MIN: u64 = 60;
    let secs = match attempts {
        ..=1 => MIN,
        2 => 5 * MIN,
        3 => 30 * MIN,
        4 => 2 * 3600,
        5 => 6 * 3600,
        _ => 12 * 3600,
    };
    Duration::from_secs(secs)
}

pub fn sign(secret: &str, timestamp: i64, body: &[u8]) -> String {
    let mut mac =
        Hmac::<Sha256>::new_from_slice(secret.as_bytes()).expect("HMAC nhận khóa mọi độ dài");
    mac.update(timestamp.to_string().as_bytes());
    mac.update(b".");
    mac.update(body);
    let hex: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    format!("t={timestamp},v1={hex}")
}

/// Kiểm tra chữ ký (dùng trong F-SDK và test). Chấp nhận lệch đồng hồ tối đa 5 phút.
pub fn verify(secret: &str, header: &str, body: &[u8], now: i64) -> bool {
    let mut t = None;
    let mut v1 = None;
    for part in header.split(',') {
        match part.split_once('=') {
            Some(("t", x)) => t = x.parse::<i64>().ok(),
            Some(("v1", x)) => v1 = Some(x),
            _ => {}
        }
    }
    let (Some(t), Some(v1)) = (t, v1) else {
        return false;
    };
    if (now - t).abs() > 300 {
        return false;
    }
    let expected = sign(secret, t, body);
    let expected = expected.rsplit("v1=").next().unwrap_or_default();
    expected.len() == v1.len()
        && expected
            .bytes()
            .zip(v1.bytes())
            .fold(0u8, |acc, (a, b)| acc | (a ^ b))
            == 0
}

async fn deliver_one(
    http: &reqwest::Client,
    key: &DataKey,
    d: &PendingDelivery,
) -> Result<(), String> {
    let secret = key
        .decrypt_text(&d.secret, &store::webhook_aad(&d.client_id))
        .map_err(|e| format!("khóa webhook: {e}"))?;
    let body = serde_json::to_vec(&d.payload).map_err(|e| e.to_string())?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let res = http
        .post(&d.url)
        .header("content-type", "application/json")
        .header("f-eco-event", &d.event_type)
        .header("f-eco-delivery", d.id.to_string())
        .header("f-eco-signature", sign(&secret, now, &body))
        .body(body)
        .send()
        .await
        .map_err(|e| e.to_string())?;
    if res.status().is_success() {
        Ok(())
    } else {
        Err(format!("HTTP {}", res.status().as_u16()))
    }
}

/// Gửi mọi webhook đến hạn. Trả về (số đã gửi thành công, số thất bại lần này).
pub async fn deliver_due(
    db: &Db,
    http: &reqwest::Client,
    key: &DataKey,
) -> sqlx::Result<(usize, usize)> {
    let mut tx = db.begin().await?;
    let due = store::due_deliveries(&mut tx, 50).await?;
    let (mut ok, mut failed) = (0, 0);
    for d in &due {
        match deliver_one(http, key, d).await {
            Ok(()) => {
                ok += 1;
                sqlx::query("UPDATE webhook_outbox SET delivered_at = now(), attempts = attempts + 1, last_error = NULL WHERE id = $1")
                    .bind(d.id)
                    .execute(&mut *tx)
                    .await?;
            }
            Err(e) => {
                failed += 1;
                let attempts = d.attempts + 1;
                let next = OffsetDateTime::now_utc() + backoff(attempts);
                let expired = next - d.created_at > MAX_AGE;
                tracing::warn!(delivery = d.id, attempts, error = %e, "gửi webhook thất bại");
                sqlx::query(
                    "UPDATE webhook_outbox SET attempts = $2, last_error = $3, next_attempt_at = $4,
                         failed_at = CASE WHEN $5 THEN now() ELSE NULL END
                     WHERE id = $1",
                )
                .bind(d.id)
                .bind(attempts)
                .bind(&e)
                .bind(next)
                .bind(expired)
                .execute(&mut *tx)
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok((ok, failed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn backoff_schedule_matches_spec() {
        let mins: Vec<u64> = (1..=4).map(|a| backoff(a).as_secs() / 60).collect();
        assert_eq!(mins, [1, 5, 30, 120]);
        assert_eq!(backoff(20).as_secs(), 12 * 3600);
    }

    #[test]
    fn retries_continue_until_72_hours_then_stop() {
        // Giả lập như deliver_due: thử lại cho tới khi lần kế tiếp vượt 72 giờ.
        let mut elapsed = Duration::ZERO;
        let mut attempts = 0;
        loop {
            attempts += 1;
            let next = elapsed + backoff(attempts);
            if next > MAX_AGE {
                break;
            }
            elapsed = next;
        }
        assert!(
            elapsed > Duration::from_secs(60 * 3600),
            "phải còn thử sau 60 giờ"
        );
        assert!(elapsed <= MAX_AGE);
        assert!((8..=15).contains(&attempts), "số lần thử: {attempts}");
    }

    #[test]
    fn signature_round_trip_and_tamper_detection() {
        let body = br#"{"event":"user.revoked"}"#;
        let h = sign("khoa", 1_700_000_000, body);
        assert!(h.starts_with("t=1700000000,v1="));
        assert!(verify("khoa", &h, body, 1_700_000_100));
        assert!(!verify(
            "khoa",
            &h,
            br#"{"event":"user.deleted"}"#,
            1_700_000_100
        ));
        assert!(!verify("khoa-khac", &h, body, 1_700_000_100));
        assert!(!verify("khoa", &h, body, 1_700_000_000 + 301), "quá 5 phút");
        assert!(!verify("khoa", "v1=abc", body, 1_700_000_000));
    }
}
