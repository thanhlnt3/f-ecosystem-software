//! A finished project goes to F-Eco (tasks.md 11.6, spec app-builder "Chạy thử và nộp app" and
//! "Chuyển thành đơn xây app theo yêu cầu"):
//!
//! - [`project_store_submit`]: a draft `app/f-app.toml` for the person to review, then the version
//!   submitted through the developer portal's API (the same calls as `f-dev publish`), where it
//!   goes through review ("Chờ duyệt"). A project that read its reference deeply is never submitted.
//! - [`project_order_draft`]: "Nhờ F-Eco xây hộ", a draft custom-build order in the project with its
//!   spec (and, when the person allows, its source) attached, waiting for them to send it.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use f_builder_engine::tool::{Ctx, Effects, Requires, ToolOutput, ToolSpec, Toolset, parse_params, schema_of};
use f_builder_engine::workspace::{Status, Workspace};
use f_builder_evidence::{ErrorCode, ToolError};
use schemars::JsonSchema;
use serde::Deserialize;
use serde_json::{Value, json};

/// The manifest's file in app/.
pub const MANIFEST_FILE: &str = "f-app.toml";
/// The first line of a manifest F-Builder drafted: it's submitted only once the person has read it
/// and taken this line out.
pub const DRAFT_MARK: &str = "# F-BUILDER-DRAFT";
/// The developer token's name in the keychain.
pub const TOKEN_KEY: &str = "feco:developer";
/// The order's file in its folder.
pub const ORDER_FILE: &str = "order.json";
pub const ORDER_VERSION: u32 = 1;
/// The portal's address when none is given (a developer's own F-Eco while it's being built).
const DEFAULT_SERVER: &str = "http://127.0.0.1:8080/";

pub fn specs() -> Vec<ToolSpec> {
    vec![
        ToolSpec {
            id: "project_store_submit",
            title: "Submit to F-Store",
            summary: "Submit a Ready project to F-Store through the F-Eco developer portal. The first call writes a draft app/f-app.toml (name, version, licence, privacy policy, platform, data it uses) for the person to review; once they've removed its first line (# F-BUILDER-DRAFT), the next call checks it, packs the built program and submits the version, which then waits for review. Needs a developer token (the token parameter saves it in the system keychain, or FDEV_TOKEN). A project that used Deep mode is never submitted: what it learned from the original is for private use.",
            beginner: "Sends a finished app to F-Store for review.",
            toolset: Toolset::Projects,
            effects: Effects::ACCESSES_NETWORK.union(Effects::WRITES_FILESYSTEM),
            requires: Requires::Nothing,
            input_schema: schema_of::<SubmitParams>,
            run: project_store_submit,
        },
        ToolSpec {
            id: "project_order_draft",
            title: "Ask F-Eco to build it",
            summary: "Make a draft custom-build order from a project (\"Nhờ F-Eco xây hộ\"): its request, its OpenSpec (if it has one) and, when include_source is set, its app's source, in orders/draft-<time>/ in the project. The draft waits for the person to send it.",
            beginner: "Gets an order ready for F-Eco to build the app for you.",
            toolset: Toolset::Projects,
            effects: Effects::WRITES_FILESYSTEM,
            requires: Requires::Nothing,
            input_schema: schema_of::<OrderParams>,
            run: project_order_draft,
        },
    ]
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SubmitParams {
    /// The project's folder.
    pub project: PathBuf,
    /// The F-Eco server (default FDEV_SERVER, else a local one at http://127.0.0.1:8080/).
    #[serde(default)]
    pub server: Option<String>,
    /// A developer token from the developer portal; saved in the system keychain for next time.
    #[serde(default)]
    pub token: Option<String>,
}

#[derive(Debug, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct OrderParams {
    /// The project's folder.
    pub project: PathBuf,
    /// Attach the app's source as it is now.
    #[serde(default)]
    pub include_source: bool,
}

fn vi(ctx: &Ctx<'_>) -> bool {
    ctx.session.settings.language.to_ascii_lowercase().starts_with("vi")
}

fn say(vi: bool, vi_text: &str, en_text: &str) -> String {
    if vi { vi_text.to_string() } else { en_text.to_string() }
}

