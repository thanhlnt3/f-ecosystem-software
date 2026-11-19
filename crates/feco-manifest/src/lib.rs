//! `f-app.toml`: manifest của một phiên bản app con (spec developer-portal, "Manifest app").
//!
//! [`parse`] đọc và kiểm tra manifest, trả về **mọi** lỗi tìm thấy, mỗi lỗi chỉ rõ trường sai
//! (ví dụ `scopes.email.purpose là bắt buộc`). Cổng nhà phát triển, CLI `f-dev` và khâu kiểm
//! duyệt dùng chung hàm này.
//!
//! ```toml
//! id = "soundlab"
//! name = "SoundLab"
//! version = "1.3.0"
//! license = "MIT"
//! privacy_policy = "https://soundlab.example/privacy"
//! webhook = "https://soundlab.example/feco/webhook"   # không bắt buộc
//! summary = "Thu âm và phối nhạc"                      # không bắt buộc
//! source = "https://github.com/soundlab/soundlab"     # mã nguồn, không bắt buộc
//! terminal = false      # true: app chạy trong cửa sổ terminal, không bắt buộc
//! website = "https://soundlab.example"                # không bắt buộc
//! features = ["Thu nhiều kênh", "Phối nhạc"]           # tối đa 12 dòng, không bắt buộc
//! release_notes = "Thêm hiệu ứng vang"                 # thay đổi của phiên bản, không bắt buộc
//!
//! [[platforms]]
//! os = "windows"        # windows | macos
//! arch = "x64"          # x64 | arm64
//! entry = "SoundLab.exe"
//!
//! [scopes.profile]
//! required = true
//! purpose = "Hiển thị trên bản nhạc bạn chia sẻ"
//! purpose_en = "Shown on tracks you share"            # không bắt buộc
//!
//! [permissions]
//! network = true
//! microphone = true
//! user_folders = ["music"]
//!
//! # Mã nguồn để quét khi duyệt (không bắt buộc; thay đổi chat-and-source-scan): repo GitHub
//! # công khai và commit đủ 40 ký tự hex. Không dùng cùng dòng `source = "..."` ở trên (TOML
//! # không cho một khóa vừa là chuỗi vừa là bảng).
//! [source]
//! repository = "https://github.com/soundlab/soundlab"
//! commit = "0a3b8b708cf435f730a77453b8203fbff280d198"
//! ```

use feco_core::Scope;
use serde::{Deserialize, Serialize};
use toml::{Table, Value};

pub const OS: [&str; 2] = ["windows", "macos"];
pub const ARCH: [&str; 2] = ["x64", "arm64"];
pub const USER_FOLDERS: [&str; 6] = [
    "documents",
    "downloads",
    "pictures",
    "music",
    "videos",
    "desktop",
];
const TOP_LEVEL: [&str; 17] = [
    "id",
    "name",
    "version",
    "license",
    "privacy_policy",
    "webhook",
    "summary",
    "description",
    "category",
    "platforms",
    "scopes",
    "permissions",
    "source",
    "terminal",
    "website",
    "features",
    "release_notes",
];
pub const MAX_FEATURES: usize = 12;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Platform {
    pub os: String,
    pub arch: String,
    /// Tệp chạy chính, tương đối với gốc của gói.
    pub entry: String,
}

impl Platform {
    /// Khóa nền tảng dùng trong danh mục và kho phân phối, ví dụ `windows-x64`.
    pub fn key(&self) -> String {
        format!("{}-{}", self.os, self.arch)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScopeDecl {
    pub scope: Scope,
    pub required: bool,
    pub purpose: String,
    pub purpose_en: Option<String>,
}

/// Quyền hệ thống app cần dùng (hiện cho người dùng và người kiểm duyệt).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Permissions {
    pub network: bool,
    pub camera: bool,
    pub microphone: bool,
    pub user_folders: Vec<String>,
}

/// Mã nguồn trên GitHub tại một commit (`[source]` trong `f-app.toml`), được quét khi duyệt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceGit {
    /// `https://github.com/<owner>/<repo>`
    pub repository: String,
    /// Commit đủ 40 ký tự hex (chữ thường).
    pub commit: String,
}

impl SourceGit {
    /// (chủ repo, tên repo).
    pub fn owner_repo(&self) -> Option<(String, String)> {
        github_repo(&self.repository)
    }
}

