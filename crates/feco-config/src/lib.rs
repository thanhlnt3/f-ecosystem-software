//! Cấu hình dịch vụ F-Eco, đọc từ biến môi trường và kiểm tra lúc khởi động.
//!
//! Spec `payments`, "Môi trường thanh toán thử": môi trường dev và staging chỉ được
//! dùng sandbox của cổng thanh toán; khóa thật chỉ có ở production. Vì mã merchant
//! và khóa của sandbox và production trông giống hệt nhau, quy tắc được kiểm tra
//! qua địa chỉ endpoint mà cấu hình trỏ tới: dev/staging phải trỏ tới host sandbox,
//! production phải trỏ tới host thật, qua HTTPS.
//!
//! Spec `platform-security`: bí mật không được xuất hiện trong nhật ký, nên mọi bí mật
//! được bọc trong [`Secret`], kiểu này in ra `***` khi debug.

use std::{collections::HashMap, fmt, net::SocketAddr};

use feco_core::Environment;
use url::Url;

/// Danh sách host của cổng thanh toán. Cần đối chiếu lại với tài liệu chính thức của
/// từng cổng khi tích hợp thật (tasks.md 13.2).
pub mod payment_hosts {
    pub const VNPAY_SANDBOX: &[&str] = &["sandbox.vnpayment.vn"];
    pub const VNPAY_PRODUCTION: &[&str] = &["pay.vnpay.vn"];
    pub const MOMO_SANDBOX: &[&str] = &["test-payment.momo.vn"];
    pub const MOMO_PRODUCTION: &[&str] = &["payment.momo.vn"];
}

/// Giá trị bí mật: không bao giờ in ra qua `Debug` hay `Display`.
#[derive(Clone, PartialEq, Eq)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Secret(value.into())
    }

    /// Lấy giá trị thật. Chỉ gọi ở chỗ thực sự dùng bí mật (ký yêu cầu, kết nối).
    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Secret(***)")
    }
}

#[derive(Debug, Clone)]
pub struct VnpayConfig {
    pub tmn_code: String,
    pub hash_secret: Secret,
    pub payment_url: Url,
}

#[derive(Debug, Clone)]
pub struct MomoConfig {
    pub partner_code: String,
    pub access_key: Secret,
    pub secret_key: Secret,
    pub endpoint: Url,
}

