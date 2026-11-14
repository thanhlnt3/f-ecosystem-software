//! Công cụ dòng lệnh quản lý kho TUF của F-Eco. Xem `docs/security/tuf-keys.md`.

use feco_tuf::{
    keys, Client, Expiry, InitKeys, Key, Repo, RoleType, RootChange, SyncState, TargetMeta,
};
use jiff::Timestamp;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use tough::key_source::KeySource;

const USAGE: &str = "\
feco-tuf: quản lý kho TUF của F-Eco

  feco-tuf keygen <file>                     tạo khóa Ed25519 mới (không ghi đè)
  feco-tuf keyid <file>                      in key ID của một khóa
  feco-tuf init --repo DIR --root A,B,C --targets T --snapshot S --timestamp TS --sign K,...
  feco-tuf publish --repo DIR --file F --name N --app A --version V --platform P --sign T,S,TS
                   [--title TÊN] [--entry FILE-CHẠY] [--client-id ID] [--private true]
                   [--summary MÔ-TẢ] [--category NHÓM] [--developer TÊN] [--source URL]
                   [--terminal true] [--description TEXT] [--features A|B|C] [--website URL]
                   [--release-notes TEXT] [--license SPDX] [--released-at RFC3339]
  feco-tuf revoke --repo DIR --name N --reason TEXT --sign T,S,TS
  feco-tuf remove --repo DIR --app A --sign T,S,TS   gỡ mọi phiên bản của app khỏi danh mục
                                             (không thu hồi: máy đã cài vẫn mở được)
  feco-tuf refresh --repo DIR --sign S,TS    ký lại snapshot và timestamp (chạy định kỳ)
  feco-tuf resign --repo DIR --sign T,S,TS   gia hạn targets khi không có phát hành mới
  feco-tuf rotate --repo DIR --role root|targets|snapshot|timestamp --keys K,... [--threshold N]
                  --sign <khóa root đủ ngưỡng cũ và mới>,T,S,TS
  feco-tuf status --repo DIR [--trusted-root FILE] [--datastore DIR]

--sign: danh sách file khóa dùng để ký, ngăn bởi dấu phẩy. Khóa root chỉ dùng trên máy ngoại tuyến.
";

#[tokio::main]
async fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match run(&args).await {
        Ok(()) => ExitCode::SUCCESS,
        Err(message) => {
            eprintln!("lỗi: {message}");
            ExitCode::FAILURE
        }
    }
}

struct Args {
    command: String,
    positional: Vec<String>,
    flags: HashMap<String, String>,
}

impl Args {
    fn parse(raw: &[String]) -> Result<Self, String> {
        let mut iter = raw.iter();
        let command = iter.next().cloned().ok_or_else(|| USAGE.to_owned())?;
        let mut positional = Vec::new();
        let mut flags = HashMap::new();
        while let Some(arg) = iter.next() {
            if let Some(name) = arg.strip_prefix("--") {
                let value = iter
                    .next()
                    .ok_or_else(|| format!("--{name} cần một giá trị"))?;
                flags.insert(name.to_owned(), value.clone());
            } else {
                positional.push(arg.clone());
            }
        }
        Ok(Self {
            command,
            positional,
            flags,
        })
    }

    fn get(&self, name: &str) -> Result<&str, String> {
        self.flags
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| format!("thiếu --{name}\n\n{USAGE}"))
    }

    fn list(&self, name: &str) -> Result<Vec<PathBuf>, String> {
        Ok(self
            .get(name)?
            .split(',')
            .filter(|s| !s.trim().is_empty())
            .map(|s| PathBuf::from(s.trim()))
            .collect())
    }

    fn repo(&self) -> Result<Repo, String> {
        Ok(Repo::open(self.get("repo")?))
    }

    fn signers(&self) -> Result<Vec<Box<dyn KeySource>>, String> {
        Ok(self.list("sign")?.into_iter().map(keys::source).collect())
    }
}

async fn public_keys(paths: &[PathBuf]) -> Result<Vec<Key>, String> {
    let mut out = Vec::new();
    for path in paths {
        out.push(
            keys::public_key(keys::source(path).as_ref())
                .await
                .map_err(|e| e.to_string())?,
        );
    }
    Ok(out)
}

fn role(name: &str) -> Result<RoleType, String> {
    match name {
        "root" => Ok(RoleType::Root),
        "targets" => Ok(RoleType::Targets),
        "snapshot" => Ok(RoleType::Snapshot),
        "timestamp" => Ok(RoleType::Timestamp),
        other => Err(format!("vai trò không hợp lệ: {other}")),
    }
}

