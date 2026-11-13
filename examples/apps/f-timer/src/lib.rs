//! Logic của app Bấm giờ: đồng hồ bấm giờ có vòng, và hẹn giờ tập trung (làm việc rồi nghỉ).
//! Thời gian được truyền vào dưới dạng [`Duration`] kể từ một mốc, nên kiểm thử không cần chờ.

use serde::{Deserialize, Serialize};
use std::path::Path;
use std::time::Duration;

/// Đồng hồ bấm giờ. `now` là thời điểm hiện tại tính từ một mốc cố định bất kỳ.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Stopwatch {
    /// Thời gian đã chạy trước lần bấm Bắt đầu gần nhất.
    banked: Duration,
    /// Thời điểm bấm Bắt đầu (đang chạy).
    started: Option<Duration>,
    /// Các vòng: thời gian của từng vòng.
    pub laps: Vec<Duration>,
}

impl Stopwatch {
    pub fn running(&self) -> bool {
        self.started.is_some()
    }

    pub fn elapsed(&self, now: Duration) -> Duration {
        self.banked
            + self
                .started
                .map_or(Duration::ZERO, |s| now.saturating_sub(s))
    }

    pub fn start(&mut self, now: Duration) {
        if self.started.is_none() {
            self.started = Some(now);
        }
    }

    pub fn pause(&mut self, now: Duration) {
        if let Some(s) = self.started.take() {
            self.banked += now.saturating_sub(s);
        }
    }

    /// Ghi một vòng: thời gian kể từ vòng trước.
    pub fn lap(&mut self, now: Duration) {
        let total = self.elapsed(now);
        let before: Duration = self.laps.iter().sum();
        self.laps.push(total.saturating_sub(before));
    }

    pub fn reset(&mut self) {
        *self = Self::default();
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Phase {
    Focus,
    Break,
}

impl Phase {
    pub const fn label(self) -> &'static str {
        match self {
            Phase::Focus => "Tập trung",
            Phase::Break => "Nghỉ",
        }
    }
}

/// Hẹn giờ tập trung: hết giờ làm việc thì sang giờ nghỉ và ngược lại.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FocusTimer {
    pub focus: Duration,
    pub rest: Duration,
    pub phase: Phase,
    clock: Stopwatch,
}

impl FocusTimer {
    pub fn new(focus_minutes: u64, rest_minutes: u64) -> Self {
        Self {
            focus: Duration::from_secs(focus_minutes.max(1) * 60),
            rest: Duration::from_secs(rest_minutes.max(1) * 60),
            phase: Phase::Focus,
            clock: Stopwatch::default(),
        }
    }

    fn length(&self) -> Duration {
        match self.phase {
            Phase::Focus => self.focus,
            Phase::Break => self.rest,
        }
    }

    pub fn running(&self) -> bool {
        self.clock.running()
    }

    pub fn start(&mut self, now: Duration) {
        self.clock.start(now);
    }

    pub fn pause(&mut self, now: Duration) {
        self.clock.pause(now);
    }

    pub fn remaining(&self, now: Duration) -> Duration {
        self.length().saturating_sub(self.clock.elapsed(now))
    }

    /// Gọi mỗi khung hình. Hết giờ thì chuyển pha, dừng chờ người dùng bấm tiếp, và trả pha vừa
    /// xong (để đếm số phiên tập trung).
    pub fn tick(&mut self, now: Duration) -> Option<Phase> {
        if !self.running() || self.remaining(now) > Duration::ZERO {
            return None;
        }
        let done = self.phase;
        self.phase = match done {
            Phase::Focus => Phase::Break,
            Phase::Break => Phase::Focus,
        };
        self.clock.reset();
        Some(done)
    }

    pub fn reset(&mut self) {
        self.phase = Phase::Focus;
        self.clock.reset();
    }
}

/// `mm:ss`, hoặc `h:mm:ss` từ một giờ trở lên; kèm phần trăm giây khi `centis`.
pub fn format(d: Duration, centis: bool) -> String {
    let secs = d.as_secs();
    let (h, m, s) = (secs / 3600, (secs / 60) % 60, secs % 60);
    let base = if h > 0 {
        format!("{h}:{m:02}:{s:02}")
    } else {
        format!("{m:02}:{s:02}")
    };
    if centis {
        format!("{base}.{:02}", d.subsec_millis() / 10)
    } else {
        base
    }
}

/// Số phiên tập trung đã hoàn thành, lưu trong `stats.json` ở thư mục dữ liệu của app.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Stats {
    pub focus_sessions: u64,
}

impl Stats {
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join("stats.json"))
            .ok()
            .and_then(|b| serde_json::from_slice(&b).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        feco_sample_common::write_atomic(&dir.join("stats.json"), &bytes)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn s(secs: u64) -> Duration {
        Duration::from_secs(secs)
    }

    #[test]
    fn stopwatch_pause_resume_and_laps() {
        let mut w = Stopwatch::default();
        w.start(s(10));
        assert_eq!(w.elapsed(s(15)), s(5));
        w.lap(s(15));
        w.pause(s(20));
        assert_eq!(w.elapsed(s(100)), s(10), "dừng thì không chạy tiếp");
        w.start(s(100));
        w.lap(s(103));
        assert_eq!(w.laps, [s(5), s(8)]);
        assert_eq!(w.elapsed(s(103)), s(13));
        w.reset();
        assert_eq!(w.elapsed(s(200)), Duration::ZERO);
        assert!(!w.running());
    }

    #[test]
    fn focus_timer_switches_phase_when_done() {
        let mut t = FocusTimer::new(25, 5);
        assert_eq!(t.tick(s(0)), None, "chưa chạy");
        t.start(s(0));
        assert_eq!(t.remaining(s(60)), s(24 * 60));
        assert_eq!(t.tick(s(60)), None);
        assert_eq!(t.tick(s(25 * 60)), Some(Phase::Focus));
        assert_eq!(t.phase, Phase::Break);
        assert!(!t.running(), "chờ người dùng bấm tiếp");
        assert_eq!(t.remaining(s(9999)), s(5 * 60));
        t.start(s(2000));
        assert_eq!(t.tick(s(2000 + 5 * 60)), Some(Phase::Break));
        assert_eq!(t.phase, Phase::Focus);
    }

    #[test]
    fn formatting() {
        assert_eq!(format(s(65), false), "01:05");
        assert_eq!(format(s(3725), false), "1:02:05");
        assert_eq!(format(Duration::from_millis(1_234), true), "00:01.23");
    }

    #[test]
    fn stats_round_trip() {
        let dir = std::env::temp_dir().join(format!("f-timer-test-{}", std::process::id()));
        assert_eq!(Stats::load(&dir), Stats::default());
        Stats { focus_sessions: 3 }.save(&dir).unwrap();
        assert_eq!(Stats::load(&dir).focus_sessions, 3);
        let _ = std::fs::remove_dir_all(dir);
    }
}