/// Tên chủ repo hoặc repo GitHub hợp lệ.
fn github_name(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 100
        && s != "."
        && s != ".."
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
}

/// `https://github.com/<owner>/<repo>` (có thể kết thúc bằng `/` hoặc `.git`) → (owner, repo).
pub fn github_repo(url: &str) -> Option<(String, String)> {
    let rest = url.trim().strip_prefix("https://github.com/")?;
    let rest = rest.trim_end_matches('/');
    let rest = rest.strip_suffix(".git").unwrap_or(rest);
    let (owner, repo) = rest.split_once('/')?;
    (github_name(owner) && github_name(repo)).then(|| (owner.to_owned(), repo.to_owned()))
}

/// Commit đủ 40 ký tự hex.
pub fn valid_commit(commit: &str) -> bool {
    commit.len() == 40 && commit.chars().all(|c| c.is_ascii_hexdigit())
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Manifest {
    pub id: String,
    pub name: String,
    /// SemVer đã kiểm tra; xem [`Manifest::semver`].
    pub version: String,
    pub license: String,
    pub privacy_policy: String,
    pub webhook: Option<String>,
    pub summary: String,
    pub description: String,
    pub category: String,
    pub platforms: Vec<Platform>,
    pub scopes: Vec<ScopeDecl>,
    pub permissions: Permissions,
    /// Địa chỉ mã nguồn (HTTPS). Có trường này thì gói phải kèm giấy phép gốc.
    #[serde(default)]
    pub source: Option<String>,
    /// Repo GitHub và commit của mã nguồn (`[source]`), được quét khi duyệt.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_git: Option<SourceGit>,
    /// App chạy trong cửa sổ terminal.
    #[serde(default)]
    pub terminal: bool,
    /// Website của app (HTTPS).
    #[serde(default)]
    pub website: Option<String>,
    /// Các tính năng chính (tối đa [`MAX_FEATURES`] dòng, mỗi dòng tối đa 120 ký tự).
    #[serde(default)]
    pub features: Vec<String>,
    /// Các thay đổi của phiên bản (tối đa 2.000 ký tự).
    #[serde(default)]
    pub release_notes: Option<String>,
}

impl Manifest {
    pub fn semver(&self) -> semver::Version {
        semver::Version::parse(&self.version).expect("phiên bản đã được kiểm tra khi đọc manifest")
    }
}

/// Mã app: chữ thường, số, `.`, `-`, `_`; bắt đầu bằng chữ hoặc số; tối đa 63 ký tự.
pub fn valid_app_id(id: &str) -> bool {
    let mut chars = id.chars();
    matches!(chars.next(), Some(c) if c.is_ascii_lowercase() || c.is_ascii_digit())
        && id.len() <= 63
        && id
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '.' | '-' | '_'))
}

fn https(url: &str) -> bool {
    url.strip_prefix("https://")
        .is_some_and(|rest| !rest.is_empty() && !rest.starts_with('/') && !rest.contains(' '))
}

/// Đường dẫn file chạy (`entry`) an toàn: tương đối trong gói, ngăn cách bằng `/`, chỉ gồm
/// `A-Z a-z 0-9 . _ - /`; mỗi đoạn không rỗng, không bắt đầu hay kết thúc bằng `.`. Ký tự như
/// `& ^ % ( )` hay dấu cách bị từ chối vì launcher mở app terminal qua `cmd.exe` trên Windows.
pub fn safe_entry(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 255
        && path
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-' | '/'))
        && path
            .split('/')
            .all(|p| !p.is_empty() && !p.starts_with('.') && !p.ends_with('.'))
}

/// Khóa so trùng của một mục trong gói zip: tách đoạn theo `/` và `\`, bỏ đoạn rỗng và `.`, bỏ
/// dấu chấm và dấu cách ở cuối đoạn (Windows bỏ chúng), chữ thường. Hai mục cùng khóa sẽ ghi đè
/// nhau khi giải nén trên NTFS hoặc APFS (không phân biệt hoa thường), ví dụ `SoundLab.exe`,
/// `soundlab.exe` và `./SoundLab.exe`; máy chủ và launcher đều từ chối gói như vậy.
pub fn package_path_key(name: &str) -> String {
    name.split(['/', '\\'])
        .filter(|s| !s.is_empty() && *s != ".")
        .map(|s| s.trim_end_matches(['.', ' ']).to_lowercase())
        .collect::<Vec<_>>()
        .join("/")
}

