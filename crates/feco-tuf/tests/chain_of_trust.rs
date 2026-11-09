//! Kiểm chuỗi tin cậy đầu-cuối: máy chủ ký bằng `Repo`, launcher giả lập xác minh bằng `Client`
//! (cả hai dùng `tough`). Mỗi test dùng một kho riêng trong thư mục tạm.

use feco_tuf::client::SYNC_INTERVAL;
use feco_tuf::{
    keys, Client, Error, Expiry, InitKeys, Key, Repo, RoleType, RootChange, TargetMeta,
};
use jiff::{SignedDuration, Timestamp};
use std::path::{Path, PathBuf};
use tempfile::TempDir;
use tough::key_source::KeySource;

struct Fixture {
    tmp: TempDir,
    repo: Repo,
    root: [PathBuf; 3],
    targets: PathBuf,
    snapshot: PathBuf,
    timestamp: PathBuf,
}

async fn new_key(dir: &Path, name: &str) -> PathBuf {
    let path = dir.join("keys").join(format!("{name}.pk8"));
    keys::generate(&path).await.expect("tạo khóa");
    path
}

async fn public(path: &Path) -> Key {
    keys::public_key(keys::source(path).as_ref())
        .await
        .expect("khóa công khai")
}

fn signers(paths: &[&PathBuf]) -> Vec<Box<dyn KeySource>> {
    paths.iter().map(|p| keys::source(p.as_path())).collect()
}

impl Fixture {
    async fn new() -> Self {
        let tmp = tempfile::tempdir().expect("thư mục tạm");
        let dir = tmp.path().to_path_buf();
        let root = [
            new_key(&dir, "root-1").await,
            new_key(&dir, "root-2").await,
            new_key(&dir, "root-3").await,
        ];
        let targets = new_key(&dir, "targets").await;
        let snapshot = new_key(&dir, "snapshot").await;
        let timestamp = new_key(&dir, "timestamp").await;
        let repo = Repo::open(dir.join("repo"));
        let fixture = Self {
            tmp,
            repo,
            root,
            targets,
            snapshot,
            timestamp,
        };
        let init_keys = fixture.init_keys().await;
        // Buổi ký root đầu tiên: chỉ 2 trong 3 người giữ khóa root có mặt.
        fixture
            .repo
            .init(
                &init_keys,
                &signers(&[
                    &fixture.root[0],
                    &fixture.root[1],
                    &fixture.targets,
                    &fixture.snapshot,
                    &fixture.timestamp,
                ]),
                Expiry::default(),
                Timestamp::now(),
            )
            .await
            .expect("dựng kho");
        fixture
    }

    async fn init_keys(&self) -> InitKeys {
        InitKeys {
            root: vec![
                public(&self.root[0]).await,
                public(&self.root[1]).await,
                public(&self.root[2]).await,
            ],
            root_threshold: 2,
            targets: vec![public(&self.targets).await],
            snapshot: vec![public(&self.snapshot).await],
            timestamp: vec![public(&self.timestamp).await],
        }
    }

    /// Khóa của dịch vụ phát hành: targets (KMS) cùng snapshot và timestamp (trực tuyến).
    fn release_keys(&self) -> Vec<Box<dyn KeySource>> {
        signers(&[&self.targets, &self.snapshot, &self.timestamp])
    }

    fn online_keys(&self) -> Vec<Box<dyn KeySource>> {
        signers(&[&self.snapshot, &self.timestamp])
    }

    async fn client(&self, name: &str) -> Client {
        Client::for_local_repo(
            &self.repo,
            &self.repo.root_path(1),
            self.tmp.path().join("clients").join(name),
        )
        .await
        .expect("client")
    }

    async fn package(&self, name: &str, content: &[u8]) -> PathBuf {
        let dir = self.tmp.path().join("uploads");
        tokio::fs::create_dir_all(&dir).await.unwrap();
        let path = dir.join(name);
        tokio::fs::write(&path, content).await.unwrap();
        path
    }

