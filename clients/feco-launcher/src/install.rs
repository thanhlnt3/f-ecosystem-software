//! Cài, cập nhật, quay lại, gỡ và chuẩn bị mở app con (spec launcher: "Cài đặt có xác minh",
//! "Cập nhật app con", "Gỡ app con", "Gỡ khẩn cấp"; design D4).
//!
//! Cập nhật nguyên tử: gói được tải vào `downloads/` và xác minh bằng TUF (độ dài, SHA-256,
//! chữ ký của nền tảng), giải nén vào thư mục tạm cạnh thư mục app, đổi tên thành thư mục
//! phiên bản, và chỉ khi đó mới ghi `state.json`. Lỗi ở bất kỳ bước nào trước đó thì trạng thái
//! cũ giữ nguyên và bản cũ vẫn mở được. Bản ngay trước được giữ lại trên đĩa để quay lại.

use crate::catalog::{self, CatalogEntry};
use crate::events::EventLog;
use crate::paths::{safe_component, safe_relative, Paths};
use crate::state::{InstalledApp, InstalledVersion, LocalState};
use feco_tuf::{Client, Revocation};
use jiff::{SignedDuration, Timestamp};
use serde_json::json;
use std::cmp::Ordering;
use std::path::{Path, PathBuf};

/// Kiểm tra bản cập nhật mỗi 6 giờ (spec launcher, "Cập nhật app con").
pub const UPDATE_CHECK_INTERVAL: SignedDuration = SignedDuration::from_hours(6);

#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("Đang ngoại tuyến ({0})")]
    Offline(String),
    #[error("Gói cài đặt không hợp lệ")]
    InvalidPackage(String),
    #[error("Metadata phân phối không hợp lệ, đã từ chối ({0})")]
    InvalidMetadata(String),
    #[error("{title} {version} đã bị thu hồi: {reason}")]
    Revoked {
        title: String,
        version: String,
        reason: String,
    },
    #[error("App {0} chưa được cài")]
    NotInstalled(String),
    #[error("Không có bản trước để quay lại")]
    NoPrevious,
    #[error("Lỗi đọc/ghi {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("{0}")]
    Other(String),
}

impl LauncherError {
    /// Chi tiết kỹ thuật (để ghi nhật ký), ngoài thông báo cho người dùng.
    pub fn detail(&self) -> String {
        match self {
            LauncherError::InvalidPackage(d) | LauncherError::InvalidMetadata(d) => d.clone(),
            other => other.to_string(),
        }
    }
}

fn io(path: &Path) -> impl FnOnce(std::io::Error) -> LauncherError + '_ {
    move |source| LauncherError::Io {
        path: path.to_path_buf(),
        source,
    }
}