/// Hai tên đầu tiên trong `names` có cùng [`package_path_key`].
pub fn duplicate_package_path<'a>(
    names: impl IntoIterator<Item = &'a str>,
) -> Option<(&'a str, &'a str)> {
    let mut seen = std::collections::HashMap::new();
    for name in names {
        if let Some(first) = seen.insert(package_path_key(name), name) {
            return Some((first, name));
        }
    }
    None
}

struct Checker {
    errors: Vec<String>,
}

impl Checker {
    fn required(&mut self, path: &str) {
        self.errors.push(format!("{path} là bắt buộc"));
    }

    fn invalid(&mut self, path: &str, why: &str) {
        self.errors.push(format!("{path} không hợp lệ: {why}"));
    }

    /// Chuỗi bắt buộc, không rỗng.
    fn string(&mut self, table: &Table, key: &str, path: &str) -> Option<String> {
        match table.get(key) {
            None => {
                self.required(path);
                None
            }
            Some(Value::String(s)) if !s.trim().is_empty() => Some(s.trim().to_owned()),
            Some(Value::String(_)) => {
                self.required(path);
                None
            }
            Some(_) => {
                self.invalid(path, "phải là chuỗi");
                None
            }
        }
    }

    fn optional_string(&mut self, table: &Table, key: &str, path: &str) -> Option<String> {
        match table.get(key) {
            None => None,
            Some(Value::String(s)) => Some(s.trim().to_owned()).filter(|s| !s.is_empty()),
            Some(_) => {
                self.invalid(path, "phải là chuỗi");
                None
            }
        }
    }

    fn boolean(&mut self, table: &Table, key: &str, path: &str) -> bool {
        match table.get(key) {
            None => false,
            Some(Value::Boolean(b)) => *b,
            Some(_) => {
                self.invalid(path, "phải là true hoặc false");
                false
            }
        }
    }
}

/// Đọc và kiểm tra `f-app.toml`. Lỗi là danh sách thông báo, mỗi thông báo chỉ rõ một trường.
pub fn parse(text: &str) -> Result<Manifest, Vec<String>> {
    let table: Table = toml::from_str(text)
        .map_err(|e| vec![format!("f-app.toml sai cú pháp: {}", e.to_string().trim())])?;
    let mut c = Checker { errors: Vec::new() };

    for key in table.keys() {
        if !TOP_LEVEL.contains(&key.as_str()) {
            c.errors
                .push(format!("{key} không phải trường của f-app.toml"));
        }
    }

    let id = c.string(&table, "id", "id");
    if let Some(id) = &id {
        if !valid_app_id(id) {
            c.invalid(
                "id",
                "chỉ dùng chữ thường, số, '.', '-', '_', bắt đầu bằng chữ hoặc số, tối đa 63 ký tự",
            );
        }
    }
    let name = c.string(&table, "name", "name");
    if name.as_ref().is_some_and(|n| n.chars().count() > 80) {
        c.invalid("name", "tối đa 80 ký tự");
    }
    let version = c.string(&table, "version", "version");
    if let Some(v) = &version {
        if semver::Version::parse(v).is_err() {
            c.invalid("version", "phải theo SemVer, ví dụ 1.2.0");
        }
    }
    let license = c.string(&table, "license", "license");
    let privacy_policy = c.string(&table, "privacy_policy", "privacy_policy");
    if privacy_policy.as_ref().is_some_and(|u| !https(u)) {
        c.invalid("privacy_policy", "phải là địa chỉ https://");
    }
    let webhook = c.optional_string(&table, "webhook", "webhook");
    if webhook.as_ref().is_some_and(|u| !https(u)) {
        c.invalid("webhook", "phải là địa chỉ https://");
    }
    let summary = c.optional_string(&table, "summary", "summary");
    if summary.as_ref().is_some_and(|s| s.chars().count() > 200) {
        c.invalid("summary", "tối đa 200 ký tự");
    }
    let description = c.optional_string(&table, "description", "description");
    let category = c.optional_string(&table, "category", "category");
    let (source, source_git) = source(&mut c, &table);
    let terminal = c.boolean(&table, "terminal", "terminal");
    let website = c.optional_string(&table, "website", "website");
    if website.as_ref().is_some_and(|u| !https(u)) {
        c.invalid("website", "phải là địa chỉ https://");
    }
    let features = features(&mut c, &table);
    let release_notes = c.optional_string(&table, "release_notes", "release_notes");
    if release_notes
        .as_ref()
        .is_some_and(|s| s.chars().count() > 2000)
    {
        c.invalid("release_notes", "tối đa 2.000 ký tự");
    }

    let platforms = platforms(&mut c, &table);
    let scopes = scopes(&mut c, &table);
    let permissions = permissions(&mut c, &table);

    if !c.errors.is_empty() {
        return Err(c.errors);
    }
    // Không còn lỗi nên mọi trường bắt buộc đều có giá trị.
    Ok(Manifest {
        id: id.unwrap_or_default(),
        name: name.unwrap_or_default(),
        version: version.unwrap_or_default(),
        license: license.unwrap_or_default(),
        privacy_policy: privacy_policy.unwrap_or_default(),
        webhook,
        summary: summary.unwrap_or_default(),
        description: description.unwrap_or_default(),
        category: category.unwrap_or_else(|| "other".into()),
        platforms,
        scopes,
        permissions,
        source,
        source_git,
        terminal,
        website,
        features,
        release_notes,
    })
}

