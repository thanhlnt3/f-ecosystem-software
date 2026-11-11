//! Scenario của spec launcher, chạy với kho TUF thật (feco-tuf + tough) trên đĩa và gói zip thật.

use feco_launcher::broker::{Broker, Issuer, LaunchCode};
use feco_launcher::events::EventLog;
use feco_launcher::launch::{command, LaunchEnv};
use feco_launcher::{AppStatus, Launcher, LauncherError, Paths, ISOLATION_NOTICE};
use feco_tuf::{keys, Client, Expiry, InitKeys, Repo, TargetMeta};
use jiff::{SignedDuration, Timestamp};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tempfile::TempDir;
use tough::key_source::KeySource;

const PLATFORM: &str = "windows-x64";

struct Fixture {
    tmp: TempDir,
    repo: Repo,
    keys: Vec<PathBuf>,
}

fn signers(paths: &[PathBuf]) -> Vec<Box<dyn KeySource>> {
    paths.iter().map(|p| keys::source(p.as_path())).collect()
}

async fn public(path: &Path) -> feco_tuf::Key {
    keys::public_key(keys::source(path).as_ref()).await.unwrap()
}

fn make_zip(path: &Path, files: &[(&str, &[u8])]) {
    let file = std::fs::File::create(path).unwrap();
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
    for (name, data) in files {
        zip.start_file(*name, options).unwrap();
        zip.write_all(data).unwrap();
    }
    zip.finish().unwrap();
}

fn target(app: &str, version: &str) -> String {
    format!("{app}-{version}-{PLATFORM}.zip")
}

impl Fixture {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("keys");
        let mut paths = Vec::new();
        for name in [
            "root-1",
            "root-2",
            "root-3",
            "targets",
            "snapshot",
            "timestamp",
        ] {
            let p = dir.join(format!("{name}.pk8"));
            keys::generate(&p).await.unwrap();
            paths.push(p);
        }
        let repo = Repo::open(tmp.path().join("repo"));
        let init = InitKeys {
            root: vec![
                public(&paths[0]).await,
                public(&paths[1]).await,
                public(&paths[2]).await,
            ],
            root_threshold: 2,
            targets: vec![public(&paths[3]).await],
            snapshot: vec![public(&paths[4]).await],
            timestamp: vec![public(&paths[5]).await],
        };
        repo.init(
            &init,
            &signers(&[
                paths[0].clone(),
                paths[1].clone(),
                paths[3].clone(),
                paths[4].clone(),
                paths[5].clone(),
            ]),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .unwrap();
        Self {
            tmp,
            repo,
            keys: paths[3..].to_vec(),
        }
    }

    /// Ký một phiên bản app (gói zip có file chạy `bin/<app>.cmd`).
    async fn publish(&self, app: &str, version: &str, files: &[(&str, &[u8])]) -> String {
        let name = target(app, version);
        let upload = self.tmp.path().join("uploads");
        std::fs::create_dir_all(&upload).unwrap();
        let file = upload.join(&name);
        make_zip(&file, files);
        let meta = TargetMeta {
            app: app.into(),
            version: version.into(),
            platform: PLATFORM.into(),
            title: Some(format!("{app} (tên hiển thị)")),
            entry: Some(format!("bin/{app}.cmd")),
            client_id: Some(format!("client-{app}")),
        };
        self.repo
            .publish(
                &file,
                &name,
                &meta,
                &signers(&self.keys),
                Expiry::default(),
                Timestamp::now(),
            )
            .await
            .unwrap();
        name
    }

    async fn publish_app(&self, app: &str, version: &str) -> String {
        let exe = format!("bin/{app}.cmd");
        let content = format!("@echo {app} {version}");
        self.publish(
            app,
            version,
            &[(exe.as_str(), content.as_bytes()), ("README.txt", b"doc")],
        )
        .await
    }

    async fn revoke(&self, name: &str, reason: &str) {
        self.repo
            .revoke(
                name,
                reason,
                &signers(&self.keys),
                Expiry::default(),
                Timestamp::now(),
            )
            .await
            .unwrap();
    }

    fn paths(&self) -> Paths {
        Paths::new(self.tmp.path().join("launcher"))
    }

    async fn launcher(&self) -> Launcher {
        let paths = self.paths();
        let client =
            Client::for_local_repo(&self.repo, &self.repo.root_path(1), paths.tuf_datastore())
                .await
                .unwrap();
        Launcher::open(paths, client, PLATFORM).unwrap()
    }

