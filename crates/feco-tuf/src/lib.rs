//! Chuỗi tin cậy cho gói cài đặt F-Eco theo mô hình The Update Framework (TUF).
//!
//! Toàn bộ phần ký và xác minh dùng crate `tough`; crate này chỉ ghép các bước lại theo quy trình
//! của F-Eco (design D3):
//!
//! - Bốn vai trò `root`, `targets`, `snapshot`, `timestamp`. Khóa `root` để ngoại tuyến, ngưỡng
//!   2/3. Khóa `targets` nằm trong KMS (bản dev dùng file); khóa `snapshot` và `timestamp` là khóa
//!   trực tuyến của máy chủ phân phối.
//! - Mỗi phiên bản app là một target (SHA-256, độ dài, app, phiên bản, nền tảng), được ký sau khi
//!   qua kiểm duyệt ([`Repo::publish`]).
//! - Thu hồi bằng metadata mới: gỡ target và ghi lý do vào target `feco-revocations.json`
//!   ([`Repo::revoke`]).
//! - `timestamp` hết hạn sau 1 giờ và được ký lại định kỳ ([`Repo::refresh`]), nên client đồng bộ
//!   mỗi giờ ([`client::SYNC_INTERVAL`]) thấy bản thu hồi trong vòng 1 giờ, kể cả khi kẻ tấn công
//!   "đóng băng" metadata cũ.
//! - Xoay khóa bằng root mới được ký bởi đủ ngưỡng của cả root cũ lẫn root mới
//!   ([`Repo::draft_root_update`], [`Repo::rotate`]).
//!
//! Bố cục kho trên đĩa (consistent snapshot):
//!
//! ```text
//! <kho>/metadata/N.root.json      mọi phiên bản root; client đi theo chuỗi 1 → 2 → ...
//! <kho>/metadata/N.targets.json
//! <kho>/metadata/N.snapshot.json
//! <kho>/metadata/timestamp.json
//! <kho>/targets/<sha256>.<tên>
//! ```

pub mod client;
mod error;
pub mod keys;
pub mod repo;

pub use client::{Client, SyncState, TargetInfo};
pub use error::{Error, Result};
pub use repo::{
    Expiry, InitKeys, Repo, Revocation, RootChange, RootDraft, TargetMeta, REVOCATIONS_TARGET,
};
pub use tough::schema::key::Key;
pub use tough::schema::RoleType;