/// Phân loại lỗi của chuỗi tin cậy: lỗi toàn vẹn (sai hash, sai chữ ký, metadata cũ hoặc hết
/// hạn) khác với lỗi mạng.
fn classify(e: feco_tuf::Error, package: bool) -> LauncherError {
    let detail = format!("{e:?}");
    const INTEGRITY: [&str; 9] = [
        "HashMismatch",
        "MaxSize",
        "Verify",
        "SignatureThreshold",
        "OlderMetadata",
        "ExpiredMetadata",
        "VersionMismatch",
        "ParseMetadata",
        "Canonical",
    ];
    if INTEGRITY.iter().any(|m| detail.contains(m)) {
        if package && !detail.contains("Metadata") {
            LauncherError::InvalidPackage(detail)
        } else {
            LauncherError::InvalidMetadata(detail)
        }
    } else if matches!(e, feco_tuf::Error::Invalid(_)) {
        LauncherError::Other(e.to_string())
    } else {
        LauncherError::Offline(detail)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AppStatus {
    Ready,
    Revoked { reason: String },
}

/// Một dòng trong "App của tôi".
#[derive(Clone, Debug)]
pub struct MyApp {
    pub app: String,
    pub title: String,
    pub version: String,
    pub previous_version: Option<String>,
    pub status: AppStatus,
    /// Bản mới hơn trong danh mục (nếu có).
    pub update: Option<CatalogEntry>,
}

/// Những gì cần để mở một app con.
#[derive(Clone, Debug)]
pub struct LaunchPlan {
    pub app: String,
    pub title: String,
    pub version: String,
    pub program: PathBuf,
    pub working_dir: PathBuf,
    pub data_dir: PathBuf,
    pub client_id: Option<String>,
}

pub struct Launcher {
    paths: Paths,
    tuf: Client,
    platform: String,
    state: LocalState,
    events: EventLog,
    online: bool,
}

impl Launcher {
    pub fn open(
        paths: Paths,
        tuf: Client,
        platform: impl Into<String>,
    ) -> Result<Self, LauncherError> {
        std::fs::create_dir_all(paths.root()).map_err(io(paths.root()))?;
        let state = LocalState::load(&paths.state_file()).map_err(io(&paths.state_file()))?;
        let events = EventLog::new(paths.events_file());
        Ok(Self {
            paths,
            tuf,
            platform: platform.into(),
            state,
            events,
            online: false,
        })
    }

    pub fn paths(&self) -> &Paths {
        &self.paths
    }

    pub fn state(&self) -> &LocalState {
        &self.state
    }

    pub fn events(&self) -> &EventLog {
        &self.events
    }

    /// Lần đồng bộ gần nhất có thành công không.
    pub fn is_online(&self) -> bool {
        self.online
    }

    fn save(&self) -> Result<(), LauncherError> {
        let path = self.paths.state_file();
        self.state.save(&path).map_err(io(&path))
    }

    /// Đồng bộ metadata TUF. Lỗi thì giữ nguyên danh mục và danh sách thu hồi cũ: "App của tôi"
    /// vẫn dùng được, phần danh mục báo ngoại tuyến. Metadata giả hoặc cũ bị từ chối và ghi sự
    /// kiện bảo mật.
    pub async fn sync(&mut self, now: Timestamp) -> Result<(), LauncherError> {
        match self.tuf.sync().await {
            Ok(synced) => {
                self.state.catalog = catalog::build(&synced, &self.platform);
                self.state.revocations = synced.revocations;
                self.state.last_sync = Some(now);
                self.online = true;
                self.save()
            }
            Err(e) => {
                self.online = false;
                let err = classify(e, false);
                if let LauncherError::InvalidMetadata(detail) = &err {
                    self.events
                        .record("invalid_metadata", json!({ "detail": detail }));
                }
                Err(err)
            }
        }
    }

    /// Bản mới nhất của từng app trong danh mục.
    pub fn catalog_latest(&self) -> Vec<&CatalogEntry> {
        catalog::latest(&self.state.catalog)
    }

    pub fn revocation(&self, target: &str) -> Option<&Revocation> {
        self.state
            .revocations
            .iter()
            .rev()
            .find(|r| r.target == target)
    }

    /// Bản mới hơn bản đang cài, nếu có.
    pub fn update_for(&self, app: &str) -> Option<&CatalogEntry> {
        let current = &self.state.apps.get(app)?.current;
        self.state
            .catalog
            .iter()
            .filter(|e| e.app == app)
            .filter(|e| compare(&e.version, &current.version) == Ordering::Greater)
            .max_by(|a, b| compare(&a.version, &b.version))
    }

    pub fn my_apps(&self) -> Vec<MyApp> {
        self.state
            .apps
            .values()
            .map(|a| MyApp {
                app: a.current.app.clone(),
                title: a.current.title.clone(),
                version: a.current.version.clone(),
                previous_version: a.previous.as_ref().map(|p| p.version.clone()),
                status: match self.revocation(&a.current.target) {
                    Some(r) => AppStatus::Revoked {
                        reason: r.reason.clone(),
                    },
                    None => AppStatus::Ready,
                },
                update: self.update_for(&a.current.app).cloned(),
            })
            .collect()
    }

    fn revoked_error(&self, title: &str, version: &str, target: &str) -> Option<LauncherError> {
        self.revocation(target).map(|r| LauncherError::Revoked {
            title: title.to_owned(),
            version: version.to_owned(),
            reason: r.reason.clone(),
        })
    }

    /// Cài hoặc cập nhật lên phiên bản `entry`.
    pub async fn install(
        &mut self,
        entry: &CatalogEntry,
        now: Timestamp,
    ) -> Result<(), LauncherError> {
        if !safe_component(&entry.app)
            || !safe_component(&entry.version)
            || !safe_relative(&entry.entry)
        {
            let err = LauncherError::InvalidPackage(format!(
                "tên app, phiên bản hoặc file chạy không an toàn: {entry:?}"
            ));
            self.events.record(
                "invalid_package",
                json!({ "target": entry.target, "detail": err.detail() }),
            );
            return Err(err);
        }
        if let Some(err) = self.revoked_error(&entry.title, &entry.version, &entry.target) {
            return Err(err);
        }
        if self
            .state
            .apps
            .get(&entry.app)
            .is_some_and(|a| a.current.version == entry.version)
        {
            return Ok(());
        }

        let downloads = self.paths.downloads();
        std::fs::create_dir_all(&downloads).map_err(io(&downloads))?;
        let partial = downloads.join(format!("{}.partial", entry.target));
        let staging = self.paths.app(&entry.app).join(format!(
            ".staging-{}-{}",
            entry.version,
            crate::random_hex(4)
        ));
        let result = self.download_and_extract(entry, &partial, &staging).await;
        let _ = std::fs::remove_file(&partial);
        if let Err(err) = result {
            let _ = std::fs::remove_dir_all(&staging);
            if let LauncherError::InvalidPackage(detail) = &err {
                self.events.record(
                    "invalid_package",
                    json!({ "target": entry.target, "detail": detail }),
                );
            }
            return Err(err);
        }

        let final_dir = self.paths.version(&entry.app, &entry.version);
        if final_dir.exists() {
            // Thư mục cùng phiên bản còn lại từ trước (bản "trước" sau khi quay lại).
            std::fs::remove_dir_all(&final_dir).map_err(io(&final_dir))?;
        }
        if let Err(e) = std::fs::rename(&staging, &final_dir) {
            let _ = std::fs::remove_dir_all(&staging);
            return Err(io(&final_dir)(e));
        }

        let new = InstalledVersion {
            app: entry.app.clone(),
            version: entry.version.clone(),
            title: entry.title.clone(),
            target: entry.target.clone(),
            sha256: entry.sha256.clone(),
            entry: entry.entry.clone(),
            client_id: entry.client_id.clone(),
            installed_at: now,
        };
        let before = self.state.apps.get(&entry.app).cloned();
        self.state.apps.insert(
            entry.app.clone(),
            InstalledApp {
                current: new,
                previous: before.as_ref().map(|a| a.current.clone()),
            },
        );
        if let Err(e) = self.save() {
            match before {
                Some(b) => self.state.apps.insert(entry.app.clone(), b),
                None => self.state.apps.remove(&entry.app),
            };
            return Err(e);
        }
        self.prune(&entry.app);
        Ok(())
    }

    async fn download_and_extract(
        &self,
        entry: &CatalogEntry,
        partial: &Path,
        staging: &Path,
    ) -> Result<(), LauncherError> {
        self.tuf
            .download_to(&entry.target, partial)
            .await
            .map_err(|e| classify(e, true))?;
        // Thêm một lớp: gói phải khớp đúng bản trong danh mục người dùng đã chọn.
        let sha = sha256_file(partial)?;
        if sha != entry.sha256 {
            return Err(LauncherError::InvalidPackage(format!(
                "SHA-256 {sha} khác danh mục {}",
                entry.sha256
            )));
        }
        let (archive, dest, program) = (
            partial.to_path_buf(),
            staging.to_path_buf(),
            entry.entry.clone(),
        );
        tokio::task::spawn_blocking(move || {
            extract_zip(&archive, &dest)?;
            if !dest.join(&program).is_file() {
                return Err(LauncherError::InvalidPackage(format!(
                    "gói không có file chạy {program}"
                )));
            }
            Ok(())
        })
        .await
        .map_err(|e| LauncherError::Other(e.to_string()))?
    }

    /// Xóa các thư mục phiên bản không còn là bản hiện tại hay bản trước.
    fn prune(&self, app: &str) {
        let Some(installed) = self.state.apps.get(app) else {
            return;
        };
        let keep: Vec<&str> = std::iter::once(installed.current.version.as_str())
            .chain(installed.previous.as_ref().map(|p| p.version.as_str()))
            .collect();
        let dir = self.paths.app(app);
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return;
        };
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !keep.contains(&name.as_str()) {
                let _ = std::fs::remove_dir_all(entry.path());
            }
        }
    }

    /// Cập nhật mọi app có bản mới; trả kết quả từng app.
    pub async fn update_all(&mut self, now: Timestamp) -> Vec<(String, Result<(), LauncherError>)> {
        let updates: Vec<CatalogEntry> = self
            .state
            .apps
            .keys()
            .filter_map(|app| self.update_for(app).cloned())
            .collect();
        let mut out = Vec::new();
        for entry in updates {
            let result = self.install(&entry, now).await;
            out.push((entry.app.clone(), result));
        }
        self.state.last_update_check = Some(now);
        let _ = self.save();
        out
    }

    pub fn due_for_update_check(&self, now: Timestamp) -> bool {
        self.state
            .last_update_check
            .is_none_or(|last| now.duration_since(last) >= UPDATE_CHECK_INTERVAL)
    }

    pub fn set_auto_update(&mut self, on: bool) -> Result<(), LauncherError> {
        self.state.auto_update = on;
        self.save()
    }

    pub fn set_user_name(&mut self, name: Option<String>) -> Result<(), LauncherError> {
        self.state.user_name = name;
        self.save()
    }

    /// Quay lại bản ngay trước đó (không cần tải lại).
    pub fn rollback(&mut self, app: &str) -> Result<(), LauncherError> {
        let installed = self
            .state
            .apps
            .get(app)
            .cloned()
            .ok_or_else(|| LauncherError::NotInstalled(app.to_owned()))?;
        let previous = installed
            .previous
            .clone()
            .ok_or(LauncherError::NoPrevious)?;
        if let Some(err) = self.revoked_error(&previous.title, &previous.version, &previous.target)
        {
            return Err(err);
        }
        let program = self
            .paths
            .version(app, &previous.version)
            .join(&previous.entry);
        if !program.is_file() {
            return Err(LauncherError::Other(format!(
                "bản {} không còn trên máy, hãy cài lại",
                previous.version
            )));
        }
        self.state.apps.insert(
            app.to_owned(),
            InstalledApp {
                current: previous,
                previous: Some(installed.current.clone()),
            },
        );
        if let Err(e) = self.save() {
            self.state.apps.insert(app.to_owned(), installed);
            return Err(e);
        }
        Ok(())
    }

    /// Gỡ app. Dữ liệu cục bộ chỉ bị xóa khi `delete_data`; đồng ý dữ liệu trên máy chủ không bị
    /// thu hồi (người dùng thu hồi ở trang "Ứng dụng đã kết nối").
    pub fn uninstall(&mut self, app: &str, delete_data: bool) -> Result<(), LauncherError> {
        if !self.state.apps.contains_key(app) || !safe_component(app) {
            return Err(LauncherError::NotInstalled(app.to_owned()));
        }
        remove_dir_if_exists(&self.paths.app(app))?;
        if delete_data {
            remove_dir_if_exists(&self.paths.app_data(app))?;
        }
        self.state.apps.remove(app);
        self.save()
    }

    /// Kiểm tra trước khi mở: phiên bản bị thu hồi thì không cho mở.
    pub fn prepare_launch(&self, app: &str) -> Result<LaunchPlan, LauncherError> {
        let installed = self
            .state
            .apps
            .get(app)
            .ok_or_else(|| LauncherError::NotInstalled(app.to_owned()))?;
        let current = &installed.current;
        if let Some(err) = self.revoked_error(&current.title, &current.version, &current.target) {
            return Err(err);
        }
        let working_dir = self.paths.version(app, &current.version);
        let program = working_dir.join(&current.entry);
        if !program.is_file() {
            return Err(LauncherError::Other(format!(
                "không thấy file chạy của {} {}, hãy cài lại",
                current.title, current.version
            )));
        }
        let data_dir = self.paths.app_data(app);
        std::fs::create_dir_all(&data_dir).map_err(io(&data_dir))?;
        Ok(LaunchPlan {
            app: app.to_owned(),
            title: current.title.clone(),
            version: current.version.clone(),
            program,
            working_dir,
            data_dir,
            client_id: current.client_id.clone(),
        })
    }
}