/// `source`: chuỗi địa chỉ HTTPS, hoặc bảng `[source]` với `repository` (repo GitHub) và `commit`.
fn source(c: &mut Checker, table: &Table) -> (Option<String>, Option<SourceGit>) {
    let Some(Value::Table(t)) = table.get("source") else {
        let url = c.optional_string(table, "source", "source");
        if url.as_ref().is_some_and(|u| !https(u)) {
            c.invalid("source", "phải là địa chỉ https://");
        }
        return (url, None);
    };
    for key in t.keys() {
        if key != "repository" && key != "commit" {
            c.errors
                .push(format!("source.{key} không phải trường của [source]"));
        }
    }
    let repository = c.string(t, "repository", "source.repository");
    let commit = c.string(t, "commit", "source.commit");
    let repo = repository.as_deref().and_then(github_repo);
    if repository.is_some() && repo.is_none() {
        c.invalid(
            "source.repository",
            "phải là repo GitHub công khai dạng https://github.com/<chủ>/<repo>",
        );
    }
    if commit.as_ref().is_some_and(|x| !valid_commit(x)) {
        c.invalid("source.commit", "phải là mã commit đủ 40 ký tự hex");
    }
    match (repo, commit) {
        // `[source]` chỉ khai mã nguồn để quét; không đặt `source` (địa chỉ mã nguồn bên ngoài,
        // vốn bắt gói phải kèm giấy phép gốc).
        (Some((owner, name)), Some(commit)) if valid_commit(&commit) => (
            None,
            Some(SourceGit {
                repository: format!("https://github.com/{owner}/{name}"),
                commit: commit.to_ascii_lowercase(),
            }),
        ),
        _ => (None, None),
    }
}

/// `features`: danh sách chuỗi không rỗng, tối đa [`MAX_FEATURES`] dòng, mỗi dòng ≤ 120 ký tự.
fn features(c: &mut Checker, table: &Table) -> Vec<String> {
    let items = match table.get("features") {
        None => return Vec::new(),
        Some(Value::Array(items)) => items,
        Some(_) => {
            c.invalid("features", "phải là danh sách chuỗi");
            return Vec::new();
        }
    };
    if items.len() > MAX_FEATURES {
        c.invalid("features", &format!("tối đa {MAX_FEATURES} dòng"));
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, item) in items.iter().enumerate() {
        match item {
            Value::String(s) if !s.trim().is_empty() && s.trim().chars().count() <= 120 => {
                out.push(s.trim().to_owned())
            }
            Value::String(s) if s.trim().is_empty() => {
                c.invalid(&format!("features[{i}]"), "không được để trống")
            }
            Value::String(_) => c.invalid(&format!("features[{i}]"), "tối đa 120 ký tự"),
            _ => c.invalid(&format!("features[{i}]"), "phải là chuỗi"),
        }
    }
    out
}

