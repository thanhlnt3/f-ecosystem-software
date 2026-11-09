//! Chống dò mật khẩu (spec account-identity, "Chống dò mật khẩu"):
//! - làm chậm dần các lần đăng nhập sai liên tiếp vào cùng một tài khoản;
//! - từ lần sai thứ 5 trong 15 phút, yêu cầu thử thách chống bot;
//! - không bao giờ khóa cứng tài khoản.
//!
//! Bộ đếm hiện lưu trong bộ nhớ của tiến trình. Khi chạy nhiều bản feco-server,
//! cần chuyển sang Valkey để các bản dùng chung bộ đếm (ghi trong tasks.md 2.2).

use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

#[derive(Debug, Clone, Copy)]
pub struct Policy {
    pub window: Duration,
    pub challenge_after: usize,
    pub base_delay: Duration,
    pub max_delay: Duration,
}

impl Default for Policy {
    fn default() -> Self {
        Policy {
            window: Duration::from_secs(15 * 60),
            challenge_after: 5,
            base_delay: Duration::from_millis(250),
            max_delay: Duration::from_secs(4),
        }
    }
}

impl Policy {
    /// Thời gian chờ trước lần thử tiếp theo khi đã có `failures` lần sai trong cửa sổ:
    /// 0, 250 ms, 500 ms, 1 s, 2 s, rồi tối đa 4 s.
    pub fn delay(&self, failures: usize) -> Duration {
        if failures == 0 {
            return Duration::ZERO;
        }
        let exp = u32::try_from(failures - 1).unwrap_or(u32::MAX).min(16);
        self.base_delay
            .saturating_mul(1u32 << exp)
            .min(self.max_delay)
    }
}

/// Trạng thái của một định danh trước lần đăng nhập tiếp theo.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Status {
    pub failures: usize,
    pub delay: Duration,
    pub challenge_required: bool,
}

#[derive(Debug, Default)]
pub struct LoginThrottle {
    policy: Policy,
    failures: Mutex<HashMap<String, VecDeque<Instant>>>,
}

/// Chuẩn hóa định danh: không phân biệt hoa thường và khoảng trắng hai đầu, giống
/// cách Kratos so khớp email.
pub fn normalize(identifier: &str) -> String {
    identifier.trim().to_lowercase()
}

impl LoginThrottle {
    pub fn new(policy: Policy) -> Self {
        LoginThrottle {
            policy,
            failures: Mutex::default(),
        }
    }

    pub fn policy(&self) -> &Policy {
        &self.policy
    }

    fn prune(&self, q: &mut VecDeque<Instant>, now: Instant) {
        while q
            .front()
            .is_some_and(|t| now.duration_since(*t) >= self.policy.window)
        {
            q.pop_front();
        }
    }

    pub fn status(&self, identifier: &str, now: Instant) -> Status {
        let mut map = self.failures.lock().expect("khóa bộ đếm");
        let failures = match map.get_mut(&normalize(identifier)) {
            Some(q) => {
                self.prune(q, now);
                q.len()
            }
            None => 0,
        };
        Status {
            failures,
            delay: self.policy.delay(failures),
            challenge_required: failures >= self.policy.challenge_after,
        }
    }

    /// Ghi một lần sai; trả về số lần sai trong cửa sổ sau khi ghi.
    pub fn record_failure(&self, identifier: &str, now: Instant) -> usize {
        let mut map = self.failures.lock().expect("khóa bộ đếm");
        // Dọn các định danh đã hết hạn để bộ nhớ không phình mãi.
        if map.len() > 100_000 {
            map.retain(|_, q| {
                q.back()
                    .is_some_and(|t| now.duration_since(*t) < self.policy.window)
            });
        }
        let q = map.entry(normalize(identifier)).or_default();
        self.prune(q, now);
        q.push_back(now);
        q.len()
    }

    pub fn reset(&self, identifier: &str) {
        self.failures
            .lock()
            .expect("khóa bộ đếm")
            .remove(&normalize(identifier));
    }

    /// Có phải lần sai vừa ghi làm số lần sai chạm ngưỡng thử thách không
    /// (dùng để gửi email cảnh báo đúng một lần mỗi đợt).
    pub fn crossed_threshold(&self, failures_after: usize) -> bool {
        failures_after == self.policy.challenge_after
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn delay_grows_and_is_capped() {
        let p = Policy::default();
        let ms: Vec<u128> = (0..8).map(|n| p.delay(n).as_millis()).collect();
        assert_eq!(ms, [0, 250, 500, 1000, 2000, 4000, 4000, 4000]);
        assert_eq!(p.delay(usize::MAX), p.max_delay);
    }

    /// Spec account-identity, scenario "Sai nhiều lần".
    #[test]
    fn challenge_after_five_failures_in_window() {
        let t = LoginThrottle::default();
        let now = Instant::now();
        for i in 1..=4 {
            assert_eq!(t.record_failure("lan@example.com", now), i);
            assert!(!t.status("lan@example.com", now).challenge_required);
        }
        let after = t.record_failure("lan@example.com", now);
        assert!(t.crossed_threshold(after));
        let s = t.status("lan@example.com", now);
        assert!(s.challenge_required);
        assert_eq!(s.failures, 5);
    }

    #[test]
    fn identifier_is_case_insensitive() {
        let t = LoginThrottle::default();
        let now = Instant::now();
        t.record_failure("Lan@Example.com ", now);
        assert_eq!(t.status("lan@example.com", now).failures, 1);
    }

    #[test]
    fn old_failures_leave_the_window() {
        let t = LoginThrottle::default();
        let start = Instant::now();
        for _ in 0..5 {
            t.record_failure("lan@example.com", start);
        }
        let later = start + Duration::from_secs(15 * 60);
        let s = t.status("lan@example.com", later);
        assert_eq!(s.failures, 0);
        assert!(!s.challenge_required);
    }

    #[test]
    fn success_resets_and_accounts_are_independent() {
        let t = LoginThrottle::default();
        let now = Instant::now();
        t.record_failure("lan@example.com", now);
        t.record_failure("minh@example.com", now);
        t.reset("LAN@example.com");
        assert_eq!(t.status("lan@example.com", now).failures, 0);
        assert_eq!(t.status("minh@example.com", now).failures, 1);
    }

    /// Spec: không khóa cứng tài khoản. Dù sai rất nhiều lần, thời gian chờ vẫn có
    /// trần và luôn có đường đăng nhập (qua thử thách).
    #[test]
    fn never_locks_out() {
        let t = LoginThrottle::default();
        let now = Instant::now();
        for _ in 0..1000 {
            t.record_failure("lan@example.com", now);
        }
        let s = t.status("lan@example.com", now);
        assert!(s.delay <= Duration::from_secs(4));
        assert!(s.challenge_required);
    }
}