    /// Launcher trỏ tới một máy chủ phân phối không tới được (mất mạng).
    async fn offline_launcher(&self) -> Launcher {
        let paths = self.paths();
        let client = Client::new(
            std::fs::read(self.repo.root_path(1)).unwrap(),
            url::Url::parse("http://127.0.0.1:9/metadata/").unwrap(),
            url::Url::parse("http://127.0.0.1:9/targets/").unwrap(),
            paths.tuf_datastore(),
        );
        Launcher::open(paths, client, PLATFORM).unwrap()
    }

    fn stored_target(&self, name: &str, sha: &str) -> PathBuf {
        self.repo.targets_dir().join(format!("{sha}.{name}"))
    }
}

fn entry(l: &Launcher, app: &str, version: &str) -> feco_launcher::CatalogEntry {
    l.state()
        .catalog
        .iter()
        .find(|e| e.app == app && e.version == version)
        .unwrap_or_else(|| panic!("không có {app} {version} trong danh mục"))
        .clone()
}

fn kinds(events: &EventLog) -> Vec<String> {
    events.read_all().into_iter().map(|e| e.kind).collect()
}

/// Scenario "Cài thành công": tải, xác minh, cài vào thư mục người dùng, mở được.
#[tokio::test]
async fn install_verified_package_and_open() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.1.0").await;
    f.publish_app("soundlab", "1.2.0").await;
    f.publish_app("paintbox", "0.9.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    assert!(l.is_online());
    let latest: Vec<(String, String)> = l
        .catalog_latest()
        .iter()
        .map(|e| (e.app.clone(), e.version.clone()))
        .collect();
    assert_eq!(
        latest,
        [
            ("paintbox".into(), "0.9.0".into()),
            ("soundlab".into(), "1.2.0".into())
        ]
    );

    l.install(&entry(&l, "soundlab", "1.2.0"), Timestamp::now())
        .await
        .unwrap();
    let plan = l.prepare_launch("soundlab").unwrap();
    assert_eq!(plan.version, "1.2.0");
    assert!(
        plan.program.ends_with("bin/soundlab.cmd") || plan.program.ends_with("bin\\soundlab.cmd")
    );
    assert_eq!(
        std::fs::read_to_string(&plan.program).unwrap(),
        "@echo soundlab 1.2.0"
    );
    assert!(
        plan.program.starts_with(f.paths().apps()),
        "cài trong thư mục của người dùng"
    );
    assert_eq!(plan.client_id.as_deref(), Some("client-soundlab"));
    assert!(plan.data_dir.is_dir());
    let mine = l.my_apps();
    assert_eq!(mine.len(), 1);
    assert_eq!(mine[0].status, AppStatus::Ready);
    assert!(mine[0].update.is_none());
    // Không còn file tạm.
    assert_eq!(std::fs::read_dir(f.paths().downloads()).unwrap().count(), 0);
    assert!(ISOLATION_NOTICE.contains("quyền như mọi ứng dụng khác"));
}

