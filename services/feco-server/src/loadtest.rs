//! Bộ tạo tải cho kiểm thử hiệu năng (tasks.md 15.2; spec platform-support "Hiệu năng").
//!
//! Tải mở (open-loop): gửi đều `rps` yêu cầu mỗi giây trong `duration`, không chờ yêu cầu trước
//! xong, nên độ trễ đo được phản ánh đúng khi máy chủ chậm (không bị "coordinated omission").
//! Số yêu cầu đang bay bị chặn ở `max_in_flight`; yêu cầu vượt mức được tính là lỗi.

use std::sync::Arc;
use std::time::{Duration, Instant};

use tokio::sync::{mpsc, Semaphore};

#[derive(Debug, Clone)]
pub struct Report {
    pub target_rps: u32,
    pub seconds: f64,
    pub sent: usize,
    pub ok: usize,
    pub errors: usize,
    /// Mã HTTP khác 200, kèm số lần.
    pub statuses: Vec<(u16, usize)>,
    pub achieved_rps: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub max_ms: f64,
}

/// Phân vị `p` (0..=100) của dãy đã sắp xếp tăng dần.
pub fn percentile(sorted: &[f64], p: f64) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let rank = (p / 100.0 * sorted.len() as f64).ceil() as usize;
    sorted[rank.clamp(1, sorted.len()) - 1]
}

/// Gửi GET `url` kèm `Authorization: Bearer token` với tốc độ `rps` trong `duration`.
pub async fn run(
    client: reqwest::Client,
    url: String,
    token: String,
    rps: u32,
    duration: Duration,
    max_in_flight: usize,
) -> Report {
    let (tx, mut rx) = mpsc::unbounded_channel::<Result<(u16, f64), ()>>();
    let gate = Arc::new(Semaphore::new(max_in_flight));
    let total = (f64::from(rps) * duration.as_secs_f64()).round() as usize;
    let interval = Duration::from_secs_f64(1.0 / f64::from(rps.max(1)));
    let start = Instant::now();
    let mut overflow = 0usize;
    for i in 0..total {
        let due = start + interval.mul_f64(i as f64);
        tokio::time::sleep_until(due.into()).await;
        let Ok(permit) = gate.clone().try_acquire_owned() else {
            overflow += 1;
            continue;
        };
        let (client, url, token, tx) = (client.clone(), url.clone(), token.clone(), tx.clone());
        tokio::spawn(async move {
            let t0 = Instant::now();
            let r = client.get(&url).bearer_auth(&token).send().await;
            let result = match r {
                Ok(res) => {
                    let status = res.status().as_u16();
                    let _ = res.bytes().await;
                    Ok((status, t0.elapsed().as_secs_f64() * 1000.0))
                }
                Err(_) => Err(()),
            };
            drop(permit);
            let _ = tx.send(result);
        });
    }
    drop(tx);
    let mut latencies = Vec::with_capacity(total);
    let mut statuses: std::collections::BTreeMap<u16, usize> = Default::default();
    let mut ok = 0;
    let mut errors = overflow;
    while let Some(r) = rx.recv().await {
        match r {
            Ok((200, ms)) => {
                ok += 1;
                latencies.push(ms);
            }
            Ok((status, ms)) => {
                errors += 1;
                latencies.push(ms);
                *statuses.entry(status).or_default() += 1;
            }
            Err(()) => errors += 1,
        }
    }
    let seconds = start.elapsed().as_secs_f64();
    latencies.sort_by(f64::total_cmp);
    Report {
        target_rps: rps,
        seconds,
        sent: total,
        ok,
        errors,
        statuses: statuses.into_iter().collect(),
        achieved_rps: (ok + errors) as f64 / seconds,
        p50_ms: percentile(&latencies, 50.0),
        p95_ms: percentile(&latencies, 95.0),
        p99_ms: percentile(&latencies, 99.0),
        max_ms: latencies.last().copied().unwrap_or(0.0),
    }
}

impl Report {
    /// Bảng Markdown cho biên bản kiểm thử tải.
    pub fn markdown(&self) -> String {
        format!(
            "| Chỉ số | Giá trị |\n|---|---|\n\
             | Tốc độ đặt ra | {} yêu cầu/giây |\n\
             | Thời gian | {:.1} giây |\n\
             | Đã gửi | {} |\n\
             | Thành công (200) | {} |\n\
             | Lỗi | {} {:?} |\n\
             | Tốc độ đạt được | {:.0} yêu cầu/giây |\n\
             | p50 | {:.1} ms |\n\
             | p95 | {:.1} ms |\n\
             | p99 | {:.1} ms |\n\
             | Lâu nhất | {:.1} ms |\n",
            self.target_rps,
            self.seconds,
            self.sent,
            self.ok,
            self.errors,
            self.statuses,
            self.achieved_rps,
            self.p50_ms,
            self.p95_ms,
            self.p99_ms,
            self.max_ms
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn percentiles() {
        let v: Vec<f64> = (1..=100).map(f64::from).collect();
        assert_eq!(percentile(&v, 50.0), 50.0);
        assert_eq!(percentile(&v, 95.0), 95.0);
        assert_eq!(percentile(&v, 100.0), 100.0);
        assert_eq!(percentile(&[7.0], 99.0), 7.0);
        assert_eq!(percentile(&[], 95.0), 0.0);
    }

    /// Bộ tạo tải giữ đúng nhịp với một máy chủ cục bộ đơn giản.
    #[tokio::test]
    async fn paces_requests() {
        let app = axum::Router::new().route("/me", axum::routing::get(|| async { "ok" }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("http://{}/me", listener.local_addr().unwrap());
        tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let r = run(
            reqwest::Client::new(),
            url,
            "t".into(),
            200,
            Duration::from_millis(500),
            64,
        )
        .await;
        assert_eq!(r.sent, 100);
        assert_eq!(r.ok, 100, "{r:?}");
        assert!(r.seconds >= 0.45, "{r:?}");
        assert!(r.p95_ms > 0.0);
    }
}