    async fn publish(&self, name: &str, app: &str, version: &str, content: &[u8]) {
        let file = self.package(name, content).await;
        self.repo
            .publish(
                &file,
                name,
                &meta(app, version),
                &self.release_keys(),
                Expiry::default(),
                Timestamp::now(),
            )
            .await
            .expect("phát hành target");
    }
}

fn meta(app: &str, version: &str) -> TargetMeta {
    TargetMeta {
        app: app.into(),
        version: version.into(),
        platform: "windows-x64".into(),
        ..Default::default()
    }
}

fn tuf_error(err: &Error) -> String {
    match err.tuf() {
        Some(e) => format!("{e:?}"),
        None => format!("{err:?}"),
    }
}

fn copy_dir(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).unwrap();
    for entry in std::fs::read_dir(from).unwrap() {
        let entry = entry.unwrap();
        std::fs::copy(entry.path(), to.join(entry.file_name())).unwrap();
    }
}

const SOUNDLAB: &str = "soundlab-1.2.0-windows-x64.zip";

#[tokio::test]
async fn init_requires_two_of_three_root_keys() {
    let fixture = Fixture::new().await;
    let other = Repo::open(fixture.tmp.path().join("other"));
    let err = other
        .init(
            &fixture.init_keys().await,
            &signers(&[
                &fixture.root[0],
                &fixture.targets,
                &fixture.snapshot,
                &fixture.timestamp,
            ]),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("1/3 khóa root không đủ để dựng kho");
    assert!(err.to_string().contains("chưa đủ chữ ký"), "{err}");
    assert!(!other.root_path(1).exists());

    let root = std::fs::read_to_string(fixture.repo.root_path(1)).unwrap();
    let root: serde_json::Value = serde_json::from_str(&root).unwrap();
    assert_eq!(root["signed"]["roles"]["root"]["threshold"], 2);
    assert_eq!(
        root["signed"]["roles"]["root"]["keyids"]
            .as_array()
            .unwrap()
            .len(),
        3
    );
    assert_eq!(root["signatures"].as_array().unwrap().len(), 2);
    for role in ["targets", "snapshot", "timestamp"] {
        assert_eq!(root["signed"]["roles"][role]["threshold"], 1, "{role}");
    }
}

#[tokio::test]
async fn published_target_is_verified_and_downloaded() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;

    let client = fixture.client("launcher").await;
    let state = client.sync().await.expect("đồng bộ");
    assert_eq!(state.root_version, 1);
    assert_eq!(state.targets_version, 2);
    let info = &state.targets[SOUNDLAB];
    assert_eq!(info.length, 14);
    assert_eq!(info.meta, Some(meta("soundlab", "1.2.0")));
    assert!(state.revocations.is_empty());
    assert_eq!(client.download(SOUNDLAB).await.unwrap(), b"SoundLab 1.2.0");

    // Không thay được nội dung của một phiên bản đã phát hành.
    let file = fixture.package("again.zip", b"khac").await;
    let err = fixture
        .repo
        .publish(
            &file,
            SOUNDLAB,
            &meta("soundlab", "1.2.0"),
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("trùng tên target");
    assert!(err.to_string().contains("đã tồn tại"), "{err}");

    // Tên target có thư mục con hoặc ký tự lạ bị từ chối.
    let err = fixture
        .repo
        .publish(
            &file,
            "../evil.zip",
            &meta("evil", "1"),
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("tên target không hợp lệ");
    assert!(err.to_string().contains("không hợp lệ"), "{err}");

    // Thiếu khóa targets thì không phát hành được.
    let err = fixture
        .repo
        .publish(
            &file,
            "soundlab-1.3.0-windows-x64.zip",
            &meta("soundlab", "1.3.0"),
            &fixture.online_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("thiếu khóa targets");
    assert!(err.to_string().contains("thiếu khóa targets"), "{err}");
}

#[tokio::test]
async fn tampered_package_is_rejected() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let client = fixture.client("launcher").await;
    let info = client.sync().await.unwrap().targets[SOUNDLAB].clone();
    let stored = fixture
        .repo
        .targets_dir()
        .join(format!("{}.{SOUNDLAB}", info.sha256));

    // Cùng độ dài, khác nội dung: sai SHA-256.
    std::fs::write(&stored, b"SoundLab 6.6.6").unwrap();
    let err = client.download(SOUNDLAB).await.expect_err("sai hash");
    assert!(tuf_error(&err).contains("HashMismatch"), "{err:?}");

    // Dài hơn độ dài đã ký.
    std::fs::write(&stored, b"SoundLab 1.2.0 + malware").unwrap();
    client.download(SOUNDLAB).await.expect_err("vượt độ dài");

    // Tải thẳng vào file: file không khớp bị xóa.
    let dest = fixture.tmp.path().join("download.zip");
    client
        .download_to(SOUNDLAB, &dest)
        .await
        .expect_err("gói bị sửa");
    assert!(!dest.exists(), "file không khớp phải bị xóa");
    std::fs::write(&stored, b"SoundLab 1.2.0").unwrap();
    assert_eq!(client.download_to(SOUNDLAB, &dest).await.unwrap(), 14);
    assert_eq!(std::fs::read(&dest).unwrap(), b"SoundLab 1.2.0");
}

#[tokio::test]
async fn tampered_or_foreign_metadata_is_rejected() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let metadata = fixture.repo.metadata_dir();
    let targets_path = metadata.join("2.targets.json");
    let original = std::fs::read_to_string(&targets_path).unwrap();

    // Sửa độ dài trong targets.json đã ký.
    let edited = original.replace("\"length\": 14", "\"length\": 15");
    assert_ne!(edited, original);
    std::fs::write(&targets_path, edited).unwrap();
    let err = fixture
        .client("a")
        .await
        .sync()
        .await
        .expect_err("metadata bị sửa");
    // snapshot.json ghi SHA-256 của targets.json, nên bản bị sửa không khớp hash.
    assert!(tuf_error(&err).contains("HashMismatch"), "{err:?}");
    std::fs::write(&targets_path, &original).unwrap();
    fixture.client("b").await.sync().await.expect("khôi phục");

    // Kẻ tấn công dựng kho riêng bằng khóa của mình rồi thay metadata (trừ root).
    let attacker = Fixture::new().await;
    attacker
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 6.6.6")
        .await;
    for file in ["2.targets.json", "2.snapshot.json", "timestamp.json"] {
        std::fs::copy(attacker.repo.metadata_dir().join(file), metadata.join(file)).unwrap();
    }
    let err = fixture
        .client("c")
        .await
        .sync()
        .await
        .expect_err("chữ ký lạ");
    let detail = tuf_error(&err);
    assert!(detail.contains("Verify"), "{detail}");
}

#[tokio::test]
async fn rollback_to_older_metadata_is_rejected() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let backup = fixture.tmp.path().join("metadata-v2");
    copy_dir(&fixture.repo.metadata_dir(), &backup);

    fixture
        .publish(
            "soundlab-1.2.1-windows-x64.zip",
            "soundlab",
            "1.2.1",
            b"SoundLab 1.2.1",
        )
        .await;
    let client = fixture.client("launcher").await;
    assert_eq!(client.sync().await.unwrap().targets_version, 3);

    // Máy chủ bị chiếm phục vụ lại metadata cũ (hợp lệ chữ ký, chưa hết hạn).
    std::fs::remove_dir_all(fixture.repo.metadata_dir()).unwrap();
    copy_dir(&backup, &fixture.repo.metadata_dir());
    let err = client.sync().await.expect_err("rollback");
    assert!(tuf_error(&err).contains("OlderMetadata"), "{err:?}");
}