fn platforms(c: &mut Checker, table: &Table) -> Vec<Platform> {
    let items = match table.get("platforms") {
        None => {
            c.required("platforms");
            return Vec::new();
        }
        Some(Value::Array(items)) if !items.is_empty() => items,
        Some(Value::Array(_)) => {
            c.required("platforms");
            return Vec::new();
        }
        Some(_) => {
            c.invalid("platforms", "phải là danh sách [[platforms]]");
            return Vec::new();
        }
    };
    let mut out: Vec<Platform> = Vec::new();
    for (i, item) in items.iter().enumerate() {
        let path = format!("platforms[{i}]");
        let Value::Table(t) = item else {
            c.invalid(&path, "phải là bảng có os, arch, entry");
            continue;
        };
        let os = c.string(t, "os", &format!("{path}.os"));
        if os.as_ref().is_some_and(|o| !OS.contains(&o.as_str())) {
            c.invalid(&format!("{path}.os"), "chỉ nhận windows hoặc macos");
        }
        let arch = c.string(t, "arch", &format!("{path}.arch"));
        if arch.as_ref().is_some_and(|a| !ARCH.contains(&a.as_str())) {
            c.invalid(&format!("{path}.arch"), "chỉ nhận x64 hoặc arm64");
        }
        let entry = c.string(t, "entry", &format!("{path}.entry"));
        if entry.as_ref().is_some_and(|e| !safe_entry(e)) {
            c.invalid(
                &format!("{path}.entry"),
                "phải là đường dẫn tương đối trong gói, dùng '/', chỉ gồm chữ, số, '.', '_', '-', không có '..'",
            );
        }
        if let (Some(os), Some(arch), Some(entry)) = (os, arch, entry) {
            let p = Platform { os, arch, entry };
            if out.iter().any(|x| x.key() == p.key()) {
                c.invalid(&path, &format!("{} đã được khai báo ở trên", p.key()));
            } else {
                out.push(p);
            }
        }
    }
    out
}

fn scopes(c: &mut Checker, table: &Table) -> Vec<ScopeDecl> {
    let t = match table.get("scopes") {
        None => return Vec::new(),
        Some(Value::Table(t)) => t,
        Some(_) => {
            c.invalid("scopes", "phải là bảng [scopes.<tên>]");
            return Vec::new();
        }
    };
    let mut out = Vec::new();
    for (name, value) in t {
        let path = format!("scopes.{name}");
        let Ok(scope) = name.parse::<Scope>() else {
            let known: Vec<&str> = Scope::ALL.iter().map(|s| s.as_str()).collect();
            c.invalid(
                &path,
                &format!("không phải scope dữ liệu ({})", known.join(", ")),
            );
            continue;
        };
        let Value::Table(decl) = value else {
            c.invalid(&path, "phải là bảng có purpose");
            continue;
        };
        let purpose = c.string(decl, "purpose", &format!("{path}.purpose"));
        let purpose_en = c.optional_string(decl, "purpose_en", &format!("{path}.purpose_en"));
        let required = c.boolean(decl, "required", &format!("{path}.required"));
        if let Some(purpose) = purpose {
            out.push(ScopeDecl {
                scope,
                required,
                purpose,
                purpose_en,
            });
        }
    }
    out
}

