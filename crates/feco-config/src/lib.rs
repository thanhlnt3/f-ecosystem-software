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

use std::{collections::HashMap, fmt, net::SocketAddr, path::PathBuf};

use feco_core::Environment;
use url::Url;

/// Danh sách host của cổng thanh toán. Cần đối chiếu lại với tài liệu chính thức của
/// từng cổng khi tích hợp thật (tasks.md 13.2).
pub mod payment_hosts {
    pub const VNPAY_SANDBOX: &[&str] = &["sandbox.vnpayment.vn"];
    pub const VNPAY_PRODUCTION: &[&str] = &["pay.vnpay.vn"];
    pub const MOMO_SANDBOX: &[&str] = &["test-payment.momo.vn"];
    pub const MOMO_PRODUCTION: &[&str] = &["payment.momo.vn"];
    /// ZaloPay API v2 (docs.zalopay.vn): sandbox và production.
    pub const ZALOPAY_SANDBOX: &[&str] = &["sb-openapi.zalopay.vn"];
    pub const ZALOPAY_PRODUCTION: &[&str] = &["openapi.zalopay.vn"];
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
    /// API truy vấn và hoàn tiền (`merchant_webapi/api/transaction`); mặc định cùng host với
    /// trang thanh toán (`FECO_VNPAY_API_URL` để đổi).
    pub api_url: Url,
}

#[derive(Debug, Clone)]
pub struct MomoConfig {
    pub partner_code: String,
    pub access_key: Secret,
    pub secret_key: Secret,
    pub endpoint: Url,
}

/// ZaloPay API v2: `key1` ký yêu cầu gửi đi, `key2` kiểm chữ ký callback.
#[derive(Debug, Clone)]
pub struct ZalopayConfig {
    pub app_id: i64,
    pub key1: Secret,
    pub key2: Secret,
    /// Gốc API, ví dụ `https://sb-openapi.zalopay.vn/`.
    pub endpoint: Url,
}

/// MISA meInvoice (hóa đơn điện tử, tasks.md 13.4). Môi trường thử hay thật do tài khoản quyết
/// định (cùng một địa chỉ API), nên không kiểm theo host như cổng thanh toán.
#[derive(Debug, Clone)]
pub struct MisaConfig {
    pub client_id: String,
    pub client_secret: Secret,
    /// Mã số thuế của F-Eco (người bán).
    pub tax_code: String,
    pub username: String,
    pub password: Secret,
    /// Ký hiệu hóa đơn đã đăng ký, ví dụ `1C26TFE`.
    pub inv_series: String,
    /// Thuế suất GTGT trên hóa đơn: KCT, KKKNT, 0%, 5%, 8%, 10% hoặc KHAC:x%.
    pub vat_rate: String,
    pub endpoint: Url,
}

#[derive(Debug, Clone, Default)]
pub struct PaymentsConfig {
    pub vnpay: Option<VnpayConfig>,
    pub momo: Option<MomoConfig>,
    pub zalopay: Option<ZalopayConfig>,
    /// Gốc địa chỉ công khai mà cổng thanh toán gọi callback và IPN (mặc định là
    /// `FECO_PUBLIC_URL`). Dùng khi máy chủ chạy sau tunnel hoặc proxy khác địa chỉ đăng nhập.
    pub callback_base: Option<Url>,
    pub misa: Option<MisaConfig>,
}

/// Nhà cung cấp thử thách chống bot (spec account-identity, "Chống dò mật khẩu").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChallengeProvider {
    /// Thử thách giả cho dev và test: một ô đánh dấu. Bị cấm ở production.
    Test,
}