#[tokio::test]
async fn frozen_metadata_expires_within_one_hour() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let now = Timestamp::now();
    let state = fixture.client("fresh").await.sync().await.unwrap();
    assert!(state.expires <= now + SignedDuration::from_hours(1) + SignedDuration::from_mins(1));

    // Metadata được ký lần cuối cách đây 2 giờ và không được làm mới (bị "đóng băng").
    fixture
        .repo
        .refresh(
            &fixture.online_keys(),
            Expiry::default(),
            now - SignedDuration::from_hours(2),
        )
        .await
        .unwrap();
    let err = fixture
        .client("frozen")
        .await
        .sync()
        .await
        .expect_err("metadata hết hạn");
    assert!(tuf_error(&err).contains("ExpiredMetadata"), "{err:?}");

    // Ký lại timestamp bằng khóa trực tuyến (không cần khóa targets) là hết lỗi.
    fixture
        .repo
        .refresh(&fixture.online_keys(), Expiry::default(), Timestamp::now())
        .await
        .unwrap();
    let state = fixture.client("frozen").await.sync().await.unwrap();
    assert_eq!(state.targets_version, 2);
    assert_eq!(state.timestamp_version, 4);

    // Gia hạn targets khi không có phát hành mới: cần khóa targets, nội dung giữ nguyên.
    fixture
        .repo
        .resign(&fixture.online_keys(), Expiry::default(), Timestamp::now())
        .await
        .expect_err("thiếu khóa targets");
    fixture
        .repo
        .resign(&fixture.release_keys(), Expiry::default(), Timestamp::now())
        .await
        .unwrap();
    let state = fixture.client("frozen").await.sync().await.unwrap();
    assert_eq!(state.targets_version, 3);
    assert!(state.targets.contains_key(SOUNDLAB));
}