/// Scenario "Gói bị sửa": xóa gói, báo "Gói cài đặt không hợp lệ", ghi sự kiện bảo mật.
#[tokio::test]
async fn tampered_package_is_deleted_and_reported() {
    let f = Fixture::new().await;
    let name = f.publish_app("soundlab", "1.2.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    let e = entry(&l, "soundlab", "1.2.0");
    // Máy chủ phân phối (hoặc kẻ đứng giữa) thay nội dung gói, giữ nguyên tên.
    let stored = f.stored_target(&name, &e.sha256);
    let mut bytes = std::fs::read(&stored).unwrap();
    let last = bytes.len() - 30;
    bytes[last] ^= 0xff;
    std::fs::write(&stored, &bytes).unwrap();

    let err = l.install(&e, Timestamp::now()).await.unwrap_err();
    assert!(matches!(err, LauncherError::InvalidPackage(_)), "{err:?}");
    assert_eq!(err.to_string(), "Gói cài đặt không hợp lệ");
    assert!(l.state().apps.is_empty());
    assert_eq!(
        std::fs::read_dir(f.paths().downloads()).unwrap().count(),
        0,
        "gói bị xóa"
    );
    assert!(!f.paths().version("soundlab", "1.2.0").exists());
    assert!(kinds(l.events()).contains(&"invalid_package".to_owned()));
}

/// Gói có đường dẫn thoát ra ngoài thư mục cài đặt bị từ chối, dù đã được ký.
#[tokio::test]
async fn package_with_path_traversal_is_rejected() {
    let f = Fixture::new().await;
    f.publish(
        "evil",
        "1.0.0",
        &[("bin/evil.cmd", b"x"), ("../../outside.txt", b"pwned")],
    )
    .await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    let err = l
        .install(&entry(&l, "evil", "1.0.0"), Timestamp::now())
        .await
        .unwrap_err();
    assert!(matches!(err, LauncherError::InvalidPackage(_)), "{err:?}");
    assert!(!f.tmp.path().join("outside.txt").exists());
    assert!(!f.paths().root().join("outside.txt").exists());
    assert!(!f.paths().apps().join("outside.txt").exists());
}

/// Scenario "Manifest giả": metadata không có chữ ký hợp lệ của nền tảng bị từ chối.
#[tokio::test]
async fn fake_manifest_is_rejected() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.2.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    let before = l.state().catalog.clone();

    // Kẻ tấn công ký danh mục bằng khóa của mình (gói "1.2.0" khác) và thay lên máy chủ.
    let attacker = Fixture::new().await;
    attacker.publish_app("soundlab", "1.2.0").await;
    attacker.publish_app("soundlab", "6.6.6").await;
    for file in [
        "2.targets.json",
        "2.snapshot.json",
        "3.targets.json",
        "3.snapshot.json",
        "timestamp.json",
    ] {
        std::fs::copy(
            attacker.repo.metadata_dir().join(file),
            f.repo.metadata_dir().join(file),
        )
        .unwrap();
    }
    let err = l.sync(Timestamp::now()).await.unwrap_err();
    assert!(matches!(err, LauncherError::InvalidMetadata(_)), "{err:?}");
    assert!(!l.is_online());
    assert_eq!(
        l.state().catalog,
        before,
        "danh mục giữ nguyên bản đã xác minh"
    );
    assert!(kinds(l.events()).contains(&"invalid_metadata".to_owned()));
}

/// Scenario "Cập nhật lỗi giữa chừng" và "Quay lại bản trước".
#[tokio::test]
async fn interrupted_update_keeps_old_version_and_rollback_needs_no_download() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.1.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    l.install(&entry(&l, "soundlab", "1.1.0"), Timestamp::now())
        .await
        .unwrap();

    let name = f.publish_app("soundlab", "1.2.0").await;
    l.sync(Timestamp::now()).await.unwrap();
    let update = l.my_apps()[0].update.clone().expect("có bản cập nhật");
    assert_eq!(update.version, "1.2.0");

    // Mất kết nối khi tải: gói không tới được.
    let stored = f.stored_target(&name, &update.sha256);
    let hidden = stored.with_extension("hidden");
    std::fs::rename(&stored, &hidden).unwrap();
    let err = l.install(&update, Timestamp::now()).await.unwrap_err();
    assert!(!matches!(err, LauncherError::InvalidPackage(_)), "{err:?}");
    let plan = l.prepare_launch("soundlab").expect("bản cũ vẫn mở được");
    assert_eq!(plan.version, "1.1.0");
    let leftovers: Vec<String> = std::fs::read_dir(f.paths().app("soundlab"))
        .unwrap()
        .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(leftovers, ["1.1.0"], "không còn thư mục tạm");

    // Lần kiểm tra sau có mạng: cập nhật thành công, bản cũ được giữ để quay lại.
    std::fs::rename(&hidden, &stored).unwrap();
    let results = l.update_all(Timestamp::now()).await;
    assert!(results.iter().all(|(_, r)| r.is_ok()), "{results:?}");
    assert_eq!(l.prepare_launch("soundlab").unwrap().version, "1.2.0");
    assert_eq!(l.my_apps()[0].previous_version.as_deref(), Some("1.1.0"));
    assert!(!l.due_for_update_check(Timestamp::now()));
    assert!(l.due_for_update_check(Timestamp::now() + SignedDuration::from_hours(6)));

    // "Quay lại 1.1.0" khi không có mạng: không cần tải lại.
    std::fs::remove_dir_all(f.repo.dir()).unwrap();
    l.rollback("soundlab").unwrap();
    let plan = l.prepare_launch("soundlab").unwrap();
    assert_eq!(plan.version, "1.1.0");
    assert_eq!(
        std::fs::read_to_string(plan.program).unwrap(),
        "@echo soundlab 1.1.0"
    );
    assert_eq!(l.my_apps()[0].previous_version.as_deref(), Some("1.2.0"));
}