fn project_store_submit(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: SubmitParams = parse_params(params)?;
    let vi = vi(ctx);
    let ws = Workspace::open(&p.project)?;
    // Spec scenario "Nộp kết quả từ chế độ đọc sâu".
    if ws.manifest.deep_used {
        return Err(ToolError::new(
            ErrorCode::ConsentRequired,
            say(
                vi,
                "Dự án này đã chạy ở chế độ đọc sâu, nên không nộp được lên F-Store: kết quả từ chế độ đọc sâu chỉ để dùng riêng.",
                "This project ran in Deep mode, so it can't be submitted to F-Store: what Deep mode makes is for private use.",
            ),
        )
        .with_remediation(say(
            vi,
            "Bắt đầu một dự án mới ở chế độ clean room nếu muốn nộp.",
            "Start a new project in clean room to make one you can submit.",
        )));
    }
    if ws.manifest.status != Status::Ready {
        return Err(ToolError::invalid(say(vi, "Chỉ nộp được dự án đã Sẵn sàng.", "Only a Ready project can be submitted.")));
    }
    let manifest_path = ws.app_dir().join(MANIFEST_FILE);
    let platform = platform_here().ok_or_else(|| ToolError::unsupported("F-Store takes apps for Windows and macOS."))?;
    let program = f_builder_engine::outputs::built_program(&ws.app_dir()).or_else(|| f_builder_engine::outputs::packaged_program(&ws.app_dir()));
    if !manifest_path.is_file() {
        let entry = program.as_ref().and_then(|p| p.file_name()).map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
        let draft = draft_manifest(&ws, &platform, &entry);
        f_builder_platform::fs::write_atomic(&manifest_path, draft.as_bytes())?;
        return Ok(ToolOutput::Value(json!({
            "submitted": false,
            "draft": manifest_path.display().to_string(),
            "note": say(vi,
                "Đã tạo bản nháp app/f-app.toml. Hãy xem lại tên, phiên bản, giấy phép và chính sách quyền riêng tư, xóa dòng đầu (# F-BUILDER-DRAFT), rồi nộp lại.",
                "A draft app/f-app.toml was written. Review its name, version, licence and privacy policy, remove its first line (# F-BUILDER-DRAFT), then submit again."),
        })));
    }
    let toml = std::fs::read_to_string(&manifest_path).map_err(|e| ToolError::io(manifest_path.display(), e))?;
    if toml.trim_start().starts_with(DRAFT_MARK) {
        return Err(ToolError::invalid(say(
            vi,
            "app/f-app.toml vẫn là bản nháp: xem lại rồi xóa dòng đầu (# F-BUILDER-DRAFT) trước khi nộp.",
            "app/f-app.toml is still a draft: review it and remove its first line (# F-BUILDER-DRAFT) before submitting.",
        )));
    }
    let manifest = feco_manifest::parse(&toml).map_err(|errors| ToolError::invalid(format!("app/f-app.toml:\n{}", errors.join("\n"))))?;
    let program = program.ok_or_else(|| {
        ToolError::new(ErrorCode::NotFound, say(vi, "Chưa có bản dựng của app để đóng gói.", "There's no build of the app to pack.")).with_remediation(say(
            vi,
            "Dựng app (ví dụ cargo build --release) rồi nộp lại.",
            "Build the app (e.g. cargo build --release), then submit again.",
        ))
    })?;
    let ours = manifest
        .platforms
        .iter()
        .find(|pl| pl.key() == platform.key())
        .ok_or_else(|| ToolError::invalid(format!("app/f-app.toml has no [[platforms]] entry for {} (this computer).", platform.key())))?;
    let packages_dir = ws.root.join("store");
    std::fs::create_dir_all(&packages_dir).map_err(|e| ToolError::io(packages_dir.display(), e))?;
    let zip_path = packages_dir.join(format!("{}.zip", ours.key()));
    pack(&program, &ours.entry, &zip_path)?;
    let token = token(&p.token)?;
    let server = server(p.server.as_deref())?;
    let (version_id, status) = submit(&server, &token, &manifest.id, &toml, &[(ours.key(), zip_path.clone())])?;
    Ok(ToolOutput::Value(json!({
        "submitted": true,
        "app": manifest.id,
        "version": manifest.version,
        "version_id": version_id,
        "status": status,
        "package": zip_path.display().to_string(),
        "note": if status == "pending_review" { say(vi, "Đã nộp: phiên bản đang Chờ duyệt.", "Submitted: the version is waiting for review.") } else { status.clone() },
    })))
}