#[tokio::test]
async fn revocation_reaches_client_on_next_hourly_sync() {
    assert_eq!(SYNC_INTERVAL.as_secs(), 3600);
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let client = fixture.client("launcher").await;
    assert!(client.sync().await.unwrap().targets.contains_key(SOUNDLAB));

    let revocation = fixture
        .repo
        .revoke(
            SOUNDLAB,
            "Phát hiện mã độc",
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .unwrap();
    assert_eq!(revocation.app.as_deref(), Some("soundlab"));
    assert_eq!(revocation.version.as_deref(), Some("1.2.0"));

    // Lần đồng bộ kế tiếp (tối đa SYNC_INTERVAL sau khi thu hồi).
    let state = client.sync().await.unwrap();
    assert!(!state.targets.contains_key(SOUNDLAB));
    let seen = state.revocation(SOUNDLAB).expect("lý do thu hồi");
    assert_eq!(seen.reason, "Phát hiện mã độc");
    let err = client.download(SOUNDLAB).await.expect_err("đã thu hồi");
    assert!(err.to_string().contains("thu hồi"), "{err}");

    // Không phát hành lại được cùng tên, không thu hồi lại được.
    let file = fixture.package("x.zip", b"x").await;
    fixture
        .repo
        .publish(
            &file,
            SOUNDLAB,
            &meta("soundlab", "1.2.0"),
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("tên đã thu hồi");
    fixture
        .repo
        .revoke(
            SOUNDLAB,
            "lần hai",
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("đã thu hồi rồi");
}

#[tokio::test]
async fn targets_key_rotation_drill() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let client = fixture.client("launcher").await;
    client.sync().await.unwrap();

    let new_targets = new_key(fixture.tmp.path(), "targets-2").await;
    let draft = fixture
        .repo
        .draft_root_update(
            &[RootChange {
                role: RoleType::Targets,
                keys: vec![public(&new_targets).await],
                threshold: 1,
            }],
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .unwrap();
    let signed = draft
        .sign(&signers(&[&fixture.root[0], &fixture.root[2]]))
        .await
        .unwrap();
    fixture
        .repo
        .rotate(
            &draft,
            &signed,
            &signers(&[&new_targets, &fixture.snapshot, &fixture.timestamp]),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect("xoay khóa targets");

    let state = client.sync().await.expect("client theo root mới");
    assert_eq!(state.root_version, 2);
    assert!(state.targets.contains_key(SOUNDLAB));
    assert_eq!(client.download(SOUNDLAB).await.unwrap(), b"SoundLab 1.2.0");

    // Khóa targets cũ hết hiệu lực; khóa mới phát hành được.
    let file = fixture.package("n.zip", b"SoundLab 1.3.0").await;
    let name = "soundlab-1.3.0-windows-x64.zip";
    fixture
        .repo
        .publish(
            &file,
            name,
            &meta("soundlab", "1.3.0"),
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("khóa targets cũ");
    fixture
        .repo
        .publish(
            &file,
            name,
            &meta("soundlab", "1.3.0"),
            &signers(&[&new_targets, &fixture.snapshot, &fixture.timestamp]),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect("khóa targets mới");
    assert!(client.sync().await.unwrap().targets.contains_key(name));
    // Launcher mới cài (chỉ có 1.root.json) vẫn đi theo chuỗi root lên phiên bản 2.
    let fresh = fixture.client("fresh-install").await.sync().await.unwrap();
    assert_eq!(fresh.root_version, 2);
}

#[tokio::test]
async fn root_key_rotation_drill_needs_two_of_three() {
    let fixture = Fixture::new().await;
    fixture
        .publish(SOUNDLAB, "soundlab", "1.2.0", b"SoundLab 1.2.0")
        .await;
    let client = fixture.client("launcher").await;
    client.sync().await.unwrap();

    // Khóa root-3 bị lộ: thay bằng root-4, giữ ngưỡng 2/3.
    let root4 = new_key(fixture.tmp.path(), "root-4").await;
    let new_root_keys = vec![
        public(&fixture.root[0]).await,
        public(&fixture.root[1]).await,
        public(&root4).await,
    ];
    let draft = fixture
        .repo
        .draft_root_update(
            &[RootChange {
                role: RoleType::Root,
                keys: new_root_keys,
                threshold: 2,
            }],
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .unwrap();
    assert_eq!(draft.next().keys.len(), 6);

    // Chỉ một khóa root hiện tại (kể cả khi có thêm khóa mới): bị từ chối.
    let weak = draft
        .sign(&signers(&[&fixture.root[0], &root4]))
        .await
        .unwrap();
    let err = draft.check(&weak).expect_err("1/3 khóa root cũ");
    assert!(err.to_string().contains("root hiện tại"), "{err}");
    fixture
        .repo
        .rotate(
            &draft,
            &weak,
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect_err("máy chủ không phát hành root thiếu chữ ký");
    assert!(!fixture.repo.root_path(2).exists());

    // Kẻ tấn công tự đặt root thiếu chữ ký lên máy chủ: client từ chối.
    std::fs::write(
        fixture.repo.root_path(2),
        serde_json::to_vec_pretty(&weak).unwrap(),
    )
    .unwrap();
    let err = client.sync().await.expect_err("root thiếu chữ ký");
    assert!(tuf_error(&err).contains("Verify"), "{err:?}");
    std::fs::remove_file(fixture.repo.root_path(2)).unwrap();

    // Hai người giữ khóa root hiện tại ký: hợp lệ.
    let signed = draft
        .sign(&signers(&[&fixture.root[0], &fixture.root[1], &root4]))
        .await
        .unwrap();
    draft.check(&signed).unwrap();
    fixture
        .repo
        .rotate(
            &draft,
            &signed,
            &fixture.release_keys(),
            Expiry::default(),
            Timestamp::now(),
        )
        .await
        .expect("xoay khóa root");
    let state = client.sync().await.unwrap();
    assert_eq!(state.root_version, 2);
    assert!(state.targets.contains_key(SOUNDLAB));

    // Khóa root-3 đã bị loại không còn được tính.
    let draft = fixture
        .repo
        .draft_root_update(&[], Expiry::default(), Timestamp::now())
        .await
        .unwrap();
    let with_old = draft
        .sign(&signers(&[&fixture.root[1], &fixture.root[2]]))
        .await
        .unwrap();
    draft.check(&with_old).expect_err("root-3 đã bị loại");
    let ok = draft
        .sign(&signers(&[&fixture.root[1], &root4]))
        .await
        .unwrap();
    draft.check(&ok).unwrap();
}
