//! Giới hạn tần suất (spec platform-security, "Giới hạn tần suất"): theo địa chỉ IP
//! (mọi yêu cầu), theo app và theo người dùng (API dữ liệu). Vượt giới hạn trả 429 kèm
//! `Retry-After`.
//!
//! Dùng cửa sổ trượt xấp xỉ (hai cửa sổ cố định có trọng số), bộ đếm trong bộ nhớ của
//! tiến trình. Khi chạy nhiều bản feco-server cần chuyển sang Valkey (QUESTIONS.md).

use std::{
    collections::HashMap,
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug)]
struct Window {
    start: Instant,
    current: u32,
    previous: u32,
}

#[derive(Debug, Default)]
pub struct RateLimiter {
    windows: Mutex<HashMap<String, Window>>,
}

impl RateLimiter {
    /// Ghi nhận một yêu cầu của `key`. Trả `Err(thời gian chờ)` khi vượt `limit` yêu cầu
    /// trong `period`.
    pub fn check(
        &self,
        key: &str,
        limit: u32,
        period: Duration,
        now: Instant,
    ) -> Result<(), Duration> {
        let mut map = self.windows.lock().expect("khóa bộ đếm");
        if map.len() > 200_000 {
            map.retain(|_, w| now.duration_since(w.start) < period * 2);
        }
        let w = map.entry(key.to_owned()).or_insert(Window {
            start: now,
            current: 0,
            previous: 0,
        });
        let elapsed = now.duration_since(w.start);
        if elapsed >= period * 2 {
            *w = Window {
                start: now,
                current: 0,
                previous: 0,
            };
        } else if elapsed >= period {
            w.previous = w.current;
            w.current = 0;
            w.start += period;
        }
        let into = now.duration_since(w.start).as_secs_f64() / period.as_secs_f64();
        let estimated = w.previous as f64 * (1.0 - into) + w.current as f64;
        if estimated + 1.0 > limit as f64 {
            let wait = period.saturating_sub(now.duration_since(w.start));
            return Err(wait.max(Duration::from_secs(1)));
        }
        w.current += 1;
        Ok(())
    }
}

/// Giá trị header `Retry-After` (giây, làm tròn lên).
pub fn retry_after_secs(wait: Duration) -> u64 {
    wait.as_secs() + u64::from(wait.subsec_nanos() > 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    const MIN: Duration = Duration::from_secs(60);

    #[test]
    fn allows_up_to_limit_then_rejects_with_wait() {
        let r = RateLimiter::default();
        let t = Instant::now();
        for _ in 0..5 {
            assert!(r.check("ip:1", 5, MIN, t).is_ok());
        }
        let wait = r
            .check("ip:1", 5, MIN, t + Duration::from_secs(10))
            .unwrap_err();
        assert_eq!(retry_after_secs(wait), 50);
    }

    #[test]
    fn keys_are_independent() {
        let r = RateLimiter::default();
        let t = Instant::now();
        assert!(r.check("app:a", 1, MIN, t).is_ok());
        assert!(r.check("app:a", 1, MIN, t).is_err());
        assert!(r.check("app:b", 1, MIN, t).is_ok());
    }

    #[test]
    fn previous_window_is_weighted_then_forgotten() {
        let r = RateLimiter::default();
        let t = Instant::now();
        for _ in 0..10 {
            r.check("k", 10, MIN, t).unwrap();
        }
        // Đầu phút sau: phút trước vẫn còn tính gần đủ, nên chưa gửi được.
        assert!(r.check("k", 10, MIN, t + Duration::from_secs(61)).is_err());
        // Cuối phút sau: phần phút trước chỉ còn ít, gửi được trở lại.
        assert!(r.check("k", 10, MIN, t + Duration::from_secs(115)).is_ok());
        // Sau hai phút không gửi gì: đếm lại từ đầu.
        for _ in 0..10 {
            assert!(r.check("k", 10, MIN, t + Duration::from_secs(300)).is_ok());
        }
    }

    #[test]
    fn retry_after_rounds_up() {
        assert_eq!(retry_after_secs(Duration::from_millis(1500)), 2);
        assert_eq!(retry_after_secs(Duration::from_secs(3)), 3);
    }
}
