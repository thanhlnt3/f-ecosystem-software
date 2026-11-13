//! Phía máy chủ phân phối: dựng kho, ký target sau kiểm duyệt, thu hồi, ký lại định kỳ, xoay khóa.

use crate::error::{io_err, Error, Result};
use aws_lc_rs::rand::SystemRandom;
use jiff::{SignedDuration, Timestamp};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::{HashMap, HashSet};
use std::num::NonZeroU64;
use std::path::{Path, PathBuf};
use tough::editor::signed::SignedRole;
use tough::editor::RepositoryEditor;
use tough::key_source::KeySource;
use tough::schema::key::Key;
use tough::schema::{
    Hashes, KeyHolder, Metafile, RoleKeys, RoleType, Root, Signature, Signed, Target,
};
use tough::{
    ExpirationEnforcement, FilesystemTransport, IntoVec, Repository, RepositoryLoader, TargetName,
};
use url::Url;

/// Target chứa danh sách phiên bản đã bị thu hồi kèm lý do. Nó là một target bình thường nên
/// được bảo vệ bằng cùng chuỗi chữ ký với các gói cài đặt.
pub const REVOCATIONS_TARGET: &str = "feco-revocations.json";
const SPEC_VERSION: &str = "1.0.0";

/// Thời hạn hiệu lực của từng vai trò kể từ lúc ký.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Expiry {
    pub root: SignedDuration,
    pub targets: SignedDuration,
    pub snapshot: SignedDuration,
    /// Giới hạn trên của cửa sổ tấn công "đóng băng": client từ chối metadata quá hạn này.
    pub timestamp: SignedDuration,
}

impl Default for Expiry {
    fn default() -> Self {
        const DAY: i64 = 24;
        Self {
            root: SignedDuration::from_hours(365 * DAY),
            targets: SignedDuration::from_hours(90 * DAY),
            snapshot: SignedDuration::from_hours(7 * DAY),
            timestamp: SignedDuration::from_hours(1),
        }
    }
}

/// Thông tin của một phiên bản app, ghi vào trường `custom` của target và được ký cùng hash.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct TargetMeta {
    pub app: String,
    pub version: String,
    pub platform: String,
    /// Tên hiển thị của app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub title: Option<String>,
    /// Đường dẫn (trong gói) tới file chạy của app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub entry: Option<String>,
    /// Client ID OAuth của app, để launcher xin mã khởi chạy đúng app.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub client_id: Option<String>,
    /// App riêng tư (bản build bàn giao của đơn xây app): launcher chỉ hiện cho người dùng mà
    /// máy chủ liệt kê ở `GET /v1/catalog/private`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub private: bool,
    /// Mô tả ngắn hiển thị trong launcher (từ `summary` của manifest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub summary: Option<String>,
    /// Nhóm của app trong danh mục, ví dụ `productivity` (từ `category` của manifest).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub category: Option<String>,
    /// Tên nhà phát triển (từ danh mục F-Store).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub developer: Option<String>,
    /// Địa chỉ mã nguồn (HTTPS), ví dụ kho GitHub của app mã nguồn mở.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    /// App chạy trong cửa sổ terminal (launcher mở terminal riêng cho app).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub terminal: bool,
    /// Giới thiệu chung (trang chi tiết trong launcher).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Các tính năng chính, mỗi dòng một tính năng.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub features: Vec<String>,
    /// Website của app (HTTPS).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub website: Option<String>,
    /// Các thay đổi trong phiên bản này.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub release_notes: Option<String>,
    /// Giấy phép (SPDX), ví dụ `MIT OR Apache-2.0`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub license: Option<String>,
    /// Thời điểm phát hành (RFC 3339, UTC).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub released_at: Option<String>,
}

/// Một bản ghi thu hồi trong [`REVOCATIONS_TARGET`].
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct Revocation {
    pub target: String,
    pub app: Option<String>,
    pub version: Option<String>,
    pub reason: String,
    pub revoked_at: Timestamp,
}

/// Khóa công khai của từng vai trò khi dựng kho.
#[derive(Clone, Debug)]
pub struct InitKeys {
    pub root: Vec<Key>,
    pub root_threshold: u64,
    pub targets: Vec<Key>,
    pub snapshot: Vec<Key>,
    pub timestamp: Vec<Key>,
}