#[derive(Debug, Clone, Default)]
pub struct PaymentsConfig {
    pub vnpay: Option<VnpayConfig>,
    pub momo: Option<MomoConfig>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub http_addr: SocketAddr,
    pub database_url: Option<Secret>,
    pub payments: PaymentsConfig,
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ConfigError {
    #[error("thiếu biến môi trường bắt buộc: {0}")]
    Missing(&'static str),
    #[error("biến {var} không hợp lệ: {reason}")]
    Invalid { var: &'static str, reason: String },
    #[error("cấu hình {gateway} chưa đủ: thiếu {missing:?} (phải đặt đủ tất cả hoặc không đặt biến nào)")]
    Incomplete {
        gateway: &'static str,
        missing: Vec<&'static str>,
    },
    #[error(
        "{gateway}: môi trường {environment} chỉ được dùng sandbox, nhưng {var} trỏ tới {host:?}. \
         Khóa thật của cổng thanh toán chỉ dùng ở production."
    )]
    ProductionGatewayOutsideProduction {
        gateway: &'static str,
        environment: Environment,
        var: &'static str,
        host: String,
    },
    #[error("{gateway}: production phải trỏ tới endpoint thật, nhưng {var} trỏ tới {host:?}")]
    NonProductionGatewayInProduction {
        gateway: &'static str,
        var: &'static str,
        host: String,
    },
    #[error("{var} phải dùng https, nhận {scheme:?}")]
    InsecureScheme { var: &'static str, scheme: String },
}

const DEFAULT_HTTP_ADDR: &str = "127.0.0.1:8080";

impl Config {
    /// Đọc từ biến môi trường của tiến trình.
    pub fn from_env() -> Result<Config, ConfigError> {
        Config::from_vars(&std::env::vars().collect())
    }

    /// Đọc từ một bảng biến (dùng cho test).
    pub fn from_vars(vars: &HashMap<String, String>) -> Result<Config, ConfigError> {
        let get = |name: &str| vars.get(name).map(|s| s.trim()).filter(|s| !s.is_empty());

        let environment = get("FECO_ENV")
            .ok_or(ConfigError::Missing("FECO_ENV"))?
            .parse::<Environment>()
            .map_err(|e| ConfigError::Invalid {
                var: "FECO_ENV",
                reason: e.to_string(),
            })?;

        let http_addr = get("FECO_HTTP_ADDR")
            .unwrap_or(DEFAULT_HTTP_ADDR)
            .parse::<SocketAddr>()
            .map_err(|e| ConfigError::Invalid {
                var: "FECO_HTTP_ADDR",
                reason: e.to_string(),
            })?;

        let database_url = get("FECO_DATABASE_URL").map(Secret::new);

        let vnpay = group(
            "VNPAY",
            [
                ("FECO_VNPAY_TMN_CODE", get("FECO_VNPAY_TMN_CODE")),
                ("FECO_VNPAY_HASH_SECRET", get("FECO_VNPAY_HASH_SECRET")),
                ("FECO_VNPAY_PAYMENT_URL", get("FECO_VNPAY_PAYMENT_URL")),
            ],
        )?
        .map(|[tmn, secret, url]| -> Result<VnpayConfig, ConfigError> {
            Ok(VnpayConfig {
                tmn_code: tmn.to_owned(),
                hash_secret: Secret::new(secret),
                payment_url: parse_url("FECO_VNPAY_PAYMENT_URL", url)?,
            })
        })
        .transpose()?;

        let momo = group(
            "MoMo",
            [
                ("FECO_MOMO_PARTNER_CODE", get("FECO_MOMO_PARTNER_CODE")),
                ("FECO_MOMO_ACCESS_KEY", get("FECO_MOMO_ACCESS_KEY")),
                ("FECO_MOMO_SECRET_KEY", get("FECO_MOMO_SECRET_KEY")),
                ("FECO_MOMO_ENDPOINT", get("FECO_MOMO_ENDPOINT")),
            ],
        )?
        .map(
            |[partner, access, secret, endpoint]| -> Result<MomoConfig, ConfigError> {
                Ok(MomoConfig {
                    partner_code: partner.to_owned(),
                    access_key: Secret::new(access),
                    secret_key: Secret::new(secret),
                    endpoint: parse_url("FECO_MOMO_ENDPOINT", endpoint)?,
                })
            },
        )
        .transpose()?;

        let config = Config {
            environment,
            http_addr,
            database_url,
            payments: PaymentsConfig { vnpay, momo },
        };
        config.validate()?;
        Ok(config)
    }

    /// Kiểm tra quy tắc giữa môi trường và cổng thanh toán.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(v) = &self.payments.vnpay {
            check_gateway(
                self.environment,
                "VNPAY",
                "FECO_VNPAY_PAYMENT_URL",
                &v.payment_url,
                payment_hosts::VNPAY_SANDBOX,
                payment_hosts::VNPAY_PRODUCTION,
            )?;
        }
        if let Some(m) = &self.payments.momo {
            check_gateway(
                self.environment,
                "MoMo",
                "FECO_MOMO_ENDPOINT",
                &m.endpoint,
                payment_hosts::MOMO_SANDBOX,
                payment_hosts::MOMO_PRODUCTION,
            )?;
        }
        Ok(())
    }
}

