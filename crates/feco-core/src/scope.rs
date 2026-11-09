use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Scope dữ liệu người dùng mà app con có thể xin (spec `user-data-consent`,
/// "Danh mục scope dữ liệu"). Danh mục này cố định: app con không truy cập được
/// dữ liệu nằm ngoài các scope dưới đây.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Scope {
    /// Tên hiển thị và ảnh đại diện.
    Profile,
    Email,
    /// Ngôn ngữ và múi giờ.
    Locale,
    Phone,
    /// Thông tin xuất hóa đơn.
    Billing,
    /// Quyền sử dụng của chính app đó.
    Purchases,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("scope không tồn tại: {0:?}")]
pub struct ParseScopeError(pub String);

impl Scope {
    pub const ALL: [Scope; 6] = [
        Scope::Profile,
        Scope::Email,
        Scope::Locale,
        Scope::Phone,
        Scope::Billing,
        Scope::Purchases,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Scope::Profile => "profile",
            Scope::Email => "email",
            Scope::Locale => "locale",
            Scope::Phone => "phone",
            Scope::Billing => "billing",
            Scope::Purchases => "purchases",
        }
    }

    /// Mô tả hiện trên màn hình đồng ý, tiếng Việt.
    pub const fn description_vi(self) -> &'static str {
        match self {
            Scope::Profile => "Tên hiển thị và ảnh đại diện",
            Scope::Email => "Địa chỉ email",
            Scope::Locale => "Ngôn ngữ và múi giờ",
            Scope::Phone => "Số điện thoại",
            Scope::Billing => "Thông tin xuất hóa đơn (tên, mã số thuế, địa chỉ)",
            Scope::Purchases => "Quyền sử dụng của bạn với app này",
        }
    }

    /// Mô tả hiện trên màn hình đồng ý, tiếng Anh.
    pub const fn description_en(self) -> &'static str {
        match self {
            Scope::Profile => "Display name and profile picture",
            Scope::Email => "Email address",
            Scope::Locale => "Language and time zone",
            Scope::Phone => "Phone number",
            Scope::Billing => "Billing details (name, tax code, address)",
            Scope::Purchases => "Your entitlements for this app",
        }
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Scope {
    type Err = ParseScopeError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Scope::ALL
            .into_iter()
            .find(|scope| scope.as_str() == s)
            .ok_or_else(|| ParseScopeError(s.to_owned()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_scope() {
        for scope in Scope::ALL {
            assert_eq!(scope.as_str().parse::<Scope>().unwrap(), scope);
            let json = serde_json::to_string(&scope).unwrap();
            assert_eq!(json, format!("\"{}\"", scope.as_str()));
        }
    }

    #[test]
    fn unknown_scope_is_rejected() {
        assert!("contacts".parse::<Scope>().is_err());
        assert!(
            "Profile".parse::<Scope>().is_err(),
            "phân biệt hoa thường như OAuth"
        );
        assert!(serde_json::from_str::<Scope>("\"location\"").is_err());
    }

    #[test]
    fn every_scope_has_both_descriptions() {
        for scope in Scope::ALL {
            assert!(!scope.description_vi().is_empty());
            assert!(!scope.description_en().is_empty());
        }
    }
}