/// Scenario "Phát hành 10%" phía launcher: thiết bị ngoài nhóm vẫn ở bản cũ; vào nhóm thì
/// được đề nghị bản mới.
#[tokio::test]
async fn staged_rollout_limits_offered_update() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.2.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    l.install(&entry(&l, "soundlab", "1.2.0"), Timestamp::now())
        .await
        .unwrap();
    f.publish_app("soundlab", "1.3.0").await;
    l.sync(Timestamp::now()).await.unwrap();

    let device = l.device_id().unwrap();
    assert_eq!(device.len(), 32);
    assert_eq!(l.device_id().unwrap(), device, "mã thiết bị không đổi");
    // Máy chủ: thiết bị này vẫn ở 1.2.0 (ngoài 10%).
    l.set_rollout("soundlab", Some("1.2.0".into())).unwrap();
    assert!(l.my_apps()[0].update.is_none());
    assert!(l.update_all(Timestamp::now()).await.is_empty());
    // Máy chủ: thiết bị này được nhận 1.3.0.
    l.set_rollout("soundlab", Some("1.3.0".into())).unwrap();
    assert_eq!(l.my_apps()[0].update.as_ref().unwrap().version, "1.3.0");
    // Không có thông tin phát hành dần (app không qua danh mục): nhận bản mới nhất đã ký.
    l.set_rollout("soundlab", None).unwrap();
    assert_eq!(l.update_for("soundlab").unwrap().version, "1.3.0");
}

/// Scenario "Không có mạng": "App của tôi" vẫn hiện và mở được; danh mục báo ngoại tuyến.
#[tokio::test]
async fn offline_start_keeps_my_apps_usable() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.2.0").await;
    {
        let mut l = f.launcher().await;
        l.sync(Timestamp::now()).await.unwrap();
        l.install(&entry(&l, "soundlab", "1.2.0"), Timestamp::now())
            .await
            .unwrap();
        l.set_user_name(Some("Lan".into())).unwrap();
    }
    // Khởi động lại launcher khi đã ngắt mạng.
    let mut l = f.offline_launcher().await;
    let err = l.sync(Timestamp::now()).await.unwrap_err();
    assert!(matches!(err, LauncherError::Offline(_)), "{err:?}");
    assert!(err.to_string().starts_with("Đang ngoại tuyến"));
    assert!(!l.is_online());
    assert_eq!(l.my_apps().len(), 1);
    assert_eq!(l.prepare_launch("soundlab").unwrap().version, "1.2.0");
    assert_eq!(l.state().user_name.as_deref(), Some("Lan"));
    assert!(!kinds(l.events()).contains(&"invalid_metadata".to_owned()));
}

/// Scenario "Phiên bản bị thu hồi": sau lần đồng bộ kế tiếp không cho mở, hiện lý do.
#[tokio::test]
async fn revoked_version_is_blocked_after_next_sync() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.1.0").await;
    let bad = f.publish_app("soundlab", "1.2.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    l.install(&entry(&l, "soundlab", "1.1.0"), Timestamp::now())
        .await
        .unwrap();
    l.install(&entry(&l, "soundlab", "1.2.0"), Timestamp::now())
        .await
        .unwrap();

    f.revoke(&bad, "Phát hiện mã độc").await;
    assert!(l.prepare_launch("soundlab").is_ok(), "chưa đồng bộ");
    l.sync(Timestamp::now()).await.unwrap();
    let err = l.prepare_launch("soundlab").unwrap_err();
    assert!(matches!(err, LauncherError::Revoked { .. }), "{err:?}");
    assert!(err.to_string().contains("Phát hiện mã độc"), "{err}");
    assert_eq!(
        l.my_apps()[0].status,
        AppStatus::Revoked {
            reason: "Phát hiện mã độc".into()
        }
    );
    // Trạng thái thu hồi được lưu: khởi động lại khi ngoại tuyến vẫn chặn.
    let offline = f.offline_launcher().await;
    assert!(offline.prepare_launch("soundlab").is_err());
    // Đề xuất: quay lại bản an toàn (hoặc gỡ).
    l.rollback("soundlab").unwrap();
    assert_eq!(l.prepare_launch("soundlab").unwrap().version, "1.1.0");
    // Không cài lại được bản đã thu hồi.
    assert!(l.state().catalog.iter().all(|e| e.version != "1.2.0"));
}