/// Một nhóm biến của một cổng: hoặc đặt đủ, hoặc không đặt biến nào.
fn group<'a, const N: usize>(
    gateway: &'static str,
    vars: [(&'static str, Option<&'a str>); N],
) -> Result<Option<[&'a str; N]>, ConfigError> {
    let missing: Vec<&'static str> = vars
        .iter()
        .filter(|(_, v)| v.is_none())
        .map(|(n, _)| *n)
        .collect();
    if missing.len() == N {
        return Ok(None);
    }
    if !missing.is_empty() {
        return Err(ConfigError::Incomplete { gateway, missing });
    }
    Ok(Some(vars.map(|(_, v)| v.unwrap_or_default())))
}

fn parse_url(var: &'static str, raw: &str) -> Result<Url, ConfigError> {
    let url = Url::parse(raw).map_err(|e| ConfigError::Invalid {
        var,
        reason: e.to_string(),
    })?;
    if url.scheme() != "https" {
        return Err(ConfigError::InsecureScheme {
            var,
            scheme: url.scheme().to_owned(),
        });
    }
    if url.host_str().is_none() {
        return Err(ConfigError::Invalid {
            var,
            reason: "thiếu host".into(),
        });
    }
    Ok(url)
}

fn check_gateway(
    environment: Environment,
    gateway: &'static str,
    var: &'static str,
    url: &Url,
    sandbox_hosts: &[&str],
    production_hosts: &[&str],
) -> Result<(), ConfigError> {
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    if environment.is_production() {
        if !production_hosts.contains(&host.as_str()) {
            return Err(ConfigError::NonProductionGatewayInProduction { gateway, var, host });
        }
    } else if !sandbox_hosts.contains(&host.as_str()) {
        // Mọi host khác sandbox đều bị coi là thật: an toàn hơn là đoán.
        return Err(ConfigError::ProductionGatewayOutsideProduction {
            gateway,
            environment,
            var,
            host,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vars(pairs: &[(&str, &str)]) -> HashMap<String, String> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    const VNPAY_SANDBOX: &[(&str, &str)] = &[
        ("FECO_VNPAY_TMN_CODE", "DEMO0001"),
        ("FECO_VNPAY_HASH_SECRET", "sandbox-secret"),
        (
            "FECO_VNPAY_PAYMENT_URL",
            "https://sandbox.vnpayment.vn/paymentv2/vpcpay.html",
        ),
    ];
    const VNPAY_LIVE: &[(&str, &str)] = &[
        ("FECO_VNPAY_TMN_CODE", "LIVE0001"),
        ("FECO_VNPAY_HASH_SECRET", "live-secret"),
        ("FECO_VNPAY_PAYMENT_URL", "https://pay.vnpay.vn/vpcpay.html"),
    ];
    const MOMO_LIVE: &[(&str, &str)] = &[
        ("FECO_MOMO_PARTNER_CODE", "MOMOLIVE"),
        ("FECO_MOMO_ACCESS_KEY", "live-access"),
        ("FECO_MOMO_SECRET_KEY", "live-secret"),
        (
            "FECO_MOMO_ENDPOINT",
            "https://payment.momo.vn/v2/gateway/api/create",
        ),
    ];

    fn with_env(env: &str, extra: &[(&str, &str)]) -> HashMap<String, String> {
        let mut v = vars(extra);
        v.insert("FECO_ENV".into(), env.into());
        v
    }

    #[test]
    fn minimal_dev_config_loads() {
        let c = Config::from_vars(&with_env("dev", &[])).unwrap();
        assert_eq!(c.environment, Environment::Dev);
        assert_eq!(c.http_addr.to_string(), DEFAULT_HTTP_ADDR);
        assert!(c.payments.vnpay.is_none() && c.payments.momo.is_none());
    }

    #[test]
    fn missing_environment_is_an_error() {
        assert_eq!(
            Config::from_vars(&vars(&[])).unwrap_err(),
            ConfigError::Missing("FECO_ENV")
        );
        assert!(matches!(
            Config::from_vars(&with_env("prod", &[])).unwrap_err(),
            ConfigError::Invalid {
                var: "FECO_ENV",
                ..
            }
        ));
    }

    /// Spec payments, scenario "Staging dùng khóa thật".
    #[test]
    fn staging_with_production_vnpay_is_rejected() {
        let err = Config::from_vars(&with_env("staging", VNPAY_LIVE)).unwrap_err();
        assert_eq!(
            err,
            ConfigError::ProductionGatewayOutsideProduction {
                gateway: "VNPAY",
                environment: Environment::Staging,
                var: "FECO_VNPAY_PAYMENT_URL",
                host: "pay.vnpay.vn".into(),
            }
        );
        assert!(err.to_string().contains("chỉ được dùng sandbox"));
    }

    #[test]
    fn dev_with_production_momo_is_rejected() {
        assert!(matches!(
            Config::from_vars(&with_env("dev", MOMO_LIVE)).unwrap_err(),
            ConfigError::ProductionGatewayOutsideProduction {
                gateway: "MoMo",
                ..
            }
        ));
    }

    #[test]
    fn unknown_host_outside_production_is_treated_as_live() {
        let mut v = with_env("staging", VNPAY_SANDBOX);
        v.insert(
            "FECO_VNPAY_PAYMENT_URL".into(),
            "https://vnpay.example.com/pay".into(),
        );
        assert!(matches!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::ProductionGatewayOutsideProduction { .. }
        ));
    }

    #[test]
    fn staging_with_sandbox_is_accepted() {
        let c = Config::from_vars(&with_env("staging", VNPAY_SANDBOX)).unwrap();
        assert_eq!(c.payments.vnpay.unwrap().tmn_code, "DEMO0001");
    }

    #[test]
    fn production_with_sandbox_is_rejected() {
        assert!(matches!(
            Config::from_vars(&with_env("production", VNPAY_SANDBOX)).unwrap_err(),
            ConfigError::NonProductionGatewayInProduction {
                gateway: "VNPAY",
                ..
            }
        ));
    }

    #[test]
    fn production_with_live_gateways_is_accepted() {
        let mut pairs = VNPAY_LIVE.to_vec();
        pairs.extend_from_slice(MOMO_LIVE);
        let c = Config::from_vars(&with_env("production", &pairs)).unwrap();
        assert!(c.payments.vnpay.is_some() && c.payments.momo.is_some());
    }

    #[test]
    fn partial_gateway_config_is_rejected() {
        let err = Config::from_vars(&with_env("dev", &[("FECO_VNPAY_TMN_CODE", "DEMO0001")]))
            .unwrap_err();
        assert_eq!(
            err,
            ConfigError::Incomplete {
                gateway: "VNPAY",
                missing: vec!["FECO_VNPAY_HASH_SECRET", "FECO_VNPAY_PAYMENT_URL"],
            }
        );
    }

    #[test]
    fn plain_http_endpoint_is_rejected() {
        let mut v = with_env("dev", VNPAY_SANDBOX);
        v.insert(
            "FECO_VNPAY_PAYMENT_URL".into(),
            "http://sandbox.vnpayment.vn/paymentv2/vpcpay.html".into(),
        );
        assert!(matches!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::InsecureScheme { .. }
        ));
    }

    /// Spec platform-security: bí mật không xuất hiện trong nhật ký.
    #[test]
    fn secrets_never_appear_in_debug_output() {
        let mut v = with_env("staging", VNPAY_SANDBOX);
        v.insert(
            "FECO_DATABASE_URL".into(),
            "postgres://feco:db-password@localhost/feco".into(),
        );
        let c = Config::from_vars(&v).unwrap();
        let dump = format!("{c:?}");
        assert!(!dump.contains("sandbox-secret"), "{dump}");
        assert!(!dump.contains("db-password"), "{dump}");
        assert!(dump.contains("Secret(***)"));
    }
}