/// Thay toàn bộ khóa của một vai trò trong root mới.
#[derive(Clone, Debug)]
pub struct RootChange {
    pub role: RoleType,
    pub keys: Vec<Key>,
    pub threshold: u64,
}

/// Root mới đang chờ ký. Root mới chỉ được chấp nhận khi có đủ ngưỡng chữ ký của cả root cũ
/// lẫn root mới (TUF 5.3.4).
#[derive(Clone, Debug)]
pub struct RootDraft {
    previous: Root,
    next: Root,
}

impl RootDraft {
    pub fn previous(&self) -> &Root {
        &self.previous
    }

    pub fn next(&self) -> &Root {
        &self.next
    }

    /// Ký root mới bằng những khóa root (cũ và mới) có trong `signers`. Ở buổi ký thật, mỗi người
    /// giữ khóa ký trên máy ngoại tuyến của mình; hàm này không tự kiểm ngưỡng, xem [`Self::check`].
    pub async fn sign(&self, signers: &[Box<dyn KeySource>]) -> Result<Signed<Root>> {
        sign_root(&self.next, &[&self.previous, &self.next], signers).await
    }

    /// Kiểm root mới đã đủ ngưỡng chữ ký của root cũ và của chính nó.
    pub fn check(&self, signed: &Signed<Root>) -> Result<()> {
        self.previous.verify_role(signed).map_err(|e| {
            Error::Invalid(format!("root mới chưa đủ chữ ký theo root hiện tại: {e}"))
        })?;
        self.next
            .verify_role(signed)
            .map_err(|e| Error::Invalid(format!("root mới chưa đủ chữ ký theo chính nó: {e}")))?;
        Ok(())
    }
}

/// Kho TUF trên đĩa của máy chủ phân phối.
#[derive(Clone, Debug)]
pub struct Repo {
    dir: PathBuf,
}