fn compare(a: &str, b: &str) -> Ordering {
    catalog::compare_versions(a, b)
}

fn remove_dir_if_exists(path: &Path) -> Result<(), LauncherError> {
    match std::fs::remove_dir_all(path) {
        Ok(()) => Ok(()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(e) => Err(io(path)(e)),
    }
}

fn sha256_file(path: &Path) -> Result<String, LauncherError> {
    use sha2::Digest;
    use std::io::Read;
    let mut file = std::fs::File::open(path).map_err(io(path))?;
    let mut hasher = sha2::Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = file.read(&mut buf).map_err(io(path))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
    }
    Ok(hex::encode(hasher.finalize()))
}

/// Giải nén gói zip; từ chối đường dẫn thoát ra ngoài thư mục đích và liên kết tượng trưng.
fn extract_zip(archive: &Path, dest: &Path) -> Result<(), LauncherError> {
    let invalid = |e: String| LauncherError::InvalidPackage(e);
    let file = std::fs::File::open(archive).map_err(io(archive))?;
    let mut zip =
        zip::ZipArchive::new(file).map_err(|e| invalid(format!("không đọc được gói zip: {e}")))?;
    std::fs::create_dir_all(dest).map_err(io(dest))?;
    for i in 0..zip.len() {
        let mut item = zip
            .by_index(i)
            .map_err(|e| invalid(format!("mục {i} trong gói hỏng: {e}")))?;
        let name = item.name().map(|n| n.into_owned()).unwrap_or_default();
        if item.is_symlink() {
            return Err(invalid(format!("gói chứa liên kết tượng trưng: {name}")));
        }
        let Some(relative) = item.enclosed_name() else {
            return Err(invalid(format!(
                "đường dẫn không an toàn trong gói: {name}"
            )));
        };
        let out = dest.join(relative);
        if item.is_dir() {
            std::fs::create_dir_all(&out).map_err(io(&out))?;
            continue;
        }
        if let Some(parent) = out.parent() {
            std::fs::create_dir_all(parent).map_err(io(parent))?;
        }
        let mut file = std::fs::File::create(&out).map_err(io(&out))?;
        std::io::copy(&mut item, &mut file).map_err(io(&out))?;
        #[cfg(unix)]
        if let Some(mode) = item.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&out, std::fs::Permissions::from_mode(mode & 0o755))
                .map_err(io(&out))?;
        }
    }
    Ok(())
}
