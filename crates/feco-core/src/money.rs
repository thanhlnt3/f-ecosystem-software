use std::fmt;

use serde::{Deserialize, Serialize};

/// Số tiền VND. Đồng Việt Nam không có đơn vị lẻ, nên luôn lưu bằng số nguyên
/// (spec `payments`: "số tiền là số nguyên").
///
/// Dùng `u64` vì số tiền của một giao dịch, mốc hay lệnh hoàn không bao giờ âm;
/// mọi phép trừ đều được kiểm tra để không xuống dưới 0.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default, Serialize, Deserialize,
)]
#[serde(transparent)]
pub struct Vnd(u64);

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum MoneyError {
    #[error("số tiền vượt quá giới hạn")]
    Overflow,
    #[error("kết quả âm: {minuend} − {subtrahend}")]
    Negative { minuend: Vnd, subtrahend: Vnd },
    #[error("tỉ lệ phần trăm phải từ 0 đến 100, nhận {0}")]
    InvalidPercent(u8),
}

impl Vnd {
    pub const ZERO: Vnd = Vnd(0);

    pub const fn new(amount: u64) -> Self {
        Vnd(amount)
    }

    pub const fn amount(self) -> u64 {
        self.0
    }

    pub fn checked_add(self, other: Vnd) -> Result<Vnd, MoneyError> {
        self.0
            .checked_add(other.0)
            .map(Vnd)
            .ok_or(MoneyError::Overflow)
    }

    /// Trừ, báo lỗi nếu kết quả âm (ví dụ hoàn tiền vượt số đã thu).
    pub fn checked_sub(self, other: Vnd) -> Result<Vnd, MoneyError> {
        self.0
            .checked_sub(other.0)
            .map(Vnd)
            .ok_or(MoneyError::Negative {
                minuend: self,
                subtrahend: other,
            })
    }

    /// Tổng của nhiều khoản (ví dụ các mốc của một báo giá).
    pub fn sum<I: IntoIterator<Item = Vnd>>(items: I) -> Result<Vnd, MoneyError> {
        items.into_iter().try_fold(Vnd::ZERO, Vnd::checked_add)
    }

    /// `percent`% của số tiền, làm tròn xuống tới đồng.
    pub fn percent(self, percent: u8) -> Result<Vnd, MoneyError> {
        if percent > 100 {
            return Err(MoneyError::InvalidPercent(percent));
        }
        // u64 × 100 có thể tràn với số rất lớn, nên tính trên u128.
        let v = (u128::from(self.0) * u128::from(percent)) / 100;
        u64::try_from(v).map(Vnd).map_err(|_| MoneyError::Overflow)
    }
}

/// Hiển thị theo định dạng Việt Nam: `9.000.000 ₫` (spec `platform-support`, "Ngôn ngữ").
impl fmt::Display for Vnd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let digits = self.0.to_string();
        let mut out = String::with_capacity(digits.len() + digits.len() / 3 + 4);
        for (i, ch) in digits.chars().enumerate() {
            if i > 0 && (digits.len() - i) % 3 == 0 {
                out.push('.');
            }
            out.push(ch);
        }
        write!(f, "{out} ₫")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_vietnamese_style() {
        assert_eq!(Vnd::new(9_000_000).to_string(), "9.000.000 ₫");
        assert_eq!(Vnd::new(30_000_000).to_string(), "30.000.000 ₫");
        assert_eq!(Vnd::new(999).to_string(), "999 ₫");
        assert_eq!(Vnd::new(1_000).to_string(), "1.000 ₫");
        assert_eq!(Vnd::ZERO.to_string(), "0 ₫");
    }

    #[test]
    fn quote_milestones_sum() {
        let total = Vnd::sum([
            Vnd::new(9_000_000),
            Vnd::new(12_000_000),
            Vnd::new(9_000_000),
        ])
        .unwrap();
        assert_eq!(total, Vnd::new(30_000_000));
    }

    #[test]
    fn deposit_cap_is_thirty_percent() {
        // Spec custom-build-orders: đặt cọc không vượt quá 30% tổng giá.
        assert_eq!(
            Vnd::new(30_000_000).percent(30).unwrap(),
            Vnd::new(9_000_000)
        );
        assert_eq!(
            Vnd::new(10).percent(33).unwrap(),
            Vnd::new(3),
            "làm tròn xuống"
        );
        assert_eq!(
            Vnd::new(1).percent(101),
            Err(MoneyError::InvalidPercent(101))
        );
    }

    #[test]
    fn refund_cannot_exceed_collected() {
        // Spec payments: tổng số tiền hoàn không vượt số tiền đã thu.
        let collected = Vnd::new(12_000_000);
        assert_eq!(
            collected.checked_sub(Vnd::new(4_000_000)).unwrap(),
            Vnd::new(8_000_000)
        );
        assert!(matches!(
            collected.checked_sub(Vnd::new(13_000_000)),
            Err(MoneyError::Negative { .. })
        ));
    }

    #[test]
    fn overflow_is_reported() {
        assert_eq!(
            Vnd::new(u64::MAX).checked_add(Vnd::new(1)),
            Err(MoneyError::Overflow)
        );
        assert_eq!(Vnd::new(u64::MAX).percent(100).unwrap(), Vnd::new(u64::MAX));
    }

    #[test]
    fn serializes_as_plain_integer() {
        assert_eq!(
            serde_json::to_string(&Vnd::new(9_000_000)).unwrap(),
            "9000000"
        );
        assert_eq!(
            serde_json::from_str::<Vnd>("12000000").unwrap(),
            Vnd::new(12_000_000)
        );
        assert!(serde_json::from_str::<Vnd>("-5").is_err());
        assert!(serde_json::from_str::<Vnd>("1.5").is_err());
    }
}