/// Scenario "Gỡ app": xóa thư mục cài đặt và dữ liệu cục bộ, app khác không bị ảnh hưởng.
#[tokio::test]
async fn uninstall_removes_only_that_app() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.2.0").await;
    f.publish_app("paintbox", "0.9.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    for (app, v) in [("soundlab", "1.2.0"), ("paintbox", "0.9.0")] {
        l.install(&entry(&l, app, v), Timestamp::now())
            .await
            .unwrap();
        let data = l.prepare_launch(app).unwrap().data_dir;
        std::fs::write(data.join("settings.json"), b"{}").unwrap();
    }

    l.uninstall("soundlab", true).unwrap();
    assert!(!f.paths().app("soundlab").exists());
    assert!(!f.paths().app_data("soundlab").exists());
    assert!(l.prepare_launch("paintbox").is_ok());
    assert!(f
        .paths()
        .app_data("paintbox")
        .join("settings.json")
        .exists());

    // Mặc định giữ dữ liệu cục bộ.
    l.uninstall("paintbox", false).unwrap();
    assert!(!f.paths().app("paintbox").exists());
    assert!(f
        .paths()
        .app_data("paintbox")
        .join("settings.json")
        .exists());
    assert!(l.my_apps().is_empty());
    assert!(matches!(
        l.uninstall("paintbox", false),
        Err(LauncherError::NotInstalled(_))
    ));
}

/// Mở app: chỉ truyền mã khởi chạy và thông tin của chính app đó, không truyền biến của launcher.
#[tokio::test]
async fn launch_command_passes_only_app_specific_values() {
    let f = Fixture::new().await;
    f.publish_app("soundlab", "1.2.0").await;
    let mut l = f.launcher().await;
    l.sync(Timestamp::now()).await.unwrap();
    l.install(&entry(&l, "soundlab", "1.2.0"), Timestamp::now())
        .await
        .unwrap();
    let plan = l.prepare_launch("soundlab").unwrap();
    let cmd = command(
        &plan,
        &LaunchEnv {
            launch_code: Some("ma-mot-lan".into()),
            server_url: "http://127.0.0.1:8080/".into(),
            issuer_url: "http://127.0.0.1:4444/".into(),
            broker_url: Some("http://127.0.0.1:5000/".into()),
            broker_secret: Some("bi-mat".into()),
        },
    );
    let envs: std::collections::HashMap<String, Option<String>> = cmd
        .get_envs()
        .map(|(k, v)| {
            (
                k.to_string_lossy().into_owned(),
                v.map(|v| v.to_string_lossy().into_owned()),
            )
        })
        .collect();
    assert_eq!(envs["FECO_LAUNCH_CODE"].as_deref(), Some("ma-mot-lan"));
    assert_eq!(envs["FECO_CLIENT_ID"].as_deref(), Some("client-soundlab"));
    assert_eq!(envs["FECO_LAUNCHER_SECRET"].as_deref(), Some("bi-mat"));
    assert_eq!(cmd.get_program(), plan.program.as_os_str());
}

/// Scenario "App xin token của app khác": API cục bộ từ chối client ID không khớp.
#[tokio::test]
async fn broker_refuses_codes_for_other_apps() {
    let tmp = tempfile::tempdir().unwrap();
    let events = EventLog::new(tmp.path().join("events.jsonl"));
    let issuer: Issuer = Arc::new(|client_id: String| {
        Box::pin(async move {
            Ok(LaunchCode {
                code: format!("ma-cho-{client_id}"),
                expires_in: 60,
            })
        })
    });
    let broker = Broker::new(issuer, events.clone());
    let addr = broker.serve().await.unwrap();
    let secret_a = broker.register("client-a");
    let http = reqwest::Client::new();
    let ask = |secret: &str, client: &str| {
        http.post(format!("http://{addr}/v1/launch-code"))
            .bearer_auth(secret)
            .json(&serde_json::json!({ "client_id": client }))
            .send()
    };

    let res = ask(&secret_a, "client-b").await.unwrap();
    assert_eq!(res.status(), 403);
    let body: serde_json::Value = res.json().await.unwrap();
    assert_eq!(body["error"], "client_mismatch");
    assert!(kinds(&events).contains(&"token_request_for_other_app".to_owned()));

    let res = ask("bi-mat-doan-bua", "client-a").await.unwrap();
    assert_eq!(res.status(), 401);

    let res = ask(&secret_a, "client-a").await.unwrap();
    assert_eq!(res.status(), 200);
    let code: LaunchCode = res.json().await.unwrap();
    assert_eq!(code.code, "ma-cho-client-a");
}