async fn run(raw: &[String]) -> Result<(), String> {
    let args = Args::parse(raw)?;
    let expiry = Expiry::default();
    let now = Timestamp::now();
    let e = |e: feco_tuf::Error| e.to_string();
    match args.command.as_str() {
        "keygen" | "keyid" => {
            let path = args
                .positional
                .first()
                .ok_or_else(|| format!("thiếu đường dẫn file khóa\n\n{USAGE}"))?;
            let id = if args.command == "keygen" {
                keys::generate(Path::new(path)).await.map_err(e)?
            } else {
                keys::key_id(keys::source(path).as_ref()).await.map_err(e)?
            };
            println!("{id}");
        }
        "init" => {
            let init = InitKeys {
                root: public_keys(&args.list("root")?).await?,
                root_threshold: 2,
                targets: public_keys(&args.list("targets")?).await?,
                snapshot: public_keys(&args.list("snapshot")?).await?,
                timestamp: public_keys(&args.list("timestamp")?).await?,
            };
            let repo = args.repo()?;
            repo.init(&init, &args.signers()?, expiry, now)
                .await
                .map_err(e)?;
            println!("đã dựng kho tại {}", repo.dir().display());
        }
        "publish" => {
            let meta = TargetMeta {
                app: args.get("app")?.to_owned(),
                version: args.get("version")?.to_owned(),
                platform: args.get("platform")?.to_owned(),
                title: args.flags.get("title").cloned(),
                entry: args.flags.get("entry").cloned(),
                client_id: args.flags.get("client-id").cloned(),
                private: args.flags.get("private").is_some_and(|v| v == "true"),
                summary: args.flags.get("summary").cloned(),
                category: args.flags.get("category").cloned(),
                developer: args.flags.get("developer").cloned(),
                source: args.flags.get("source").cloned(),
                terminal: args.flags.get("terminal").is_some_and(|v| v == "true"),
                description: args.flags.get("description").cloned(),
                features: args
                    .flags
                    .get("features")
                    .map(|f| {
                        f.split('|')
                            .map(str::trim)
                            .filter(|x| !x.is_empty())
                            .map(str::to_owned)
                            .collect()
                    })
                    .unwrap_or_default(),
                website: args.flags.get("website").cloned(),
                release_notes: args.flags.get("release-notes").cloned(),
                license: args.flags.get("license").cloned(),
                released_at: Some(
                    args.flags
                        .get("released-at")
                        .cloned()
                        .unwrap_or_else(|| now.to_string()),
                ),
            };
            let name = args.get("name")?;
            args.repo()?
                .publish(
                    Path::new(args.get("file")?),
                    name,
                    &meta,
                    &args.signers()?,
                    expiry,
                    now,
                )
                .await
                .map_err(e)?;
            println!("đã ký target {name}");
        }
        "revoke" => {
            let revocation = args
                .repo()?
                .revoke(
                    args.get("name")?,
                    args.get("reason")?,
                    &args.signers()?,
                    expiry,
                    now,
                )
                .await
                .map_err(e)?;
            println!(
                "đã thu hồi {} lúc {}: {}",
                revocation.target, revocation.revoked_at, revocation.reason
            );
        }
        "remove" => {
            let app = args.get("app")?;
            let removed = args
                .repo()?
                .remove_app(app, &args.signers()?, expiry, now)
                .await
                .map_err(e)?;
            println!("đã gỡ {app} khỏi danh mục: {}", removed.join(", "));
        }
        "refresh" => {
            args.repo()?
                .refresh(&args.signers()?, expiry, now)
                .await
                .map_err(e)?;
            println!("đã ký lại snapshot và timestamp");
        }
        "resign" => {
            args.repo()?
                .resign(&args.signers()?, expiry, now)
                .await
                .map_err(e)?;
            println!("đã ký lại targets, snapshot và timestamp");
        }
        "rotate" => {
            let role = role(args.get("role")?)?;
            let new_keys = public_keys(&args.list("keys")?).await?;
            let threshold = match args.flags.get("threshold") {
                Some(t) => t
                    .parse()
                    .map_err(|_| format!("--threshold không hợp lệ: {t}"))?,
                None if role == RoleType::Root => 2,
                None => 1,
            };
            let repo = args.repo()?;
            let signers = args.signers()?;
            let draft = repo
                .draft_root_update(
                    &[RootChange {
                        role,
                        keys: new_keys,
                        threshold,
                    }],
                    expiry,
                    now,
                )
                .await
                .map_err(e)?;
            let signed = draft.sign(&signers).await.map_err(e)?;
            repo.rotate(&draft, &signed, &signers, expiry, now)
                .await
                .map_err(e)?;
            println!(
                "đã phát hành root phiên bản {} ({} chữ ký)",
                draft.next().version,
                signed.signatures.len()
            );
        }
        "status" => {
            let repo = args.repo()?;
            let trusted_root = match args.flags.get("trusted-root") {
                Some(p) => PathBuf::from(p),
                None => repo.root_path(1),
            };
            // Không có --datastore: kiểm như một launcher mới cài, rồi xóa thư mục tạm.
            let (datastore, temporary) = match args.flags.get("datastore") {
                Some(p) => (PathBuf::from(p), false),
                None => (
                    std::env::temp_dir().join(format!(
                        "feco-tuf-status-{}-{}",
                        std::process::id(),
                        now.as_millisecond()
                    )),
                    true,
                ),
            };
            let client = Client::for_local_repo(&repo, &trusted_root, &datastore)
                .await
                .map_err(e)?;
            let state = client.sync().await;
            if temporary {
                let _ = std::fs::remove_dir_all(&datastore);
            }
            print_state(&state.map_err(e)?);
        }
        "help" | "--help" | "-h" => print!("{USAGE}"),
        other => return Err(format!("lệnh không hợp lệ: {other}\n\n{USAGE}")),
    }
    Ok(())
}

fn print_state(state: &SyncState) {
    println!(
        "xác minh OK: root v{}, targets v{}, snapshot v{}, timestamp v{}; hết hạn sớm nhất {}",
        state.root_version,
        state.targets_version,
        state.snapshot_version,
        state.timestamp_version,
        state.expires
    );
    for info in state.targets.values() {
        println!(
            "  {} ({} byte, sha256 {})",
            info.name, info.length, info.sha256
        );
    }
    for r in &state.revocations {
        println!(
            "  đã thu hồi {} lúc {}: {}",
            r.target, r.revoked_at, r.reason
        );
    }
}
