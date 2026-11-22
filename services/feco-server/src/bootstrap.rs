//! Tài khoản quản trị viên mặc định, luôn hoạt động (thay đổi production-readiness, design K1).
//!
//! Mỗi lần khởi động và mỗi 10 phút, [`ensure_admin`] bảo đảm tài khoản `FECO_ADMIN_EMAIL`:
//! - tồn tại trong Kratos, đang hoạt động, email đã xác minh;
//! - là Quản trị viên với đủ năm vai trò;
//! - có tên trong `system_admins`, nên không bị hạ vai trò hay xóa (xem `roles::set_role`).
//!
//! Mật khẩu ban đầu chỉ đọc từ tệp bí mật và chỉ dùng khi tạo tài khoản; không bao giờ được ghi
//! vào nhật ký. Không có mật khẩu mặc định nào trong mã nguồn.

use std::path::Path;

use feco_config::{AdminBootstrapConfig, ADMIN_PASSWORD_MIN_LEN};
use serde_json::json;
use uuid::Uuid;

use crate::{
    admin,
    audit::{self, Actor},
    roles::{self, ChangedBy, PlatformRole},
    store::Db,
    AppState,
};

/// Người thực hiện ghi trong nhật ký khi máy chủ tự khôi phục tài khoản.
pub const SYSTEM_ACTOR: &str = "he-thong";

/// Đọc mật khẩu ban đầu. Ở dev, tệp chưa có thì sinh mật khẩu ngẫu nhiên và ghi vào tệp.
pub fn load_password(cfg: &AdminBootstrapConfig) -> Result<String, String> {
    match std::fs::read_to_string(&cfg.password_file) {
        Ok(text) => {
            let password = text.trim_end_matches(['\r', '\n']).to_owned();
            check_password(&password).map(|_| password)
        }
        Err(e) if e.kind() == std::io::ErrorKind::NotFound && cfg.generate_password => {
            let password = generate_password();
            write_secret(&cfg.password_file, &password)?;
            tracing::warn!(
                file = %cfg.password_file.display(),
                "dev: đã sinh mật khẩu cho tài khoản quản trị viên mặc định"
            );
            Ok(password)
        }
        Err(e) => Err(format!(
            "không đọc được FECO_ADMIN_PASSWORD_FILE ({}): {e}",
            cfg.password_file.display()
        )),
    }
}

fn check_password(password: &str) -> Result<(), String> {
    if password.chars().count() < ADMIN_PASSWORD_MIN_LEN {
        return Err(format!(
            "mật khẩu quản trị viên mặc định phải dài ít nhất {ADMIN_PASSWORD_MIN_LEN} ký tự"
        ));
    }
    Ok(())
}

/// 32 ký tự ngẫu nhiên (hai UUID v4 lấy từ bộ sinh số ngẫu nhiên của hệ điều hành).
fn generate_password() -> String {
    // 32 ký tự từ nguồn ngẫu nhiên của hệ điều hành. Ba ký tự đầu lần lượt là chữ hoa, chữ
    // thường, chữ số để mật khẩu luôn có đủ ba loại; phần còn lại lấy từ cả 62 ký tự. Chỉ lấy byte
    // nằm trong bội lớn nhất của độ dài bảng (ví dụ 248 = 62 × 4) để chia đều.
    const UPPER: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZ";
    const LOWER: &[u8] = b"abcdefghijklmnopqrstuvwxyz";
    const DIGIT: &[u8] = b"0123456789";
    const ALL: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    let mut out = String::with_capacity(32);
    let mut buf = [0u8; 64];
    while out.len() < 32 {
        getrandom::fill(&mut buf).expect("nguồn ngẫu nhiên");
        for b in buf {
            if out.len() == 32 {
                break;
            }
            let set = match out.len() {
                0 => UPPER,
                1 => LOWER,
                2 => DIGIT,
                _ => ALL,
            };
            // Bội lớn nhất của độ dài bảng không vượt 256: bỏ byte ngoài khoảng để chia đều.
            let limit = 256 - 256 % set.len();
            if (b as usize) < limit {
                out.push(set[b as usize % set.len()] as char);
            }
        }
    }
    out
}

fn write_secret(path: &Path, value: &str) -> Result<(), String> {
    if let Some(dir) = path.parent().filter(|d| !d.as_os_str().is_empty()) {
        std::fs::create_dir_all(dir)
            .map_err(|e| format!("không tạo được {}: {e}", dir.display()))?;
    }
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    use std::io::Write;
    options
        .open(path)
        .and_then(|mut f| f.write_all(format!("{value}\n").as_bytes()))
        .map_err(|e| format!("không ghi được {}: {e}", path.display()))
}

/// Kết quả một lượt kiểm.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Outcome {
    pub subject: Uuid,
    pub created: bool,
    pub reactivated: bool,
    pub roles_restored: bool,
}