/// Cấu hình đăng nhập và tài khoản (spec account-identity).
#[derive(Debug, Clone)]
pub struct AuthConfig {
    /// Địa chỉ công khai của chính feco-server (để tạo liên kết trong email).
    pub public_url: Url,
    /// API công khai của Ory Kratos.
    pub kratos_public_url: Url,
    /// API quản trị của Ory Kratos (chỉ gọi từ mạng nội bộ).
    pub kratos_admin_url: Url,
    /// Máy chủ SMTP gửi email cảnh báo, ví dụ `smtp://127.0.0.1:1025`.
    pub smtp_url: Url,
    pub email_from: String,
    pub challenge: ChallengeProvider,
    /// API công khai của Ory Hydra (OAuth 2.0 / OIDC cho app con).
    pub hydra_public_url: Url,
    /// API quản trị của Ory Hydra (chỉ gọi từ mạng nội bộ).
    pub hydra_admin_url: Url,
    /// Khóa tạo định danh người dùng riêng cho từng app (spec user-data-consent).
    pub pairwise_secret: Secret,
    /// Client ID của F-Launcher trong Hydra (client công khai, bên thứ nhất). Chỉ token của
    /// client này được xin mã khởi chạy app con. Không đặt thì tắt tính năng mã khởi chạy.
    pub launcher_client_id: Option<String>,
}

/// Cấu hình bảo mật (spec platform-security).
#[derive(Debug, Clone)]
pub struct SecurityConfig {
    /// Khóa AES-256 mã hóa dữ liệu nhạy cảm khi lưu (32 byte, khai báo dạng base64).
    pub data_key: Secret,
    /// Số yêu cầu tối đa mỗi phút từ một địa chỉ IP (mọi trang và API).
    pub rate_ip_per_min: u32,
    /// Số lần gọi API dữ liệu người dùng tối đa mỗi phút của một app.
    pub rate_app_per_min: u32,
    /// Số lần gọi API dữ liệu tối đa mỗi phút cho một người dùng (mọi app cộng lại).
    pub rate_user_per_min: u32,
    /// `FECO_ADMIN_PASSKEY_OPTIONAL=true`: bảng quản trị không bắt buộc passkey. Chỉ được đặt khi
    /// `FECO_ENV=dev` (WebAuthn không chạy trên RP ID 127.0.0.1); môi trường khác từ chối khởi động.
    pub admin_passkey_optional: bool,
}

/// Kho TUF mà máy chủ ký target khi một phiên bản được phát hành (design D3). Bản dev dùng khóa
/// trong file; production dùng KMS (chưa chọn nhà cung cấp, xem QUESTIONS.md).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TufConfig {
    pub repo: PathBuf,
    /// File khóa targets, snapshot, timestamp.
    pub sign_keys: Vec<PathBuf>,
}

