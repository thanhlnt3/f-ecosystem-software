use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

/// Môi trường chạy (design D11: dev, staging, production).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Environment {
    Dev,
    Staging,
    Production,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("môi trường không hợp lệ: {0:?} (chỉ nhận dev, staging, production)")]
pub struct ParseEnvironmentError(pub String);

impl Environment {
    pub const fn as_str(self) -> &'static str {
        match self {
            Environment::Dev => "dev",
            Environment::Staging => "staging",
            Environment::Production => "production",
        }
    }

    pub const fn is_production(self) -> bool {
        matches!(self, Environment::Production)
    }
}

impl fmt::Display for Environment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for Environment {
    type Err = ParseEnvironmentError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "dev" => Ok(Environment::Dev),
            "staging" => Ok(Environment::Staging),
            "production" => Ok(Environment::Production),
            other => Err(ParseEnvironmentError(other.to_owned())),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_known_environments_only() {
        assert_eq!("dev".parse::<Environment>().unwrap(), Environment::Dev);
        assert_eq!(
            "staging".parse::<Environment>().unwrap(),
            Environment::Staging
        );
        assert_eq!(
            "production".parse::<Environment>().unwrap(),
            Environment::Production
        );
        assert!(
            "prod".parse::<Environment>().is_err(),
            "không đoán tên viết tắt"
        );
        assert!("".parse::<Environment>().is_err());
    }
}