fn permissions(c: &mut Checker, table: &Table) -> Permissions {
    let t = match table.get("permissions") {
        None => return Permissions::default(),
        Some(Value::Table(t)) => t,
        Some(_) => {
            c.invalid("permissions", "phải là bảng [permissions]");
            return Permissions::default();
        }
    };
    for key in t.keys() {
        if !["network", "camera", "microphone", "user_folders"].contains(&key.as_str()) {
            c.invalid(
                &format!("permissions.{key}"),
                "chỉ có network, camera, microphone, user_folders",
            );
        }
    }
    let mut user_folders = Vec::new();
    match t.get("user_folders") {
        None => {}
        Some(Value::Array(items)) => {
            for (i, item) in items.iter().enumerate() {
                match item.as_str() {
                    Some(f) if USER_FOLDERS.contains(&f) => user_folders.push(f.to_owned()),
                    _ => c.invalid(
                        &format!("permissions.user_folders[{i}]"),
                        &format!("chỉ nhận {}", USER_FOLDERS.join(", ")),
                    ),
                }
            }
        }
        Some(_) => c.invalid("permissions.user_folders", "phải là danh sách"),
    }
    Permissions {
        network: c.boolean(t, "network", "permissions.network"),
        camera: c.boolean(t, "camera", "permissions.camera"),
        microphone: c.boolean(t, "microphone", "permissions.microphone"),
        user_folders,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const FULL: &str = r#"
id = "soundlab"
name = "SoundLab"
version = "1.3.0"
license = "MIT"
privacy_policy = "https://soundlab.example/privacy"
webhook = "https://soundlab.example/feco/webhook"
summary = "Thu âm và phối nhạc"
category = "Âm nhạc"

[[platforms]]
os = "windows"
arch = "x64"
entry = "SoundLab.exe"

[[platforms]]
os = "macos"
arch = "arm64"
entry = "SoundLab.app/Contents/MacOS/SoundLab"

[scopes.profile]
required = true
purpose = "Hiển thị trên bản nhạc bạn chia sẻ"
purpose_en = "Shown on tracks you share"

[scopes.email]
purpose = "Gửi bản tin"

[permissions]
network = true
microphone = true
user_folders = ["music"]
"#;

    fn errors(text: &str) -> Vec<String> {
        parse(text).expect_err("manifest phải bị từ chối")
    }

    /// Bỏ một dòng khỏi manifest đầy đủ.
    fn without(line_prefix: &str) -> String {
        let out: Vec<&str> = FULL
            .lines()
            .filter(|l| !l.starts_with(line_prefix))
            .collect();
        assert_ne!(
            out.len(),
            FULL.lines().count(),
            "không có dòng {line_prefix}"
        );
        out.join("\n")
    }

    #[test]
    fn full_manifest_is_accepted() {
        let m = parse(FULL).expect("hợp lệ");
        assert_eq!((m.id.as_str(), m.semver().minor), ("soundlab", 3));
        assert_eq!(m.platforms[1].key(), "macos-arm64");
        assert_eq!(m.scopes.len(), 2);
        let email = m.scopes.iter().find(|s| s.scope == Scope::Email).unwrap();
        assert!(!email.required && email.purpose_en.is_none());
        assert!(m.permissions.network && m.permissions.microphone && !m.permissions.camera);
        assert_eq!(m.permissions.user_folders, ["music"]);
    }

    #[test]
    fn each_required_field_is_reported_by_name() {
        for field in ["id", "name", "version", "license", "privacy_policy"] {
            assert_eq!(
                errors(&without(&format!("{field} ="))),
                [format!("{field} là bắt buộc")],
                "{field}"
            );
        }
        // Trường rỗng cũng là thiếu.
        assert_eq!(
            errors(&FULL.replace("name = \"SoundLab\"", "name = \"  \"")),
            ["name là bắt buộc"]
        );
    }

    /// Scenario "Thiếu mục đích của scope".
    #[test]
    fn scope_without_purpose_is_rejected() {
        assert_eq!(
            errors(&without("purpose = \"Gửi bản tin\"")),
            ["scopes.email.purpose là bắt buộc"]
        );
        let e = errors(&FULL.replace("[scopes.email]", "[scopes.contacts]"));
        assert!(
            e[0].starts_with("scopes.contacts không hợp lệ: không phải scope dữ liệu"),
            "{e:?}"
        );
    }

    #[test]
    fn platforms_need_os_arch_and_entry() {
        let no_platforms: String = FULL.split("[[platforms]]").next().unwrap().to_owned()
            + FULL.split("[scopes.profile]").nth(1).map(|_| "").unwrap();
        assert_eq!(errors(&no_platforms), ["platforms là bắt buộc"]);
        assert_eq!(
            errors(&without("entry = \"SoundLab.exe\"")),
            ["platforms[0].entry là bắt buộc"]
        );
        assert_eq!(
            errors(&without("arch = \"x64\"")),
            ["platforms[0].arch là bắt buộc"]
        );
        assert_eq!(
            errors(&without("os = \"macos\"")),
            ["platforms[1].os là bắt buộc"]
        );
        let e = errors(&FULL.replace("os = \"macos\"", "os = \"linux\""));
        assert_eq!(
            e,
            ["platforms[1].os không hợp lệ: chỉ nhận windows hoặc macos"]
        );
        let e = errors(&FULL.replace("entry = \"SoundLab.exe\"", "entry = \"../evil.exe\""));
        assert!(e[0].starts_with("platforms[0].entry không hợp lệ"), "{e:?}");
        // Ký tự đặc biệt của cmd.exe, dấu cách, đường dẫn tuyệt đối, đoạn bắt đầu/kết thúc bằng '.'.
        for bad in [
            "a.exe&calc.exe",
            "a^b.exe",
            "%COMSPEC%",
            "x(1).exe",
            "My App.exe",
            "/abs.exe",
            "C:/a.exe",
            "a\\\\b.exe",
            "bin/.hidden",
            "app.exe.",
            "a//b.exe",
            "./a.exe",
        ] {
            let e = errors(&FULL.replace("SoundLab.exe", bad));
            assert!(
                e[0].starts_with("platforms[0].entry không hợp lệ"),
                "{bad}: {e:?}"
            );
        }
        for good in [
            "bin/sound_lab-2.exe",
            "SoundLab.app/Contents/MacOS/SoundLab",
        ] {
            assert!(safe_entry(good), "{good}");
        }
        let e = errors(&FULL.replace(
            "os = \"macos\"\narch = \"arm64\"",
            "os = \"windows\"\narch = \"x64\"",
        ));
        assert!(e[0].contains("windows-x64 đã được khai báo"), "{e:?}");
    }

    #[test]
    fn values_are_checked_and_all_errors_are_listed() {
        let bad = FULL
            .replace("version = \"1.3.0\"", "version = \"1.3\"")
            .replace(
                "https://soundlab.example/privacy",
                "http://soundlab.example/privacy",
            )
            .replace("id = \"soundlab\"", "id = \"Sound Lab\"")
            .replace(
                "user_folders = [\"music\"]",
                "user_folders = [\"system32\"]",
            );
        let e = errors(&bad);
        assert_eq!(e.len(), 4, "{e:?}");
        assert!(e.iter().any(|m| m.starts_with("id không hợp lệ")));
        assert!(e
            .iter()
            .any(|m| m == "version không hợp lệ: phải theo SemVer, ví dụ 1.2.0"));
        assert!(e
            .iter()
            .any(|m| m == "privacy_policy không hợp lệ: phải là địa chỉ https://"));
        assert!(e
            .iter()
            .any(|m| m.starts_with("permissions.user_folders[0] không hợp lệ")));
    }

    #[test]
    fn syntax_errors_and_unknown_fields_are_explained() {
        let e = errors("id = \"a\"\nname = ");
        assert!(e[0].starts_with("f-app.toml sai cú pháp:"), "{e:?}");
        assert!(e[0].contains("line 2"), "{e:?}");
        let e = errors(&format!("nmae = \"x\"\n{FULL}"));
        assert_eq!(e, ["nmae không phải trường của f-app.toml"]);
    }

    #[test]
    fn app_ids() {
        for ok in ["soundlab", "so-tay.hoc_tap", "7zip"] {
            assert!(valid_app_id(ok), "{ok}");
        }
        for bad in ["", "SoundLab", "-x", "a b", "a/b", &"a".repeat(64)] {
            assert!(!valid_app_id(bad), "{bad}");
        }
    }

    /// Spec developer-portal (improve-launcher-experience), "Thông tin hiển thị trong metadata
    /// phát hành": `source` phải là HTTPS, `terminal` là true/false.
    #[test]
    fn source_and_terminal() {
        let m = parse(FULL).unwrap();
        assert_eq!(m.source, None);
        assert!(!m.terminal);

        let text = format!(
            "source = \"https://github.com/ilai-deutel/kibi\"
terminal = true
{FULL}"
        );
        let m = parse(&text).unwrap();
        assert_eq!(
            m.source.as_deref(),
            Some("https://github.com/ilai-deutel/kibi")
        );
        assert!(m.terminal);

        let text = format!(
            "source = \"http://github.com/a/b\"
terminal = \"yes\"
{FULL}"
        );
        let errors = parse(&text).unwrap_err();
        assert!(
            errors.contains(&"source không hợp lệ: phải là địa chỉ https://".to_string()),
            "{errors:?}"
        );
        assert!(
            errors
                .iter()
                .any(|e| e.starts_with("terminal không hợp lệ")),
            "{errors:?}"
        );
    }

    /// `[source]`: repo GitHub và commit để quét mã nguồn khi duyệt (thay đổi chat-and-source-scan).
    #[test]
    fn source_table() {
        let commit = "0A3B8B708CF435F730A77453B8203FBFF280D198";
        let text = format!(
            "{FULL}
[source]
repository = \"https://github.com/soundlab/soundlab.git\"
commit = \"{commit}\"
"
        );
        let m = parse(&text).unwrap();
        let git = m.source_git.clone().unwrap();
        assert_eq!(git.repository, "https://github.com/soundlab/soundlab");
        assert_eq!(git.commit, commit.to_ascii_lowercase());
        assert_eq!(
            git.owner_repo(),
            Some(("soundlab".into(), "soundlab".into()))
        );
        assert_eq!(
            m.source, None,
            "[source] không bật quy tắc giấy phép gốc của `source`"
        );
        // Lưu và đọc lại dạng JSON (CSDL) vẫn giữ nguyên; manifest cũ không có trường này.
        let back: Manifest = serde_json::from_value(serde_json::to_value(&m).unwrap()).unwrap();
        assert_eq!(back, m);
        assert_eq!(parse(FULL).unwrap().source_git, None);

        let bad = format!(
            "{FULL}
[source]
repository = \"https://gitlab.com/a/b\"
commit = \"main\"
branch = \"x\"
"
        );
        let e = errors(&bad);
        assert!(
            e.iter()
                .any(|m| m == "source.branch không phải trường của [source]"),
            "{e:?}"
        );
        assert!(
            e.iter()
                .any(|m| m.starts_with("source.repository không hợp lệ")),
            "{e:?}"
        );
        assert!(
            e.iter()
                .any(|m| m.starts_with("source.commit không hợp lệ")),
            "{e:?}"
        );
        let e = errors(&format!(
            "{FULL}
[source]
commit = \"{commit}\"
"
        ));
        assert_eq!(e, ["source.repository là bắt buộc"]);
        for bad in [
            "https://github.com/a",
            "https://github.com/a/b/c",
            "http://github.com/a/b",
            "https://github.com/../b",
        ] {
            assert_eq!(github_repo(bad), None, "{bad}");
        }
    }

    /// Scenario "Phát hành có ghi chú" (phần manifest) và "Tính năng quá dài".
    #[test]
    fn website_features_release_notes() {
        let text = format!(
            "website = \"https://soundlab.example\"\nfeatures = [\"Thu nhiều kênh\", \" Phối nhạc \"]\nrelease_notes = \"Thêm tìm kiếm\"\n{FULL}"
        );
        let m = parse(&text).unwrap();
        assert_eq!(m.website.as_deref(), Some("https://soundlab.example"));
        assert_eq!(m.features, ["Thu nhiều kênh", "Phối nhạc"]);
        assert_eq!(m.release_notes.as_deref(), Some("Thêm tìm kiếm"));

        let many: Vec<String> = (0..13).map(|i| format!("\"Tính năng {i}\"")).collect();
        let text = format!(
            "features = [{}]\nwebsite = \"ftp://x\"\n{FULL}",
            many.join(", ")
        );
        let errors = parse(&text).unwrap_err();
        assert!(
            errors.contains(&"features không hợp lệ: tối đa 12 dòng".to_string()),
            "{errors:?}"
        );
        assert!(
            errors.iter().any(|e| e.starts_with("website không hợp lệ")),
            "{errors:?}"
        );

        let long = "x".repeat(121);
        let text = format!("features = [\"\", \"{long}\", 3]\n{FULL}");
        let errors = parse(&text).unwrap_err();
        assert_eq!(errors.len(), 3, "{errors:?}");
    }

    /// Mục trong gói ghi đè nhau trên NTFS (hoa thường, `./`, `\`, dấu chấm cuối) bị phát hiện.
    #[test]
    fn colliding_package_paths() {
        assert_eq!(package_path_key("./Bin\\SoundLab.EXE."), "bin/soundlab.exe");
        for other in [
            "soundlab.exe",
            "./SoundLab.exe",
            "SOUNDLAB.EXE",
            "SoundLab.exe.",
            ".//SoundLab.exe",
        ] {
            assert_eq!(
                duplicate_package_path(["SoundLab.exe", "README.txt", other]),
                Some(("SoundLab.exe", other)),
                "{other}"
            );
        }
        assert_eq!(
            duplicate_package_path(["bin/", "bin/a.exe", "bin/b.exe", "Bin2/a.exe"]),
            None
        );
    }
}