/// Cấu hình cổng nhà phát triển (spec developer-portal).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeveloperConfig {
    /// Máy chủ DNS dùng để xác minh tên miền bằng bản ghi TXT.
    pub dns_servers: Vec<SocketAddr>,
    /// Thư mục lưu gói nhà phát triển tải lên.
    pub package_dir: PathBuf,
    pub tuf: Option<TufConfig>,
    /// ClamAV (`clamd`), engine quét thứ hai cạnh engine chữ ký của F-Eco. Bắt buộc ở production.
    pub clamd: Option<SocketAddr>,
    /// Địa chỉ nhận cảnh báo kiểm duyệt (mã độc, phát hiện muộn).
    pub admin_emails: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct Config {
    pub environment: Environment,
    pub http_addr: SocketAddr,
    pub database_url: Secret,
    pub payments: PaymentsConfig,
    pub auth: AuthConfig,
    pub security: SecurityConfig,
    pub developer: DeveloperConfig,
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
    #[error(
        "production không được dùng thử thách chống bot thử nghiệm (FECO_CHALLENGE_PROVIDER=test)"
    )]
    TestChallengeInProduction,
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

        let database_url = match get("FECO_DATABASE_URL") {
            Some(v) => Secret::new(v),
            None if environment.is_production() => {
                return Err(ConfigError::Missing("FECO_DATABASE_URL"))
            }
            // Khớp dịch vụ postgres trong infra/docker-compose.yml.
            None => Secret::new("postgres://feco:feco-dev-only@127.0.0.1:5432/feco"),
        };

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
                payment_url: gateway_url(environment, "FECO_VNPAY_PAYMENT_URL", url)?,
                api_url: match get("FECO_VNPAY_API_URL") {
                    Some(api) => gateway_url(environment, "FECO_VNPAY_API_URL", api)?,
                    None => {
                        let mut u = gateway_url(environment, "FECO_VNPAY_PAYMENT_URL", url)?;
                        u.set_path("/merchant_webapi/api/transaction");
                        u.set_query(None);
                        u
                    }
                },
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
                    endpoint: gateway_url(environment, "FECO_MOMO_ENDPOINT", endpoint)?,
                })
            },
        )
        .transpose()?;

        let zalopay = group(
            "ZaloPay",
            [
                ("FECO_ZALOPAY_APP_ID", get("FECO_ZALOPAY_APP_ID")),
                ("FECO_ZALOPAY_KEY1", get("FECO_ZALOPAY_KEY1")),
                ("FECO_ZALOPAY_KEY2", get("FECO_ZALOPAY_KEY2")),
                ("FECO_ZALOPAY_ENDPOINT", get("FECO_ZALOPAY_ENDPOINT")),
            ],
        )?
        .map(
            |[app_id, key1, key2, endpoint]| -> Result<ZalopayConfig, ConfigError> {
                let mut endpoint = gateway_url(environment, "FECO_ZALOPAY_ENDPOINT", endpoint)?;
                if !endpoint.path().ends_with('/') {
                    let path = format!("{}/", endpoint.path());
                    endpoint.set_path(&path);
                }
                Ok(ZalopayConfig {
                    app_id: app_id.parse().map_err(|_| ConfigError::Invalid {
                        var: "FECO_ZALOPAY_APP_ID",
                        reason: "phải là số".into(),
                    })?,
                    key1: Secret::new(key1),
                    key2: Secret::new(key2),
                    endpoint,
                })
            },
        )
        .transpose()?;

        let callback_base = get("FECO_PAYMENT_CALLBACK_BASE")
            .map(|v| -> Result<Url, ConfigError> {
                let mut u = parse_url("FECO_PAYMENT_CALLBACK_BASE", v)?;
                if !u.path().ends_with('/') {
                    let path = format!("{}/", u.path());
                    u.set_path(&path);
                }
                Ok(u)
            })
            .transpose()?;

        let misa = group(
            "MISA meInvoice",
            [
                ("FECO_MISA_CLIENT_ID", get("FECO_MISA_CLIENT_ID")),
                ("FECO_MISA_CLIENT_SECRET", get("FECO_MISA_CLIENT_SECRET")),
                ("FECO_MISA_TAX_CODE", get("FECO_MISA_TAX_CODE")),
                ("FECO_MISA_USERNAME", get("FECO_MISA_USERNAME")),
                ("FECO_MISA_PASSWORD", get("FECO_MISA_PASSWORD")),
                ("FECO_MISA_INV_SERIES", get("FECO_MISA_INV_SERIES")),
                ("FECO_MISA_VAT_RATE", get("FECO_MISA_VAT_RATE")),
            ],
        )?
        .map(
            |[client_id, secret, tax_code, username, password, series, vat]| -> Result<MisaConfig, ConfigError> {
                let vat_ok = matches!(vat, "KCT" | "KKKNT" | "0%" | "5%" | "8%" | "10%")
                    || vat.strip_prefix("KHAC:").is_some_and(|r| r.ends_with('%'));
                if !vat_ok {
                    return Err(ConfigError::Invalid {
                        var: "FECO_MISA_VAT_RATE",
                        reason: "phải là KCT, KKKNT, 0%, 5%, 8%, 10% hoặc KHAC:x%".into(),
                    });
                }
                let mut endpoint = gateway_url(
                    environment,
                    "FECO_MISA_ENDPOINT",
                    get("FECO_MISA_ENDPOINT")
                        .unwrap_or("https://developer.misa.vn/apis/itg/meinvoice/"),
                )?;
                if !endpoint.path().ends_with('/') {
                    let path = format!("{}/", endpoint.path());
                    endpoint.set_path(&path);
                }
                Ok(MisaConfig {
                    client_id: client_id.to_owned(),
                    client_secret: Secret::new(secret),
                    tax_code: tax_code.to_owned(),
                    username: username.to_owned(),
                    password: Secret::new(password),
                    inv_series: series.to_owned(),
                    vat_rate: vat.to_owned(),
                    endpoint,
                })
            },
        )
        .transpose()?;

        let auth = auth_config(environment, &get)?;
        let security = security_config(environment, &get)?;
        let developer = developer_config(environment, &get)?;

        let config = Config {
            environment,
            http_addr,
            database_url,
            auth,
            security,
            developer,
            payments: PaymentsConfig {
                vnpay,
                momo,
                zalopay,
                callback_base,
                misa,
            },
        };
        config.validate()?;
        Ok(config)
    }

    /// Kiểm tra quy tắc giữa môi trường và cổng thanh toán.
    pub fn validate(&self) -> Result<(), ConfigError> {
        if let Some(v) = &self.payments.vnpay {
            for (var, url) in [
                ("FECO_VNPAY_PAYMENT_URL", &v.payment_url),
                ("FECO_VNPAY_API_URL", &v.api_url),
            ] {
                check_gateway(
                    self.environment,
                    "VNPAY",
                    var,
                    url,
                    payment_hosts::VNPAY_SANDBOX,
                    payment_hosts::VNPAY_PRODUCTION,
                )?;
            }
        }
        if let Some(z) = &self.payments.zalopay {
            check_gateway(
                self.environment,
                "ZaloPay",
                "FECO_ZALOPAY_ENDPOINT",
                &z.endpoint,
                payment_hosts::ZALOPAY_SANDBOX,
                payment_hosts::ZALOPAY_PRODUCTION,
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

/// Cấu hình tài khoản. Ngoài production có giá trị mặc định trỏ tới hạ tầng dev
/// (`infra/docker-compose.yml`); production bắt buộc khai báo đủ.
fn auth_config<'a>(
    environment: Environment,
    get: &impl Fn(&str) -> Option<&'a str>,
) -> Result<AuthConfig, ConfigError> {
    let prod = environment.is_production();
    let url_var = |var: &'static str, dev_default: &'static str| -> Result<Url, ConfigError> {
        let raw = match get(var) {
            Some(v) => v,
            None if prod => return Err(ConfigError::Missing(var)),
            None => dev_default,
        };
        Url::parse(raw).map_err(|e| ConfigError::Invalid {
            var,
            reason: e.to_string(),
        })
    };
    let public_url = url_var("FECO_PUBLIC_URL", "http://127.0.0.1:8080/")?;
    if prod && public_url.scheme() != "https" {
        return Err(ConfigError::InsecureScheme {
            var: "FECO_PUBLIC_URL",
            scheme: public_url.scheme().to_owned(),
        });
    }
    let challenge = match get("FECO_CHALLENGE_PROVIDER") {
        Some("test") => ChallengeProvider::Test,
        None if !prod => ChallengeProvider::Test,
        None => return Err(ConfigError::Missing("FECO_CHALLENGE_PROVIDER")),
        Some(other) => {
            return Err(ConfigError::Invalid {
                var: "FECO_CHALLENGE_PROVIDER",
                reason: format!("nhà cung cấp chưa được hỗ trợ: {other:?}"),
            })
        }
    };
    if prod && challenge == ChallengeProvider::Test {
        return Err(ConfigError::TestChallengeInProduction);
    }
    Ok(AuthConfig {
        public_url,
        kratos_public_url: url_var("FECO_KRATOS_PUBLIC_URL", "http://127.0.0.1:4433/")?,
        kratos_admin_url: url_var("FECO_KRATOS_ADMIN_URL", "http://127.0.0.1:4434/")?,
        smtp_url: url_var("FECO_SMTP_URL", "smtp://127.0.0.1:1025")?,
        hydra_public_url: url_var("FECO_HYDRA_PUBLIC_URL", "http://127.0.0.1:4444/")?,
        hydra_admin_url: url_var("FECO_HYDRA_ADMIN_URL", "http://127.0.0.1:4445/")?,
        pairwise_secret: match get("FECO_PAIRWISE_SECRET") {
            Some(v) if v.len() >= 32 => Secret::new(v),
            Some(_) => {
                return Err(ConfigError::Invalid {
                    var: "FECO_PAIRWISE_SECRET",
                    reason: "phải dài ít nhất 32 ký tự".into(),
                })
            }
            None if prod => return Err(ConfigError::Missing("FECO_PAIRWISE_SECRET")),
            None => Secret::new("dev-only-pairwise-secret-not-for-production"),
        },
        launcher_client_id: get("FECO_LAUNCHER_CLIENT_ID")
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .map(str::to_owned),
        email_from: get("FECO_EMAIL_FROM")
            .unwrap_or("F-Eco <no-reply@f-eco.example>")
            .to_owned(),
        challenge,
    })
}

fn security_config<'a>(
    environment: Environment,
    get: &impl Fn(&str) -> Option<&'a str>,
) -> Result<SecurityConfig, ConfigError> {
    let data_key = match get("FECO_DATA_KEY") {
        Some(v) => {
            let decoded = decode_base64(v).ok_or(ConfigError::Invalid {
                var: "FECO_DATA_KEY",
                reason: "không phải base64 hợp lệ".into(),
            })?;
            if decoded.len() != 32 {
                return Err(ConfigError::Invalid {
                    var: "FECO_DATA_KEY",
                    reason: format!("phải là 32 byte, nhận {}", decoded.len()),
                });
            }
            Secret::new(v)
        }
        None if environment.is_production() => return Err(ConfigError::Missing("FECO_DATA_KEY")),
        // Khóa cố định chỉ dùng cho dev: 32 byte "feco-dev-only-data-key-not-prod!".
        None => Secret::new("ZmVjby1kZXYtb25seS1kYXRhLWtleS1ub3QtcHJvZCE="), // gitleaks:allow (khóa dev công khai)
    };
    let num = |var: &'static str, default: u32| -> Result<u32, ConfigError> {
        match get(var) {
            None => Ok(default),
            Some(v) => v
                .parse::<u32>()
                .ok()
                .filter(|n| *n > 0)
                .ok_or(ConfigError::Invalid {
                    var,
                    reason: "phải là số nguyên dương".into(),
                }),
        }
    };
    let admin_passkey_optional = match get("FECO_ADMIN_PASSKEY_OPTIONAL") {
        None | Some("false" | "0" | "") => false,
        Some("true" | "1") if environment == Environment::Dev => true,
        Some("true" | "1") => {
            return Err(ConfigError::Invalid {
                var: "FECO_ADMIN_PASSKEY_OPTIONAL",
                reason: format!(
                    "chỉ được bật khi FECO_ENV=dev; môi trường {environment} luôn bắt buộc passkey"
                ),
            })
        }
        Some(v) => {
            return Err(ConfigError::Invalid {
                var: "FECO_ADMIN_PASSKEY_OPTIONAL",
                reason: format!("{v:?} không phải true/false"),
            })
        }
    };
    Ok(SecurityConfig {
        data_key,
        admin_passkey_optional,
        rate_ip_per_min: num("FECO_RATE_IP_PER_MIN", 1200)?,
        rate_app_per_min: num("FECO_RATE_APP_PER_MIN", 1200)?,
        rate_user_per_min: num("FECO_RATE_USER_PER_MIN", 120)?,
    })
}