/// Bảo đảm tài khoản quản trị viên mặc định tồn tại, hoạt động và đủ vai trò.
pub async fn ensure_admin(state: &AppState, cfg: &AdminBootstrapConfig) -> Result<Outcome, String> {
    let email = cfg.email.as_str();
    let found = state
        .kratos
        .find_identity(email)
        .await
        .map_err(|e| e.to_string())?;
    let (subject, created, reactivated) = match found {
        None => {
            let password = load_password(cfg)?;
            let body = json!({
                "schema_id": "default",
                "state": "active",
                "traits": {"email": email, "name": "Quản trị viên F-Eco", "locale": "vi"},
                "credentials": {"password": {"config": {"password": password}}},
                "verifiable_addresses": [
                    {"value": email, "verified": true, "via": "email", "status": "completed"}
                ],
                "recovery_addresses": [{"value": email, "via": "email"}],
            });
            let identity = state
                .kratos
                .create_identity(&body)
                .await
                .map_err(|e| e.to_string())?;
            (identity_id(&identity)?, true, false)
        }
        Some(identity) => {
            let id = identity_id(&identity)?;
            let inactive = identity["state"].as_str() != Some("active");
            if inactive {
                state
                    .kratos
                    .set_identity_state(&id.to_string(), "active")
                    .await
                    .map_err(|e| e.to_string())?;
            }
            (id, false, inactive)
        }
    };
    sqlx::query(
        "INSERT INTO system_admins (subject, email) VALUES ($1, $2)
         ON CONFLICT (subject) DO UPDATE SET email = EXCLUDED.email",
    )
    .bind(subject)
    .bind(email)
    .execute(&state.db)
    .await
    .map_err(|e| e.to_string())?;

    let role = roles::role_of(&state.db, subject)
        .await
        .map_err(|e| e.to_string())?;
    let have = roles::admin_roles(&state.db, subject)
        .await
        .map_err(|e| e.to_string())?;
    let complete = role == PlatformRole::Admin
        && admin::Role::ALL
            .iter()
            .all(|r| have.iter().any(|h| h == r.as_str()));
    if !complete {
        roles::set_role(
            &state.db,
            subject,
            email,
            PlatformRole::Admin,
            &admin::Role::ALL,
            &ChangedBy {
                subject: None,
                name: SYSTEM_ACTOR.into(),
            },
            "Tài khoản quản trị viên mặc định",
        )
        .await
        .map_err(|e| e.to_string())?;
    }
    let outcome = Outcome {
        subject,
        created,
        reactivated,
        roles_restored: !complete,
    };
    if created || reactivated || !complete {
        audit::record(
            &state.db,
            Actor::Admin(SYSTEM_ACTOR.into()),
            "admin.bootstrap",
            Some(&subject.to_string()),
            None,
            json!({
                "email": email,
                "created": created,
                "reactivated": reactivated,
                "roles_restored": !complete,
            }),
        )
        .await;
        tracing::info!(%email, created, reactivated, roles_restored = !complete, "đã khôi phục tài khoản quản trị viên mặc định");
    }
    Ok(outcome)
}

fn identity_id(identity: &serde_json::Value) -> Result<Uuid, String> {
    identity["id"]
        .as_str()
        .and_then(|s| s.parse().ok())
        .ok_or_else(|| "Kratos không trả mã danh tính".to_owned())
}

/// Tài khoản này là quản trị viên mặc định (không được hạ vai trò hay xóa).
pub async fn is_system_admin(db: &Db, subject: Uuid) -> sqlx::Result<bool> {
    sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM system_admins WHERE subject = $1)")
        .bind(subject)
        .fetch_one(db)
        .await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cfg(dir: &Path, generate: bool) -> AdminBootstrapConfig {
        AdminBootstrapConfig {
            email: "admin@f-eco.local".into(),
            password_file: dir.join("sub").join("admin-password"),
            generate_password: generate,
        }
    }

    /// Dev tự sinh mật khẩu một lần rồi đọc lại; production không có tệp thì báo lỗi; mật khẩu
    /// ngắn bị từ chối.
    #[test]
    fn password_file_rules() {
        let dir = tempfile::tempdir().unwrap();
        let dev = cfg(dir.path(), true);
        let first = load_password(&dev).unwrap();
        assert_eq!(first.len(), 32);
        assert!(first.chars().any(|c| c.is_ascii_uppercase()));
        // Lần nào cũng đủ chữ hoa, chữ thường, chữ số (trước đây đôi khi thiếu chữ hoa).
        for _ in 0..2000 {
            let p = generate_password();
            assert_eq!(p.len(), 32);
            assert!(p.chars().all(|c| c.is_ascii_alphanumeric()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_uppercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_lowercase()), "{p}");
            assert!(p.chars().any(|c| c.is_ascii_digit()), "{p}");
        }
        assert_eq!(
            load_password(&dev).unwrap(),
            first,
            "đọc lại đúng mật khẩu đã sinh"
        );

        let other = tempfile::tempdir().unwrap();
        let prod = cfg(other.path(), false);
        let err = load_password(&prod).unwrap_err();
        assert!(err.contains("FECO_ADMIN_PASSWORD_FILE"), "{err}");
        assert!(!prod.password_file.exists(), "production không tự sinh");

        std::fs::create_dir_all(prod.password_file.parent().unwrap()).unwrap();
        std::fs::write(&prod.password_file, "ngan\n").unwrap();
        assert!(load_password(&prod).unwrap_err().contains("12"));
        std::fs::write(&prod.password_file, "mot-mat-khau-du-dai\r\n").unwrap();
        assert_eq!(load_password(&prod).unwrap(), "mot-mat-khau-du-dai");
    }
}
