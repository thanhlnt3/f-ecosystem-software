//! Phía client (launcher): đồng bộ metadata, xác minh chữ ký, hạn, phiên bản, và tải target đã
//! xác minh hash. Mọi kiểm tra do `tough` thực hiện; client lưu metadata đã tin trong `datastore`
//! để chặn rollback giữa các lần đồng bộ.

use crate::error::{io_err, Error, Result};
use crate::repo::{
    read_revocations, target_meta, Repo, Revocation, TargetMeta, REVOCATIONS_TARGET,
};
use jiff::Timestamp;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tough::{ExpirationEnforcement, IntoVec, Repository, RepositoryLoader, TargetName};
use url::Url;

/// Chu kỳ đồng bộ của launcher. Cùng với thời hạn 1 giờ của timestamp, một bản thu hồi tới
/// client trong vòng 1 giờ.
pub const SYNC_INTERVAL: Duration = Duration::from_secs(60 * 60);

/// Một target đang phát hành, đã được xác minh chữ ký.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TargetInfo {
    pub name: String,
    pub length: u64,
    pub sha256: String,
    pub meta: Option<TargetMeta>,
}

/// Kết quả một lần đồng bộ thành công.
#[derive(Clone, Debug)]
pub struct SyncState {
    pub root_version: u64,
    pub targets_version: u64,
    pub snapshot_version: u64,
    pub timestamp_version: u64,
    /// Thời điểm hết hạn sớm nhất trong các vai trò (thường là timestamp).
    pub expires: Timestamp,
    pub targets: BTreeMap<String, TargetInfo>,
    pub revocations: Vec<Revocation>,
}

impl SyncState {
    pub fn revocation(&self, target: &str) -> Option<&Revocation> {
        self.revocations.iter().rev().find(|r| r.target == target)
    }
}

/// Client TUF. `trusted_root` là root.json đi kèm bộ cài launcher (thường là `1.root.json`).
#[derive(Clone, Debug)]
pub struct Client {
    trusted_root: Vec<u8>,
    metadata_url: Url,
    targets_url: Url,
    datastore: PathBuf,
}

impl Client {
    pub fn new(
        trusted_root: Vec<u8>,
        metadata_url: Url,
        targets_url: Url,
        datastore: impl Into<PathBuf>,
    ) -> Self {
        Self {
            trusted_root,
            metadata_url,
            targets_url,
            datastore: datastore.into(),
        }
    }

    /// Client đọc thẳng một kho trên đĩa (test, công cụ dòng lệnh).
    pub async fn for_local_repo(
        repo: &Repo,
        trusted_root: &Path,
        datastore: impl Into<PathBuf>,
    ) -> Result<Self> {
        let root = tokio::fs::read(trusted_root)
            .await
            .map_err(io_err(trusted_root))?;
        Ok(Self::new(
            root,
            repo.metadata_url()?,
            repo.targets_url()?,
            datastore,
        ))
    }

    async fn load(&self) -> Result<Repository> {
        tokio::fs::create_dir_all(&self.datastore)
            .await
            .map_err(io_err(&self.datastore))?;
        Ok(RepositoryLoader::new(
            &self.trusted_root,
            self.metadata_url.clone(),
            self.targets_url.clone(),
        )
        .datastore(&self.datastore)
        .expiration_enforcement(ExpirationEnforcement::Safe)
        .load()
        .await?)
    }

    /// Đồng bộ: tải và xác minh metadata mới nhất. Lỗi nếu metadata bị sửa, cũ hơn bản đã tin
    /// (rollback), hết hạn (freeze) hoặc chữ ký không đủ ngưỡng.
    pub async fn sync(&self) -> Result<SyncState> {
        let repo = self.load().await?;
        let revocations = read_revocations(&repo).await?;
        let targets = repo
            .targets()
            .signed
            .targets
            .iter()
            .filter(|(name, _)| name.raw() != REVOCATIONS_TARGET)
            .map(|(name, target)| {
                let info = TargetInfo {
                    name: name.raw().to_owned(),
                    length: target.length,
                    sha256: hex::encode(&target.hashes.sha256),
                    meta: target_meta(target),
                };
                (info.name.clone(), info)
            })
            .collect();
        let expires = [
            repo.root().signed.expires,
            repo.targets().signed.expires,
            repo.snapshot().signed.expires,
            repo.timestamp().signed.expires,
        ]
        .into_iter()
        .min()
        .unwrap_or(repo.timestamp().signed.expires);
        Ok(SyncState {
            root_version: repo.root().signed.version.get(),
            targets_version: repo.targets().signed.version.get(),
            snapshot_version: repo.snapshot().signed.version.get(),
            timestamp_version: repo.timestamp().signed.version.get(),
            expires,
            targets,
            revocations,
        })
    }

    /// Tải một target và trả nội dung chỉ khi khớp độ dài và SHA-256 trong metadata đã ký.
    pub async fn download(&self, name: &str) -> Result<Vec<u8>> {
        let repo = self.load().await?;
        let target_name = TargetName::new(name)?;
        let stream = repo
            .read_target(&target_name)
            .await?
            .filter(|_| name != REVOCATIONS_TARGET)
            .ok_or_else(|| {
                Error::Invalid(format!(
                    "target {name} không có trong metadata đã ký (chưa phát hành hoặc đã bị thu hồi)"
                ))
            })?;
        Ok(stream.into_vec().await?)
    }
}