fn developer_config<'a>(
    environment: Environment,
    get: &impl Fn(&str) -> Option<&'a str>,
) -> Result<DeveloperConfig, ConfigError> {
    let dns_servers = get("FECO_DNS_SERVERS")
        .unwrap_or("1.1.1.1:53,8.8.8.8:53")
        .split(',')
        .map(|s| {
            s.trim()
                .parse::<SocketAddr>()
                .map_err(|_| ConfigError::Invalid {
                    var: "FECO_DNS_SERVERS",
                    reason: format!("{s:?} không phải địa chỉ ip:cổng"),
                })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let package_dir = match get("FECO_PACKAGE_DIR") {
        Some(p) => PathBuf::from(p),
        None if environment.is_production() => {
            return Err(ConfigError::Missing("FECO_PACKAGE_DIR"))
        }
        None => PathBuf::from("infra/.data/packages"),
    };
    let tuf = match (get("FECO_TUF_REPO"), get("FECO_TUF_SIGN_KEYS")) {
        (None, None) => None,
        (Some(repo), Some(keys)) => Some(TufConfig {
            repo: PathBuf::from(repo),
            sign_keys: keys.split(',').map(|k| PathBuf::from(k.trim())).collect(),
        }),
        (Some(_), None) => return Err(ConfigError::Missing("FECO_TUF_SIGN_KEYS")),
        (None, Some(_)) => return Err(ConfigError::Missing("FECO_TUF_REPO")),
    };
    let clamd = match get("FECO_CLAMD_ADDR") {
        Some(v) => Some(v.parse::<SocketAddr>().map_err(|_| ConfigError::Invalid {
            var: "FECO_CLAMD_ADDR",
            reason: format!("{v:?} không phải địa chỉ ip:cổng"),
        })?),
        None if environment.is_production() => return Err(ConfigError::Missing("FECO_CLAMD_ADDR")),
        None => None,
    };
    let admin_emails: Vec<String> = get("FECO_ADMIN_EMAILS")
        .unwrap_or_default()
        .split(',')
        .map(|e| e.trim().to_owned())
        .filter(|e| !e.is_empty())
        .collect();
    if environment.is_production() && admin_emails.is_empty() {
        return Err(ConfigError::Missing("FECO_ADMIN_EMAILS"));
    }
    Ok(DeveloperConfig {
        dns_servers,
        package_dir,
        tuf,
        clamd,
        admin_emails,
    })
}

/// Giải mã base64 chuẩn (có hoặc không có `=`), không cần thêm thư viện.
pub fn decode_base64(s: &str) -> Option<Vec<u8>> {
    let val = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => return None,
        } as u32)
    };
    let bytes: Vec<u8> = s.trim().trim_end_matches('=').bytes().collect();
    if bytes.len() % 4 == 1 {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() * 3 / 4);
    for chunk in bytes.chunks(4) {
        let mut acc = 0u32;
        for (i, &c) in chunk.iter().enumerate() {
            acc |= val(c)? << (18 - 6 * i);
        }
        let n = chunk.len() * 6 / 8;
        for i in 0..n {
            out.push((acc >> (16 - 8 * i)) as u8);
        }
    }
    Some(out)
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