impl Repo {
    pub fn open(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn metadata_dir(&self) -> PathBuf {
        self.dir.join("metadata")
    }

    pub fn targets_dir(&self) -> PathBuf {
        self.dir.join("targets")
    }

    pub fn root_path(&self, version: u64) -> PathBuf {
        self.metadata_dir().join(format!("{version}.root.json"))
    }

    pub fn metadata_url(&self) -> Result<Url> {
        dir_url(&self.metadata_dir())
    }

    pub fn targets_url(&self) -> Result<Url> {
        dir_url(&self.targets_dir())
    }

    /// Phiên bản root mới nhất có trong kho.
    pub async fn latest_root_version(&self) -> Result<u64> {
        let dir = self.metadata_dir();
        let mut entries = tokio::fs::read_dir(&dir).await.map_err(io_err(&dir))?;
        let mut latest = 0;
        while let Some(entry) = entries.next_entry().await.map_err(io_err(&dir))? {
            let version = entry
                .file_name()
                .to_str()
                .and_then(|n| n.strip_suffix(".root.json"))
                .and_then(|n| n.parse::<u64>().ok());
            latest = latest.max(version.unwrap_or(0));
        }
        if latest == 0 {
            return Err(Error::Invalid(format!(
                "{} chưa có root.json, hãy chạy init",
                dir.display()
            )));
        }
        Ok(latest)
    }

    /// Dựng kho mới: root phiên bản 1 (ký bởi ít nhất `root_threshold` khóa root), cùng targets,
    /// snapshot và timestamp ban đầu với danh sách thu hồi rỗng.
    pub async fn init(
        &self,
        keys: &InitKeys,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<()> {
        let first = self.root_path(1);
        if tokio::fs::try_exists(&first)
            .await
            .map_err(io_err(&first))?
        {
            return Err(Error::Invalid(format!(
                "{} đã có root.json",
                self.dir.display()
            )));
        }
        let mut root = Root {
            spec_version: SPEC_VERSION.to_owned(),
            consistent_snapshot: true,
            version: NonZeroU64::MIN,
            expires: after(now, expiry.root)?,
            keys: HashMap::new(),
            roles: HashMap::new(),
            _extra: HashMap::new(),
        };
        set_role(&mut root, RoleType::Root, &keys.root, keys.root_threshold)?;
        set_role(&mut root, RoleType::Targets, &keys.targets, 1)?;
        set_role(&mut root, RoleType::Snapshot, &keys.snapshot, 1)?;
        set_role(&mut root, RoleType::Timestamp, &keys.timestamp, 1)?;
        let signed = sign_root(&root, &[&root], signers).await?;
        root.verify_role(&signed)
            .map_err(|e| Error::Invalid(format!("root.json chưa đủ chữ ký: {e}")))?;
        write_root(&first, &signed).await?;

        let result = async {
            let mut editor = RepositoryEditor::new(&first).await?;
            let (name, target) = self.stage_revocations(&[]).await?;
            editor.add_target(name, target)?;
            set_versions(&mut editor, [1, 1, 1], expiry, now)?;
            self.finish(editor, signers).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&first).await;
        }
        result
    }

    /// Đọc kho hiện tại và xác minh toàn bộ chữ ký (bắt đầu từ root phiên bản 1). Không kiểm hạn,
    /// vì máy chủ phải ký lại được cả khi metadata đã hết hạn.
    pub async fn load(&self) -> Result<Repository> {
        let first = self.root_path(1);
        let root = tokio::fs::read(&first).await.map_err(io_err(&first))?;
        Ok(
            RepositoryLoader::new(&root, self.metadata_url()?, self.targets_url()?)
                .transport(FilesystemTransport)
                .expiration_enforcement(ExpirationEnforcement::Unsafe)
                .load()
                .await?,
        )
    }

    /// Ký một phiên bản app đã qua kiểm duyệt thành target mới. Tên target là duy nhất và không
    /// được thay nội dung sau khi phát hành (kể cả sau khi bị thu hồi).
    pub async fn publish(
        &self,
        file: &Path,
        name: &str,
        meta: &TargetMeta,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<()> {
        let repo = self.load().await?;
        let target_name = TargetName::new(name)?;
        let revoked = read_revocations(&repo).await?;
        if name == REVOCATIONS_TARGET
            || repo.targets().signed.targets.contains_key(&target_name)
            || revoked.iter().any(|r| r.target == name)
        {
            return Err(Error::Invalid(format!(
                "target {name} đã tồn tại hoặc đã bị thu hồi; hãy phát hành phiên bản mới"
            )));
        }
        let custom = serde_json::from_value(serde_json::to_value(meta)?)?;
        let (target_name, target) = self.stage_target(name, file, custom).await?;
        let mut editor = self.editor_from(repo, None, expiry, now).await?;
        editor.add_target(target_name, target)?;
        self.finish(editor, signers).await
    }

    /// Thu hồi một target: phát hành metadata mới không còn target đó, và ghi lý do vào
    /// [`REVOCATIONS_TARGET`].
    pub async fn revoke(
        &self,
        name: &str,
        reason: &str,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<Revocation> {
        if reason.trim().is_empty() {
            return Err(Error::Invalid("thu hồi phải có lý do".into()));
        }
        let repo = self.load().await?;
        let target_name = TargetName::new(name)?;
        let target = repo
            .targets()
            .signed
            .targets
            .get(&target_name)
            .filter(|_| name != REVOCATIONS_TARGET)
            .cloned()
            .ok_or_else(|| Error::Invalid(format!("không có target {name} đang phát hành")))?;
        let meta = target_meta(&target);
        let revocation = Revocation {
            target: name.to_owned(),
            app: meta.as_ref().map(|m| m.app.clone()),
            version: meta.map(|m| m.version),
            reason: reason.trim().to_owned(),
            revoked_at: now,
        };
        let mut list = read_revocations(&repo).await?;
        list.push(revocation.clone());
        let (list_name, list_target) = self.stage_revocations(&list).await?;
        let mut editor = self.editor_from(repo, None, expiry, now).await?;
        editor.remove_target(&target_name)?;
        editor.add_target(list_name, list_target)?;
        self.finish(editor, signers).await?;
        Ok(revocation)
    }

    /// Ký lại snapshot và timestamp bằng khóa trực tuyến (không cần khóa targets). Máy chủ chạy
    /// việc này định kỳ, thường hơn nhiều so với thời hạn của timestamp.
    pub async fn refresh(
        &self,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<()> {
        let repo = self.load().await?;
        let holder = KeyHolder::Root(repo.root().signed.clone());
        let rng = SystemRandom::new();

        let mut snapshot = repo.snapshot().signed.clone();
        snapshot.version = next_version(snapshot.version)?;
        snapshot.expires = after(now, expiry.snapshot)?;
        let snapshot = SignedRole::new(snapshot, &holder, signers, &rng).await?;

        let mut timestamp = repo.timestamp().signed.clone();
        timestamp.version = next_version(timestamp.version)?;
        timestamp.expires = after(now, expiry.timestamp)?;
        timestamp.meta.insert(
            "snapshot.json".to_owned(),
            Metafile {
                length: Some(*snapshot.length()),
                hashes: Some(Hashes {
                    sha256: snapshot.sha256().to_vec().into(),
                    _extra: HashMap::new(),
                }),
                version: snapshot.signed().signed.version,
                _extra: HashMap::new(),
            },
        );
        let timestamp = SignedRole::new(timestamp, &holder, signers, &rng).await?;

        let dir = self.metadata_dir();
        snapshot.write(&dir, true).await?;
        timestamp.write(&dir, true).await?;
        Ok(())
    }

    /// Ký lại targets, snapshot và timestamp mà không đổi nội dung, để gia hạn targets (cần khóa
    /// targets). Chạy định kỳ, ví dụ mỗi tháng, khi không có phiên bản mới nào được phát hành.
    pub async fn resign(
        &self,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<()> {
        let repo = self.load().await?;
        let editor = self.editor_from(repo, None, expiry, now).await?;
        self.finish(editor, signers).await
    }

    /// Chuẩn bị root mới (phiên bản kế tiếp) với các thay đổi khóa đã cho.
    pub async fn draft_root_update(
        &self,
        changes: &[RootChange],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<RootDraft> {
        let version = self.latest_root_version().await?;
        let path = self.root_path(version);
        let bytes = tokio::fs::read(&path).await.map_err(io_err(&path))?;
        let previous = serde_json::from_slice::<Signed<Root>>(&bytes)?.signed;
        let mut next = previous.clone();
        next.version = next_version(previous.version)?;
        next.expires = after(now, expiry.root)?;
        for change in changes {
            set_role(&mut next, change.role, &change.keys, change.threshold)?;
        }
        let used: HashSet<_> = next
            .roles
            .values()
            .flat_map(|r| r.keyids.iter().cloned())
            .collect();
        next.keys.retain(|id, _| used.contains(id));
        Ok(RootDraft { previous, next })
    }

    /// Phát hành root mới đã ký đủ ngưỡng, rồi ký lại targets, snapshot và timestamp theo root
    /// mới (`signers` phải có khóa targets, snapshot, timestamp đang hiệu lực theo root mới).
    pub async fn rotate(
        &self,
        draft: &RootDraft,
        signed_root: &Signed<Root>,
        signers: &[Box<dyn KeySource>],
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<()> {
        draft.check(signed_root)?;
        let version = draft.next.version.get();
        if version != self.latest_root_version().await? + 1 {
            return Err(Error::Invalid(
                "bản nháp root đã cũ: kho đã có root mới hơn".into(),
            ));
        }
        let repo = self.load().await?;
        let path = self.root_path(version);
        write_root(&path, signed_root).await?;
        let result = async {
            let editor = self.editor_from(repo, Some(version), expiry, now).await?;
            self.finish(editor, signers).await
        }
        .await;
        if result.is_err() {
            let _ = tokio::fs::remove_file(&path).await;
        }
        result
    }

    async fn editor_from(
        &self,
        repo: Repository,
        root_version: Option<u64>,
        expiry: Expiry,
        now: Timestamp,
    ) -> Result<RepositoryEditor> {
        let root_version = match root_version {
            Some(v) => v,
            None => self.latest_root_version().await?,
        };
        let versions = [
            repo.targets().signed.version.get() + 1,
            repo.snapshot().signed.version.get() + 1,
            repo.timestamp().signed.version.get() + 1,
        ];
        let mut editor = RepositoryEditor::from_repo(self.root_path(root_version), repo).await?;
        set_versions(&mut editor, versions, expiry, now)?;
        Ok(editor)
    }

    async fn finish(&self, editor: RepositoryEditor, signers: &[Box<dyn KeySource>]) -> Result<()> {
        let signed = editor.sign(signers).await.map_err(|e| match e {
            tough::error::Error::SigningKeysNotFound { .. } => Error::Invalid(format!(
                "thiếu khóa targets, snapshot hoặc timestamp hợp lệ theo root hiện tại ({e})"
            )),
            other => other.into(),
        })?;
        signed.write(self.metadata_dir()).await?;
        Ok(())
    }

    /// Chép file vào `targets/<sha256>.<tên>` và tạo target tương ứng.
    async fn stage_target(
        &self,
        name: &str,
        source: &Path,
        custom: HashMap<String, Value>,
    ) -> Result<(TargetName, Target)> {
        validate_target_name(name)?;
        let mut target = Target::from_path(source).await?;
        target.custom = custom;
        let dir = self.targets_dir();
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(io_err(&dir))?;
        let dest = dir.join(format!("{}.{name}", hex::encode(&target.hashes.sha256)));
        if !tokio::fs::try_exists(&dest).await.map_err(io_err(&dest))? {
            let tmp = dest.with_extension("partial");
            tokio::fs::copy(source, &tmp)
                .await
                .map_err(io_err(source))?;
            tokio::fs::rename(&tmp, &dest)
                .await
                .map_err(io_err(&dest))?;
        }
        Ok((TargetName::new(name)?, target))
    }

    async fn stage_revocations(&self, list: &[Revocation]) -> Result<(TargetName, Target)> {
        let mut bytes = serde_json::to_vec_pretty(list)?;
        bytes.push(b'\n');
        tokio::fs::create_dir_all(&self.dir)
            .await
            .map_err(io_err(&self.dir))?;
        let tmp = self.dir.join(format!(".staging.{REVOCATIONS_TARGET}"));
        tokio::fs::write(&tmp, &bytes).await.map_err(io_err(&tmp))?;
        let staged = self
            .stage_target(REVOCATIONS_TARGET, &tmp, HashMap::new())
            .await;
        let _ = tokio::fs::remove_file(&tmp).await;
        staged
    }
}

/// Đọc danh sách thu hồi đã được xác minh từ kho.
pub(crate) async fn read_revocations(repo: &Repository) -> Result<Vec<Revocation>> {
    let name = TargetName::new(REVOCATIONS_TARGET)?;
    match repo.read_target(&name).await? {
        Some(stream) => Ok(serde_json::from_slice(&stream.into_vec().await?)?),
        None => Ok(Vec::new()),
    }
}

pub(crate) fn target_meta(target: &Target) -> Option<TargetMeta> {
    let map = target.custom.clone().into_iter().collect();
    serde_json::from_value(Value::Object(map)).ok()
}

/// Tên target chỉ gồm chữ, số, `.`, `-`, `_` (không có thư mục con), ví dụ
/// `soundlab-1.2.0-windows-x64.zip`.
pub fn validate_target_name(name: &str) -> Result<()> {
    let valid = !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    if valid {
        Ok(())
    } else {
        Err(Error::Invalid(format!(
            "tên target không hợp lệ: {name:?} (chỉ dùng chữ, số, '.', '-', '_')"
        )))
    }
}

fn set_role(root: &mut Root, role: RoleType, keys: &[Key], threshold: u64) -> Result<()> {
    let mut keyids = Vec::new();
    for key in keys {
        let id = key.key_id()?;
        root.keys.insert(id.clone(), key.clone());
        if !keyids.contains(&id) {
            keyids.push(id);
        }
    }
    let threshold = NonZeroU64::new(threshold)
        .filter(|t| t.get() <= keyids.len() as u64)
        .ok_or_else(|| {
            Error::Invalid(format!(
                "ngưỡng {threshold} không hợp lệ cho {} khóa của vai trò {role:?}",
                keyids.len()
            ))
        })?;
    root.roles.insert(
        role,
        RoleKeys {
            keyids,
            threshold,
            _extra: HashMap::new(),
        },
    );
    Ok(())
}

async fn sign_root(
    root: &Root,
    holders: &[&Root],
    signers: &[Box<dyn KeySource>],
) -> Result<Signed<Root>> {
    let rng = SystemRandom::new();
    let mut signatures: Vec<Signature> = Vec::new();
    for holder in holders {
        let holder = KeyHolder::Root((*holder).clone());
        let signed = SignedRole::new(root.clone(), &holder, signers, &rng).await?;
        for signature in &signed.signed().signatures {
            if !signatures.iter().any(|s| s.keyid == signature.keyid) {
                signatures.push(signature.clone());
            }
        }
    }
    Ok(Signed {
        signed: root.clone(),
        signatures,
    })
}

async fn write_root(path: &Path, signed: &Signed<Root>) -> Result<()> {
    if tokio::fs::try_exists(path).await.map_err(io_err(path))? {
        return Err(Error::Invalid(format!(
            "{} đã tồn tại; root đã phát hành không được ghi đè",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(io_err(parent))?;
    }
    let mut bytes = serde_json::to_vec_pretty(signed)?;
    bytes.push(b'\n');
    let tmp = path.with_extension("partial");
    tokio::fs::write(&tmp, &bytes).await.map_err(io_err(&tmp))?;
    tokio::fs::rename(&tmp, path).await.map_err(io_err(path))
}

fn set_versions(
    editor: &mut RepositoryEditor,
    [targets, snapshot, timestamp]: [u64; 3],
    expiry: Expiry,
    now: Timestamp,
) -> Result<()> {
    editor
        .targets_version(non_zero(targets)?)?
        .targets_expires(after(now, expiry.targets)?)?;
    editor
        .snapshot_version(non_zero(snapshot)?)
        .snapshot_expires(after(now, expiry.snapshot)?)
        .timestamp_version(non_zero(timestamp)?)
        .timestamp_expires(after(now, expiry.timestamp)?);
    Ok(())
}

fn non_zero(v: u64) -> Result<NonZeroU64> {
    NonZeroU64::new(v).ok_or_else(|| Error::Invalid("phiên bản metadata phải lớn hơn 0".into()))
}

fn next_version(v: NonZeroU64) -> Result<NonZeroU64> {
    v.checked_add(1)
        .ok_or_else(|| Error::Invalid("phiên bản metadata bị tràn".into()))
}

fn after(now: Timestamp, duration: SignedDuration) -> Result<Timestamp> {
    now.checked_add(duration)
        .map_err(|e| Error::Invalid(format!("thời điểm hết hạn không hợp lệ: {e}")))
}

fn dir_url(path: &Path) -> Result<Url> {
    let absolute = std::path::absolute(path).map_err(io_err(path))?;
    Url::from_directory_path(&absolute)
        .map_err(|()| Error::Invalid(format!("không đổi được {} thành URL", absolute.display())))
}

#[cfg(test)]
mod tests {
    use super::TargetMeta;

    /// Metadata của bản 1.0 (chưa có thông tin hiển thị) vẫn đọc được; trường mới đi và về
    /// nguyên vẹn, trường rỗng không được ghi ra.
    #[test]
    fn display_fields_are_optional() {
        let old: TargetMeta = serde_json::from_str(
            r#"{"app":"a","version":"1.0.0","platform":"windows-x64","title":"A","entry":"a.exe"}"#,
        )
        .unwrap();
        assert_eq!(old.summary, None);
        assert!(!old.terminal);
        let json = serde_json::to_string(&old).unwrap();
        assert!(
            !json.contains("summary") && !json.contains("terminal") && !json.contains("features"),
            "{json}"
        );

        let new = TargetMeta {
            summary: Some("Soạn thảo văn bản".into()),
            category: Some("tools".into()),
            developer: Some("Ilaï Deutel".into()),
            source: Some("https://github.com/ilai-deutel/kibi".into()),
            terminal: true,
            description: Some("Trình soạn thảo".into()),
            features: vec!["Tô màu cú pháp".into(), "Tìm kiếm".into()],
            website: Some("https://example.org/kibi".into()),
            release_notes: Some("Sửa lỗi".into()),
            license: Some("MIT OR Apache-2.0".into()),
            released_at: Some("2026-11-13T10:00:00Z".into()),
            ..old
        };
        let back: TargetMeta = serde_json::from_str(&serde_json::to_string(&new).unwrap()).unwrap();
        assert_eq!(back, new);
    }
}