/// This computer's platform as F-Store names it.
fn platform_here() -> Option<feco_manifest::Platform> {
    let os = if cfg!(windows) {
        "windows"
    } else if cfg!(target_os = "macos") {
        "macos"
    } else {
        return None;
    };
    let arch = if cfg!(target_arch = "x86_64") {
        "x64"
    } else if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        return None;
    };
    Some(feco_manifest::Platform { os: os.into(), arch: arch.into(), entry: String::new() })
}

/// An app id from a project's name: lower-case letters, digits and `-`, starting with a letter or
/// digit, at most 63 characters.
pub fn app_id(name: &str) -> String {
    let mut id = String::new();
    for c in name.to_lowercase().chars() {
        if c.is_ascii_alphanumeric() {
            id.push(c);
        } else if !id.ends_with('-') && !id.is_empty() {
            id.push('-');
        }
    }
    let id: String = id.trim_matches('-').chars().take(63).collect();
    let id = id.trim_end_matches('-').to_string();
    if id.is_empty() { "app".into() } else { id }
}

fn toml_str(s: &str) -> String {
    toml::Value::String(s.to_string()).to_string()
}

/// The draft manifest for `ws`, with what F-Builder knows filled in and the rest marked to check.
pub fn draft_manifest(ws: &Workspace, platform: &feco_manifest::Platform, entry: &str) -> String {
    let m = &ws.manifest;
    let summary: String = m.request.lines().next().unwrap_or_default().chars().take(120).collect();
    format!(
        "{DRAFT_MARK}: F-Builder drafted this file. Check every value, then remove this line and submit again.\n\
         id = {id}\n\
         name = {name}\n\
         version = \"0.1.0\"\n\
         # The licence you give the app (an SPDX id such as MIT, or Proprietary).\n\
         license = \"Proprietary\"\n\
         # Your app's privacy policy: replace with your own page.\n\
         privacy_policy = \"https://example.com/privacy\"\n\
         summary = {summary}\n\n\
         [[platforms]]\n\
         os = \"{os}\"\n\
         arch = \"{arch}\"\n\
         entry = {entry}\n\n\
         # What the app reads from the person's F-Eco account, and why.\n\
         [scopes.profile]\n\
         required = true\n\
         purpose = \"Hiển thị tên của bạn trong app\"\n\
         purpose_en = \"Show your name in the app\"\n\n\
         [permissions]\n\
         network = false\n",
        id = toml_str(&app_id(&m.name)),
        name = toml_str(&m.name),
        summary = toml_str(&summary),
        os = platform.os,
        arch = platform.arch,
        entry = toml_str(entry),
    )
}