fn loopback(url: &Url) -> bool {
    matches!(url.host_str(), Some("127.0.0.1" | "localhost"))
}

/// Địa chỉ của cổng thanh toán: HTTPS, trừ cổng giả `http://127.0.0.1` ở môi trường dev.
fn gateway_url(environment: Environment, var: &'static str, raw: &str) -> Result<Url, ConfigError> {
    if environment == Environment::Dev {
        if let Ok(u) = Url::parse(raw) {
            if u.scheme() == "http" && loopback(&u) {
                return Ok(u);
            }
        }
    }
    parse_url(var, raw)
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
    // Cổng giả trên loopback (kiểm thử) chỉ được dùng ở dev (gateway_url đã kiểm).
    if environment == Environment::Dev && url.scheme() == "http" && loopback(url) {
        return Ok(());
    }
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

    /// Production chưa khởi động được toàn bộ (chưa có nhà cung cấp chống bot thật),
    /// nên quy tắc cổng thanh toán của production được kiểm tra qua `validate()`.
    fn as_production(mut c: Config) -> Config {
        c.environment = Environment::Production;
        c
    }

    #[test]
    fn production_with_sandbox_is_rejected() {
        let c = as_production(Config::from_vars(&with_env("staging", VNPAY_SANDBOX)).unwrap());
        assert!(matches!(
            c.validate().unwrap_err(),
            ConfigError::NonProductionGatewayInProduction {
                gateway: "VNPAY",
                ..
            }
        ));
    }

    #[test]
    fn production_with_live_gateways_is_accepted() {
        let mut c = as_production(Config::from_vars(&with_env("staging", &[])).unwrap());
        c.payments.vnpay = Some(VnpayConfig {
            tmn_code: "LIVE0001".into(),
            hash_secret: Secret::new("live-secret"),
            payment_url: Url::parse(VNPAY_LIVE[2].1).unwrap(),
            api_url: Url::parse("https://pay.vnpay.vn/merchant_webapi/api/transaction").unwrap(),
        });
        c.payments.momo = Some(MomoConfig {
            partner_code: "MOMOLIVE".into(),
            access_key: Secret::new("live-access"),
            secret_key: Secret::new("live-secret"),
            endpoint: Url::parse(MOMO_LIVE[3].1).unwrap(),
        });
        c.validate().unwrap();
    }

    #[test]
    fn dev_security_defaults() {
        let c = Config::from_vars(&with_env("dev", &[])).unwrap();
        assert_eq!(
            decode_base64(c.security.data_key.expose()).unwrap().len(),
            32
        );
        assert_eq!(c.security.rate_user_per_min, 120);
    }

    #[test]
    fn data_key_must_be_32_bytes() {
        let v = with_env("dev", &[("FECO_DATA_KEY", "c2hvcnQ=")]);
        assert!(matches!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::Invalid {
                var: "FECO_DATA_KEY",
                ..
            }
        ));
    }

    #[test]
    fn rate_limits_must_be_positive() {
        let v = with_env("dev", &[("FECO_RATE_IP_PER_MIN", "0")]);
        assert!(matches!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::Invalid {
                var: "FECO_RATE_IP_PER_MIN",
                ..
            }
        ));
    }

    #[test]
    fn base64_decoder_matches_known_vectors() {
        assert_eq!(decode_base64("Zm9vYmFy").unwrap(), b"foobar");
        assert_eq!(decode_base64("Zm9vYg==").unwrap(), b"foob");
        assert_eq!(decode_base64("Zm9vYmE").unwrap(), b"fooba");
        assert!(decode_base64("Zm9v!").is_none());
    }

    #[test]
    fn dev_auth_defaults_point_at_dev_infra() {
        let c = Config::from_vars(&with_env("dev", &[])).unwrap();
        assert_eq!(c.auth.kratos_public_url.as_str(), "http://127.0.0.1:4433/");
        assert_eq!(c.auth.kratos_admin_url.as_str(), "http://127.0.0.1:4434/");
        assert_eq!(c.auth.smtp_url.as_str(), "smtp://127.0.0.1:1025");
        assert_eq!(c.auth.challenge, ChallengeProvider::Test);
    }

    #[test]
    fn review_engines_and_alert_recipients() {
        let dev = Config::from_vars(&with_env(
            "dev",
            &[
                ("FECO_CLAMD_ADDR", "127.0.0.1:3310"),
                ("FECO_ADMIN_EMAILS", "a@f-eco.example, b@f-eco.example"),
            ],
        ))
        .unwrap();
        assert_eq!(dev.developer.clamd, Some("127.0.0.1:3310".parse().unwrap()));
        assert_eq!(
            dev.developer.admin_emails,
            ["a@f-eco.example", "b@f-eco.example"]
        );
        let none = Config::from_vars(&with_env("dev", &[])).unwrap();
        assert_eq!(none.developer.clamd, None);

        // Production: ClamAV (engine thứ hai) và người nhận cảnh báo là bắt buộc.
        let vars: HashMap<String, String> = [("FECO_PACKAGE_DIR", "/srv/packages")]
            .map(|(k, v)| (k.into(), v.into()))
            .into();
        let get = |k: &str| vars.get(k).map(String::as_str);
        assert_eq!(
            developer_config(Environment::Production, &get).unwrap_err(),
            ConfigError::Missing("FECO_CLAMD_ADDR")
        );
        let vars: HashMap<String, String> = [
            ("FECO_PACKAGE_DIR", "/srv/packages"),
            ("FECO_CLAMD_ADDR", "10.0.0.5:3310"),
        ]
        .map(|(k, v)| (k.into(), v.into()))
        .into();
        let get = |k: &str| vars.get(k).map(String::as_str);
        assert_eq!(
            developer_config(Environment::Production, &get).unwrap_err(),
            ConfigError::Missing("FECO_ADMIN_EMAILS")
        );
        let bad = with_env("dev", &[("FECO_CLAMD_ADDR", "clamav")]);
        assert!(matches!(
            Config::from_vars(&bad).unwrap_err(),
            ConfigError::Invalid {
                var: "FECO_CLAMD_ADDR",
                ..
            }
        ));
    }

    #[test]
    fn production_requires_auth_settings() {
        assert_eq!(
            Config::from_vars(&with_env("production", &[])).unwrap_err(),
            ConfigError::Missing("FECO_DATABASE_URL")
        );
        let v = with_env(
            "production",
            &[("FECO_DATABASE_URL", "postgres://x@db/feco")],
        );
        assert_eq!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::Missing("FECO_PUBLIC_URL")
        );
    }

    #[test]
    fn production_rejects_test_challenge() {
        let v = with_env(
            "production",
            &[
                ("FECO_DATABASE_URL", "postgres://x@db/feco"),
                ("FECO_PUBLIC_URL", "https://f-eco.example/"),
                ("FECO_CHALLENGE_PROVIDER", "test"),
            ],
        );
        assert_eq!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::TestChallengeInProduction
        );
    }

    #[test]
    fn production_requires_https_public_url() {
        let v = with_env(
            "production",
            &[
                ("FECO_DATABASE_URL", "postgres://x@db/feco"),
                ("FECO_PUBLIC_URL", "http://f-eco.example/"),
            ],
        );
        assert!(matches!(
            Config::from_vars(&v).unwrap_err(),
            ConfigError::InsecureScheme {
                var: "FECO_PUBLIC_URL",
                ..
            }
        ));
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

    const ZALOPAY_SANDBOX: &[(&str, &str)] = &[
        ("FECO_ZALOPAY_APP_ID", "2553"),
        ("FECO_ZALOPAY_KEY1", "sandbox-key1"),
        ("FECO_ZALOPAY_KEY2", "sandbox-key2"),
        ("FECO_ZALOPAY_ENDPOINT", "https://sb-openapi.zalopay.vn"),
    ];

    /// Spec payments, "Staging dùng khóa thật", với ZaloPay.
    #[test]
    fn zalopay_sandbox_only_outside_production() {
        let c = Config::from_vars(&with_env("staging", ZALOPAY_SANDBOX)).unwrap();
        let z = c.payments.zalopay.as_ref().unwrap();
        assert_eq!(z.app_id, 2553);
        assert_eq!(z.endpoint.as_str(), "https://sb-openapi.zalopay.vn/");
        let mut live = with_env("staging", ZALOPAY_SANDBOX);
        live.insert(
            "FECO_ZALOPAY_ENDPOINT".into(),
            "https://openapi.zalopay.vn".into(),
        );
        assert!(matches!(
            Config::from_vars(&live).unwrap_err(),
            ConfigError::ProductionGatewayOutsideProduction {
                gateway: "ZaloPay",
                ..
            }
        ));
        // Cổng giả trên loopback: chỉ ở dev, không ở staging.
        let mut fake = with_env("dev", ZALOPAY_SANDBOX);
        fake.insert(
            "FECO_ZALOPAY_ENDPOINT".into(),
            "http://127.0.0.1:9123".into(),
        );
        assert!(Config::from_vars(&fake).is_ok());
        fake.insert("FECO_ENV".into(), "staging".into());
        assert!(Config::from_vars(&fake).is_err());
        let mut partial = with_env("dev", ZALOPAY_SANDBOX);
        partial.remove("FECO_ZALOPAY_KEY2");
        assert!(matches!(
            Config::from_vars(&partial).unwrap_err(),
            ConfigError::Incomplete { .. }
        ));
        assert!(!format!("{c:?}").contains("sandbox-key1"));
    }

    /// Spec admin-console, "Ngoại lệ passkey ở môi trường dev": scenario "Production với cờ bỏ
    /// passkey" và chỉ dev mới bật được.
    #[test]
    fn admin_passkey_flag_is_dev_only() {
        let mut dev = with_env("dev", VNPAY_SANDBOX);
        assert!(
            !Config::from_vars(&dev)
                .unwrap()
                .security
                .admin_passkey_optional
        );
        dev.insert("FECO_ADMIN_PASSKEY_OPTIONAL".into(), "true".into());
        assert!(
            Config::from_vars(&dev)
                .unwrap()
                .security
                .admin_passkey_optional
        );
        dev.insert("FECO_ADMIN_PASSKEY_OPTIONAL".into(), "co".into());
        assert!(Config::from_vars(&dev).is_err());
        let mut staging = with_env("staging", VNPAY_SANDBOX);
        staging.insert("FECO_ADMIN_PASSKEY_OPTIONAL".into(), "true".into());
        assert!(matches!(
            Config::from_vars(&staging).unwrap_err(),
            ConfigError::Invalid {
                var: "FECO_ADMIN_PASSKEY_OPTIONAL",
                ..
            }
        ));
        staging.insert("FECO_ADMIN_PASSKEY_OPTIONAL".into(), "false".into());
        assert!(
            !Config::from_vars(&staging)
                .unwrap()
                .security
                .admin_passkey_optional
        );
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