/// A zip with the built program as `entry`.
fn pack(program: &Path, entry: &str, out: &Path) -> Result<(), ToolError> {
    let file = std::fs::File::create(out).map_err(|e| ToolError::io(out.display(), e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default().unix_permissions(0o755);
    zip.start_file(entry.replace('\\', "/"), options).map_err(|e| ToolError::internal(e.to_string()))?;
    let mut input = std::fs::File::open(program).map_err(|e| ToolError::io(program.display(), e))?;
    std::io::copy(&mut input, &mut zip).map_err(|e| ToolError::io(out.display(), e))?;
    zip.finish().map_err(|e| ToolError::internal(e.to_string()))?.flush().map_err(|e| ToolError::io(out.display(), e))
}

fn token(given: &Option<String>) -> Result<String, ToolError> {
    let store = crate::KeyStore::system();
    if let Some(t) = given.as_deref().map(str::trim).filter(|t| !t.is_empty()) {
        // Kept for next time when the keychain can; used either way.
        let _ = store.set(TOKEN_KEY, t);
        return Ok(t.to_string());
    }
    store.get(TOKEN_KEY).or_else(|| std::env::var("FDEV_TOKEN").ok().filter(|t| !t.trim().is_empty())).ok_or_else(|| {
        ToolError::new(ErrorCode::BadSettings, "No developer token.")
            .with_remediation("Create one in the F-Eco developer portal and pass it as token (it's saved in the system keychain), or set FDEV_TOKEN.")
    })
}

fn server(given: Option<&str>) -> Result<String, ToolError> {
    let s = given.map(str::to_string).or_else(|| std::env::var("FDEV_SERVER").ok()).unwrap_or_else(|| DEFAULT_SERVER.into());
    let s = format!("{}/", s.trim().trim_end_matches('/'));
    if s.starts_with("https://") || s.starts_with("http://127.0.0.1") || s.starts_with("http://localhost") {
        Ok(s)
    } else {
        Err(ToolError::invalid(format!("The F-Eco server must use HTTPS: {s}")))
    }
}

/// The portal's answer, or its error in the person's words.
fn answer(r: Result<ureq::http::Response<ureq::Body>, ureq::Error>, what: &str) -> Result<Value, ToolError> {
    let mut res = r.map_err(|e| ToolError::new(ErrorCode::ProgramFailed, format!("Couldn't reach F-Eco ({what}): {e}")))?;
    let status = res.status();
    let text = res.body_mut().read_to_string().unwrap_or_default();
    let body: Value = serde_json::from_str(&text).unwrap_or(Value::Null);
    if status.is_success() {
        return Ok(body);
    }
    let message = body["message_vi"].as_str().or_else(|| body["message"].as_str()).unwrap_or(text.trim()).to_string();
    Err(ToolError::new(ErrorCode::ProgramFailed, format!("F-Eco refused the {what} ({status}): {message}")))
}

/// Submit a version through the developer portal: the manifest, one package per platform, then
/// "submit for review". Returns the version's id and status.
pub fn submit(server: &str, token: &str, app: &str, toml: &str, packages: &[(String, PathBuf)]) -> Result<(String, String), ToolError> {
    let agent = crate::http::agent(std::time::Duration::from_secs(20), std::time::Duration::from_secs(120), std::time::Duration::from_secs(30 * 60));
    let auth = format!("Bearer {token}");
    let version = answer(
        agent.post(format!("{server}v1/dev/apps/{app}/versions")).header("authorization", &auth).header("content-type", "text/plain; charset=utf-8").send(toml),
        "manifest",
    )?;
    let id = version["id"].as_str().ok_or_else(|| ToolError::new(ErrorCode::ProgramFailed, "F-Eco didn't return the version's id."))?.to_string();
    for (platform, path) in packages {
        let mut bytes = Vec::new();
        std::fs::File::open(path).and_then(|mut f| f.read_to_end(&mut bytes)).map_err(|e| ToolError::io(path.display(), e))?;
        let sha = hex_sha256(&bytes);
        answer(
            agent
                .put(format!("{server}v1/dev/versions/{id}/packages/{platform}"))
                .header("authorization", &auth)
                .header("x-feco-sha256", &sha)
                .send(&bytes[..]),
            &format!("{platform} package"),
        )?;
    }
    let submitted = answer(agent.post(format!("{server}v1/dev/versions/{id}/submit")).header("authorization", &auth).send_empty(), "submission")?;
    Ok((id, submitted["status"].as_str().unwrap_or_default().to_string()))
}

fn hex_sha256(bytes: &[u8]) -> String {
    use sha2::Digest;
    sha2::Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

fn project_order_draft(ctx: &mut Ctx<'_>, params: Value) -> Result<ToolOutput, ToolError> {
    let p: OrderParams = parse_params(params)?;
    let vi = vi(ctx);
    let ws = Workspace::open(&p.project)?;
    let at = f_builder_engine::inbox::now_ms();
    let dir = ws.root.join("orders").join(format!("draft-{at}"));
    std::fs::create_dir_all(&dir).map_err(|e| ToolError::io(dir.display(), e))?;
    // The spec goes with the order as it is.
    let changes = f_builder_engine::openspec::changes(&ws.root);
    let mut spec = Vec::new();
    if !changes.is_empty() {
        copy_dir(&ws.root.join("openspec"), &dir.join("openspec"))?;
        spec = changes.iter().map(|c| c.dir.clone()).collect();
    }
    let source = if p.include_source && ws.app_dir().is_dir() {
        let zip_path = dir.join("source.zip");
        zip_dir(&ws.app_dir(), &zip_path)?;
        Some("source.zip")
    } else {
        None
    };
    let order = json!({
        "version": ORDER_VERSION,
        "kind": "custom_build",
        "status": "draft",
        "created_ms": at,
        "title": ws.manifest.name,
        "description": ws.manifest.request,
        "spec": spec,
        "source": source,
    });
    let path = dir.join(ORDER_FILE);
    f_builder_platform::fs::write_atomic(&path, serde_json::to_string_pretty(&order).unwrap_or_default().as_bytes())?;
    Ok(ToolOutput::Value(json!({
        "draft": path.display().to_string(),
        "spec": spec,
        "source": source,
        "note": say(vi, "Đã tạo đơn nháp \"Nhờ F-Eco xây hộ\" kèm spec. Đơn chờ bạn gửi.", "A draft \"Ask F-Eco to build it\" order was made with the spec attached. It waits for you to send it."),
    })))
}

/// Folders an order never carries.
const SKIP: &[&str] = &["node_modules", "target", ".git", "dist", "build"];
const MAX_FILES: usize = 20_000;

fn files_in(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in std::fs::read_dir(&d).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() {
                if !SKIP.iter().any(|s| e.file_name() == *s) {
                    stack.push(p);
                }
            } else if out.len() < MAX_FILES {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn copy_dir(from: &Path, to: &Path) -> Result<(), ToolError> {
    for f in files_in(from) {
        let rel = f.strip_prefix(from).unwrap_or(&f);
        let dest = to.join(rel);
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::io(parent.display(), e))?;
        }
        std::fs::copy(&f, &dest).map_err(|e| ToolError::io(f.display(), e))?;
    }
    Ok(())
}

fn zip_dir(dir: &Path, out: &Path) -> Result<(), ToolError> {
    let file = std::fs::File::create(out).map_err(|e| ToolError::io(out.display(), e))?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default();
    for f in files_in(dir) {
        let rel = f.strip_prefix(dir).unwrap_or(&f).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
        zip.start_file(rel, options).map_err(|e| ToolError::internal(e.to_string()))?;
        let mut input = std::fs::File::open(&f).map_err(|e| ToolError::io(f.display(), e))?;
        std::io::copy(&mut input, &mut zip).map_err(|e| ToolError::io(out.display(), e))?;
    }
    zip.finish().map_err(|e| ToolError::internal(e.to_string()))?.flush().map_err(|e| ToolError::io(out.display(), e))
}

#[cfg(test)]
mod tests {
    use super::*;
    use f_builder_engine::workspace::TargetRef;
    use f_builder_engine::{CallOptions, Caller, Engine};

    fn engine() -> std::sync::Arc<Engine> {
        Engine::shared(f_builder_engine::testing::session(), specs())
    }

    fn call(e: &Engine, tool: &str, params: Value) -> Result<Value, ToolError> {
        e.call(tool, params, CallOptions::new(Caller::Mcp)).map(|o| o.to_json()["result"].clone())
    }

    fn ready(dir: &Path, name: &str) -> Workspace {
        let mut ws = Workspace::create(dir, name, "Ứng dụng quản lý kho hàng", TargetRef::None).unwrap();
        ws.manifest.status = Status::Ready;
        ws.save().unwrap();
        ws
    }

    /// Spec scenario "Nộp kết quả từ chế độ đọc sâu": refused, and why.
    #[test]
    fn a_project_that_read_deeply_is_never_submitted() {
        let d = tempfile::tempdir().unwrap();
        let mut ws = ready(d.path(), "Kho");
        ws.manifest.deep_used = true;
        ws.save().unwrap();
        let e = call(&engine(), "project_store_submit", json!({"project": ws.root})).unwrap_err();
        assert_eq!(e.code, ErrorCode::ConsentRequired);
        assert!(e.message.contains("Deep mode") && e.message.contains("private use"), "{}", e.message);
        assert!(!ws.app_dir().join(MANIFEST_FILE).exists(), "nothing was drafted");
    }

    /// Spec scenario "Nộp app tự xây": a draft manifest to review first; a draft isn't submitted.
    #[test]
    fn the_first_submit_drafts_the_manifest_and_a_draft_is_not_sent() {
        if platform_here().is_none() {
            return;
        }
        let d = tempfile::tempdir().unwrap();
        let ws = ready(d.path(), "QuanLyKho");
        let e = engine();
        let r = call(&e, "project_store_submit", json!({"project": ws.root})).unwrap();
        assert_eq!(r["submitted"], false, "{r}");
        let draft = std::fs::read_to_string(ws.app_dir().join(MANIFEST_FILE)).unwrap();
        assert!(draft.starts_with(DRAFT_MARK), "{draft}");
        let again = call(&e, "project_store_submit", json!({"project": ws.root})).unwrap_err();
        assert!(again.message.contains("still a draft"), "{}", again.message);
        // Not Ready: refused before anything.
        let other = Workspace::create(d.path(), "Other", "x", TargetRef::None).unwrap();
        assert!(call(&e, "project_store_submit", json!({"project": other.root})).unwrap_err().message.contains("Ready"));
    }

    type Seen = Vec<(String, String, Vec<u8>)>;

    /// A stand-in portal: answers each request in turn and keeps what it was sent.
    fn portal(answers: Vec<&'static str>) -> (String, std::thread::JoinHandle<Seen>) {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let url = format!("http://127.0.0.1:{}/", listener.local_addr().unwrap().port());
        let handle = std::thread::spawn(move || {
            let mut got = Vec::new();
            for answer in answers {
                let (mut s, _) = listener.accept().unwrap();
                let mut buf = Vec::new();
                let mut byte = [0u8; 1];
                while !buf.ends_with(b"\r\n\r\n") {
                    s.read_exact(&mut byte).unwrap();
                    buf.push(byte[0]);
                }
                let head = String::from_utf8_lossy(&buf).to_string();
                let len = head
                    .lines()
                    .find_map(|l| l.to_ascii_lowercase().strip_prefix("content-length:").map(|v| v.trim().parse::<usize>().unwrap_or(0)))
                    .unwrap_or(0);
                let mut body = vec![0u8; len];
                s.read_exact(&mut body).unwrap();
                let first = head.lines().next().unwrap_or_default().to_string();
                got.push((first, head, body));
                let reply =
                    format!("HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{answer}", answer.len());
                s.write_all(reply.as_bytes()).unwrap();
            }
            got
        });
        (url, handle)
    }

    #[test]
    fn a_version_is_sent_as_the_portal_takes_it() {
        let d = tempfile::tempdir().unwrap();
        let zip_path = d.path().join("windows-x64.zip");
        std::fs::write(&zip_path, b"PK fake package").unwrap();
        let (server, handle) = portal(vec![r#"{"id":"v-1"}"#, r#"{"ok":true}"#, r#"{"status":"pending_review"}"#]);
        let (id, status) = submit(&server, "fdev_test", "quanlykho", "id = \"quanlykho\"\n", &[("windows-x64".into(), zip_path)]).unwrap();
        assert_eq!((id.as_str(), status.as_str()), ("v-1", "pending_review"));
        let got = handle.join().unwrap();
        assert_eq!(got[0].0, "POST /v1/dev/apps/quanlykho/versions HTTP/1.1");
        assert_eq!(got[0].2, b"id = \"quanlykho\"\n");
        assert!(got[0].1.to_ascii_lowercase().contains("authorization: bearer fdev_test"));
        assert_eq!(got[1].0, "PUT /v1/dev/versions/v-1/packages/windows-x64 HTTP/1.1");
        assert!(got[1].1.to_ascii_lowercase().contains(&format!("x-feco-sha256: {}", hex_sha256(b"PK fake package"))));
        assert_eq!(got[1].2, b"PK fake package");
        assert_eq!(got[2].0, "POST /v1/dev/versions/v-1/submit HTTP/1.1");
    }

    #[test]
    fn a_portal_refusal_is_said_in_its_own_words() {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let server = format!("http://127.0.0.1:{}/", listener.local_addr().unwrap().port());
        std::thread::spawn(move || {
            let (mut s, _) = listener.accept().unwrap();
            let mut buf = Vec::new();
            let mut byte = [0u8; 1];
            while !buf.ends_with(b"\r\n\r\n") {
                if s.read_exact(&mut byte).is_err() {
                    break;
                }
                buf.push(byte[0]);
            }
            let body = r#"{"error":"invalid_manifest","message":"bad","message_vi":"f-app.toml sai: thiếu license"}"#;
            let _ = s.write_all(
                format!(
                    "HTTP/1.1 422 Unprocessable Entity\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                    body.len()
                )
                .as_bytes(),
            );
        });
        let e = submit(&server, "t", "x", "", &[]).unwrap_err();
        assert!(e.message.contains("thiếu license") && e.message.contains("422"), "{}", e.message);
        assert!(super::server(Some("http://feco.example/")).is_err(), "only HTTPS, or this computer");
    }

    /// Spec scenario "Nhờ xây hộ": a draft order with the spec attached, waiting to be sent.
    #[test]
    fn a_build_request_draft_carries_the_spec() {
        let d = tempfile::tempdir().unwrap();
        let ws = ready(d.path(), "Kho");
        let spec = ws.root.join("openspec/changes/add-kho/specs/kho/spec.md");
        std::fs::create_dir_all(spec.parent().unwrap()).unwrap();
        std::fs::write(&spec, "### Requirement: Nhập kho\n#### Scenario: Nhập\n").unwrap();
        std::fs::create_dir_all(ws.app_dir().join("src")).unwrap();
        std::fs::write(ws.app_dir().join("src/main.rs"), "fn main() {}").unwrap();
        let r = call(&engine(), "project_order_draft", json!({"project": ws.root, "include_source": true})).unwrap();
        let path = PathBuf::from(r["draft"].as_str().unwrap());
        let order: Value = serde_json::from_str(&std::fs::read_to_string(&path).unwrap()).unwrap();
        assert_eq!((order["status"].as_str(), order["kind"].as_str(), order["version"].as_u64()), (Some("draft"), Some("custom_build"), Some(1)));
        assert_eq!(order["spec"], json!(["openspec/changes/add-kho"]));
        let dir = path.parent().unwrap();
        assert!(dir.join("openspec/changes/add-kho/specs/kho/spec.md").is_file(), "the spec is attached");
        assert!(dir.join("source.zip").is_file(), "the source, when allowed");
        std::thread::sleep(std::time::Duration::from_millis(5));
        let r = call(&engine(), "project_order_draft", json!({"project": ws.root})).unwrap();
        assert!(r["source"].is_null(), "no source unless allowed");
    }

    #[test]
    fn app_ids_follow_the_portals_rules() {
        for (name, id) in [("Quản lý kho", "qu-n-l-kho"), ("SoundLab 2", "soundlab-2"), ("---", "app"), ("Chi tiêu gia đình!", "chi-ti-u-gia-nh")] {
            assert_eq!(app_id(name), id, "{name}");
            assert!(feco_manifest::valid_app_id(&app_id(name)), "{name}");
        }
    }

    /// The draft is what the portal accepts once the person has taken the draft line out.
    #[test]
    fn the_draft_manifest_passes_the_portals_check_once_reviewed() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "QuanLyKho", "Ứng dụng quản lý kho hàng", f_builder_engine::workspace::TargetRef::None).unwrap();
        let platform = feco_manifest::Platform { os: "windows".into(), arch: "x64".into(), entry: String::new() };
        let draft = draft_manifest(&ws, &platform, "quanlykho.exe");
        assert!(draft.starts_with(DRAFT_MARK));
        let reviewed: String = draft.lines().skip(1).map(|l| format!("{l}\n")).collect();
        let m = feco_manifest::parse(&reviewed).unwrap_or_else(|e| panic!("{e:?}\n{reviewed}"));
        assert_eq!((m.id.as_str(), m.name.as_str(), m.version.as_str()), ("quanlykho", "QuanLyKho", "0.1.0"));
        assert_eq!(m.platforms[0].key(), "windows-x64");
        assert_eq!(m.platforms[0].entry, "quanlykho.exe");
    }
}
