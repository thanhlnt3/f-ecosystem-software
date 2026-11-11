//! API keys: the OS keychain (Windows Credential Manager, macOS Keychain), then the usual
//! environment variables. Keys are never logged, never written to project or settings files, and
//! never sent to F-Eco (tasks.md 11.5, spec app-builder "Dùng khóa API riêng"). When the keychain
//! can't save a key, it isn't saved anywhere else: the person is told. Only test and demo stores
//! ([`KeyStore::file_only`]) keep keys in a file.

use crate::BackendKind;
use f_builder_engine::tools::asset_tools::AssetKeys;
use f_builder_engines::assets::{AssetProvider, AssetProviders};
use f_builder_evidence::{ErrorCode, ToolError};
use std::path::PathBuf;
use std::time::Duration;

/// Service name in the OS keychain.
pub const SERVICE: &str = "io.github.thanhlnt3.f-builder";
/// Set to `1` to skip the OS keychain (headless servers): keys then come from environment variables
/// only, and none can be saved.
pub const NO_KEYCHAIN_ENV: &str = "F_BUILDER_NO_KEYCHAIN";
/// The keychain can block (a locked desktop keyring); never wait longer than this.
const KEYCHAIN_TIMEOUT: Duration = Duration::from_secs(3);

const FILE_HEADER: &str = "# F-Builder AI keys: FALLBACK STORE.\n# These keys are here because no OS keychain was available when they were saved.\n# This file is readable only by you (0600). Delete a line to forget a key.\n";

/// Where the fal.ai key for generated game assets is stored.
pub const FAL_KEY: &str = "assets:fal";

/// Where an asset provider's key is stored (`assets:<provider id>`).
pub fn asset_key_name(provider: &str) -> String {
    format!("assets:{provider}")
}

/// An asset provider's key: stored, else the provider's own environment variable.
pub fn asset_key(store: &KeyStore, provider: &dyn AssetProvider) -> Option<String> {
    store.get(&asset_key_name(provider.id())).or_else(|| provider.key_env().and_then(|v| std::env::var(v).ok())).filter(|k| !k.trim().is_empty())
}

/// Every provider's key that is set up, for the builder's tool server.
pub fn asset_keys(store: &KeyStore, providers: &AssetProviders) -> AssetKeys {
    AssetKeys(
        providers.ids().into_iter().filter_map(|id| providers.get(id)).filter_map(|p| asset_key(store, p.as_ref()).map(|k| (p.id().to_string(), k))).collect(),
    )
}

/// After a key changed: keys a run already loaded into `session` follow, so a removed one stops being
/// spent at once. A session with none loaded stays that way. Reads the keychain, so call it off the UI
/// thread; a tool passes the session it holds, because locking it again would wait forever.
pub fn refresh_loaded(session: &mut f_builder_engine::Session, store: &KeyStore) {
    if session.ext.get::<AssetKeys>().is_some() {
        let providers = session.ext.get::<AssetProviders>().map(|p| p.as_ref().clone()).unwrap_or_default();
        session.ext.insert(std::sync::Arc::new(asset_keys(store, &providers)));
    }
}

/// The fal.ai key for game assets: stored, else fal's environment variable.
pub fn fal_key(store: &KeyStore) -> Option<String> {
    asset_key(store, &f_builder_engines::assets::fal::Fal::default())
}

/// The environment variable conventionally holding a key for this kind.
pub fn env_var(kind: BackendKind) -> Option<&'static str> {
    match kind {
        BackendKind::Anthropic => Some("ANTHROPIC_API_KEY"),
        BackendKind::Openai => Some("OPENAI_API_KEY"),
        BackendKind::Openrouter => Some("OPENROUTER_API_KEY"),
        BackendKind::Gemini => Some("GEMINI_API_KEY"),
        BackendKind::OpenaiCompatible | BackendKind::AgentCli => None,
    }
}

/// Where keys live.
#[derive(Debug, Clone)]
pub struct KeyStore {
    file: PathBuf,
    use_keychain: bool,
    /// Keys may be kept in `file` (test and demo stores only).
    file_store: bool,
}

impl KeyStore {
    /// The default store: the OS keychain only.
    pub fn system() -> Self {
        let no_chain = std::env::var(NO_KEYCHAIN_ENV).is_ok_and(|v| v == "1");
        KeyStore { file: f_builder_engine::settings::config_dir().join("ai-keys.toml"), use_keychain: !no_chain, file_store: false }
    }

    /// A store that keeps keys in `file`, never in the keychain: tests and the demo app only, so
    /// they never touch the person's keychain.
    pub fn file_only(file: PathBuf) -> Self {
        KeyStore { file, use_keychain: false, file_store: true }
    }

    fn read_file(&self) -> toml::Table {
        if !self.file_store {
            return toml::Table::new();
        }
        std::fs::read_to_string(&self.file).ok().and_then(|t| toml::from_str(&t).ok()).unwrap_or_default()
    }

    /// The file's keys before changing them: none when it doesn't exist, an error when it can't be
    /// read, so a save never replaces a file it couldn't read and drops the other keys (#262).
    fn read_file_to_change(&self) -> Result<toml::Table, ToolError> {
        let unreadable = |why: String| {
            ToolError::new(ErrorCode::FileError, format!("Couldn't read the key file {}: {why}", self.file.display()))
                .with_remediation("Fix or move that file (it was left untouched), then save the key again.")
        };
        match std::fs::read(&self.file) {
            Ok(b) => {
                let text = String::from_utf8(b).map_err(|_| unreadable("it isn't valid UTF-8".into()))?;
                toml::from_str(&text).map_err(|e| unreadable(e.to_string()))
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(toml::Table::new()),
            Err(e) => Err(unreadable(e.to_string())),
        }
    }

    fn write_file(&self, t: &toml::Table) -> Result<(), ToolError> {
        let body = toml::to_string(t).map_err(|e| ToolError::internal(e.to_string()))?;
        f_builder_platform::write_atomic_private(&self.file, format!("{FILE_HEADER}{body}").as_bytes())
    }

    /// The stored key for a backend.
    pub fn get(&self, backend: &str) -> Option<String> {
        if self.use_keychain
            && let Some(k) = keychain::get(backend)
        {
            return Some(k);
        }
        self.read_file().get(backend).and_then(|v| v.as_str()).map(str::to_string).filter(|k| !k.is_empty())
    }

    /// The key to use: stored, else the kind's environment variable.
    pub fn resolve(&self, backend: &str, kind: BackendKind) -> Option<String> {
        self.get(backend).or_else(|| env_var(kind).and_then(|v| std::env::var(v).ok()).filter(|k| !k.trim().is_empty()))
    }

    /// Save a key. Returns where it went: `"keychain"` or `"file"`.
    pub fn set(&self, backend: &str, key: &str) -> Result<&'static str, ToolError> {
        if key.trim().is_empty() {
            return Err(ToolError::invalid("The key is empty."));
        }
        if self.use_keychain && keychain::set(backend, key) {
            return Ok("keychain");
        }
        if !self.file_store {
            return Err(ToolError::new(ErrorCode::NoPermission, "The system's secure store (keychain) couldn't save the key, so it wasn't saved.")
                .with_remediation(if self.use_keychain {
                    "Unlock your keychain (Windows: Credential Manager; macOS: Keychain Access) and save the key again, or set the provider's environment variable."
                } else {
                    "The keychain is turned off (F_BUILDER_NO_KEYCHAIN=1): set the provider's environment variable instead."
                }));
        }
        let mut t = self.read_file_to_change()?;
        t.insert(backend.to_string(), toml::Value::String(key.to_string()));
        self.write_file(&t).map_err(|e| ToolError::new(ErrorCode::NoPermission, format!("Couldn't save the key: {}", e.message)))?;
        Ok("file")
    }

    pub fn delete(&self, backend: &str) -> Result<(), ToolError> {
        if self.use_keychain {
            keychain::delete(backend);
        }
        if self.file_store {
            let mut t = self.read_file_to_change()?;
            if t.remove(backend).is_some() {
                self.write_file(&t)?;
            }
        }
        // A locked or busy keychain may have refused, and it's read first, so the key would still
        // work. Reading back, not the delete's result: with no keychain at all the delete fails too.
        if self.use_keychain && keychain::get(backend).is_some() {
            return Err(ToolError::new(ErrorCode::NoPermission, "The system keychain still has this key, so it wasn't removed.")
                .with_remediation("Unlock the keychain and try again, or delete the entry from your keychain app."));
        }
        Ok(())
    }
}

/// Run a keychain operation on a helper thread with a deadline.
fn bounded<T: Send + 'static>(f: impl FnOnce() -> Option<T> + Send + 'static) -> Option<T> {
    let (tx, rx) = std::sync::mpsc::channel();
    std::thread::Builder::new()
        .name("keychain".into())
        .spawn(move || {
            let _ = tx.send(f());
        })
        .ok()?;
    rx.recv_timeout(KEYCHAIN_TIMEOUT).ok().flatten()
}

#[cfg(any(target_os = "linux", target_os = "macos", windows))]
mod keychain {
    use super::{SERVICE, bounded};

    pub fn get(backend: &str) -> Option<String> {
        let b = backend.to_string();
        bounded(move || keyring::Entry::new(SERVICE, &b).ok()?.get_password().ok())
    }

    pub fn set(backend: &str, key: &str) -> bool {
        let (b, k) = (backend.to_string(), key.to_string());
        bounded(move || keyring::Entry::new(SERVICE, &b).ok()?.set_password(&k).ok()).is_some()
    }

    pub fn delete(backend: &str) {
        let b = backend.to_string();
        let _ = bounded(move || keyring::Entry::new(SERVICE, &b).ok()?.delete_credential().ok());
    }
}

#[cfg(not(any(target_os = "linux", target_os = "macos", windows)))]
mod keychain {
    pub fn get(_: &str) -> Option<String> {
        let _ = super::bounded(|| None::<()>);
        None
    }
    pub fn set(_: &str, _: &str) -> bool {
        false
    }
    pub fn delete(_: &str) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    /// tasks.md 11.5 (spec app-builder "Dùng khóa API riêng"): the person's store keeps keys in the
    /// keychain only; when it can't, the key isn't saved, and no file is written.
    #[test]
    fn the_system_store_never_falls_back_to_a_file() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai-keys.toml");
        // As `system()` with the keychain off, pointed at a temp folder.
        let s = KeyStore { file: file.clone(), use_keychain: false, file_store: false };
        let e = s.set("anthropic", "sk-test-not-a-real-key").unwrap_err();
        assert_eq!(e.code, ErrorCode::NoPermission);
        assert!(e.message.contains("wasn't saved"), "{}", e.message);
        assert!(!file.exists(), "no key file is written");
        // A key file left by an older build isn't read either.
        std::fs::write(&file, "anthropic = \"sk-test-old\"\n").unwrap();
        assert_eq!(s.get("anthropic"), None);
        s.delete("anthropic").unwrap();
        assert!(file.exists(), "and isn't touched");
    }

    /// The key doesn't reach the settings file (which people copy between computers) or any other
    /// file in the config folder.
    #[test]
    fn a_saved_key_is_in_no_settings_or_other_file() {
        let dir = tempfile::tempdir().unwrap();
        let keys = dir.path().join("keys").join("ai-keys.toml");
        std::fs::create_dir_all(keys.parent().unwrap()).unwrap();
        let engine = std::sync::Arc::new(f_builder_engine::Engine::new(f_builder_engine::testing::session()));
        let hub = crate::AiHub::with_stores(engine, dir.path().join("settings.toml"), KeyStore::file_only(keys.clone()));
        let mut cfg = hub.config();
        cfg.upsert(crate::BackendEntry {
            config: crate::BackendConfig {
                name: "Claude".into(),
                kind: BackendKind::Anthropic,
                model: "claude".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            options: crate::BackendOptions::default(),
        });
        hub.save_config(&cfg).unwrap();
        hub.keys.set("Claude", "sk-test-secret-1234567890").unwrap();
        for e in walk(dir.path()) {
            if e != keys {
                let text = std::fs::read(&e).unwrap_or_default();
                assert!(!String::from_utf8_lossy(&text).contains("sk-test-secret-1234567890"), "the key is in {}", e.display());
            }
        }
    }

    fn walk(root: &std::path::Path) -> Vec<PathBuf> {
        let mut out = Vec::new();
        for e in std::fs::read_dir(root).into_iter().flatten().flatten() {
            let p = e.path();
            if p.is_dir() { out.extend(walk(&p)) } else { out.push(p) }
        }
        out
    }

    /// A second provider with no environment variable, so nothing here reads the real one.
    struct Other;

    impl AssetProvider for Other {
        fn id(&self) -> &'static str {
            "other"
        }
        fn name(&self) -> &'static str {
            "Other"
        }
        fn generate(&self, _key: &str, _req: &f_builder_engines::assets::AssetRequest) -> Result<f_builder_engines::assets::Generated, ToolError> {
            Err(ToolError::invalid("not here"))
        }
    }

    #[test]
    fn each_asset_provider_has_its_own_key() {
        let d = tempfile::tempdir().unwrap();
        let s = KeyStore::file_only(d.path().join("keys.toml"));
        let mut providers = AssetProviders::built_in();
        providers.register(std::sync::Arc::new(Other));
        s.set(&asset_key_name("other"), "o-1").unwrap();
        let keys = asset_keys(&s, &providers);
        assert_eq!(keys.get("other"), Some("o-1"));
        // fal's is stored where it always was, apart from the other provider's.
        assert_eq!(asset_key_name("fal"), FAL_KEY);
        s.set(FAL_KEY, "f-1").unwrap();
        let keys = asset_keys(&s, &providers);
        assert_eq!((keys.get("fal"), keys.get("other")), (Some("f-1"), Some("o-1")));
        s.delete(&asset_key_name("other")).unwrap();
        assert_eq!(asset_keys(&s, &providers).get("other"), None);
        assert_eq!(fal_key(&s).as_deref(), Some("f-1"));
    }

    /// #923: a key removed in Settings must stop being spent, in a run that already loaded it too.
    #[test]
    fn keys_loaded_by_a_run_follow_a_change_and_none_appear_otherwise() {
        let d = tempfile::tempdir().unwrap();
        let s = KeyStore::file_only(d.path().join("keys.toml"));
        let mut providers = AssetProviders::built_in();
        providers.register(std::sync::Arc::new(Other));
        s.set(&asset_key_name("other"), "o-1").unwrap();
        let mut session = f_builder_engine::testing::session();
        session.ext.insert(std::sync::Arc::new(providers));
        refresh_loaded(&mut session, &s);
        assert!(session.ext.get::<AssetKeys>().is_none(), "no run loaded any");

        session.ext.insert(std::sync::Arc::new(AssetKeys([("other".to_string(), "old".to_string())].into())));
        refresh_loaded(&mut session, &s);
        assert_eq!(session.ext.get::<AssetKeys>().unwrap().get("other"), Some("o-1"), "a changed key");
        s.delete(&asset_key_name("other")).unwrap();
        refresh_loaded(&mut session, &s);
        assert_eq!(session.ext.get::<AssetKeys>().unwrap().get("other"), None, "a removed key");
    }

    /// Its key can't be removed: the AI stays in the list, so removing it can be tried again.
    #[test]
    fn an_ai_whose_key_cannot_be_removed_stays_to_try_again() {
        let d = tempfile::tempdir().unwrap();
        let file = d.path().join("keys.toml");
        let engine = std::sync::Arc::new(f_builder_engine::Engine::new(f_builder_engine::testing::session()));
        let hub = crate::AiHub::with_stores(engine, d.path().join("settings.toml"), KeyStore::file_only(file.clone()));
        let mut cfg = hub.config();
        cfg.upsert(crate::BackendEntry {
            config: crate::BackendConfig {
                name: "Mine".into(),
                kind: BackendKind::Openrouter,
                model: "m".into(),
                base_url: None,
                ssh_host: None,
                enabled: true,
            },
            options: Default::default(),
        });
        hub.engine.session().settings.ai = cfg.to_table().unwrap();
        std::fs::write(&file, "not [ toml").unwrap();
        assert!(hub.remove_backend("Mine").is_err());
        assert!(hub.config().get("Mine").is_some(), "it can still be picked to try again");
        std::fs::remove_file(&file).unwrap();
        assert!(hub.remove_backend("Mine").unwrap());
        assert!(hub.config().get("Mine").is_none());
    }

    /// #262: a key file that can't be read is left as it is, with every other key in it.
    #[test]
    fn an_unreadable_key_file_is_never_replaced() {
        let dir = tempfile::tempdir().unwrap();
        let file = dir.path().join("ai-keys.toml");
        let s = KeyStore::file_only(file.clone());
        let valid = b"FakeBackend = \"sk-test-backend\"\n\"assets:fal\" = \"sk-test-old\"\n".to_vec();
        let broken_toml = b"FakeBackend = \"sk-test-backend\"\n\"assets:fal\" = \"sk-test-old\n".to_vec();
        let broken_utf8 = [valid.clone(), b"\xff".to_vec()].concat();
        for bad in [broken_toml, broken_utf8] {
            std::fs::write(&file, &bad).unwrap();
            let e = s.set(FAL_KEY, "sk-test-new").unwrap_err();
            assert!(e.message.contains("Couldn't read the key file"), "{e:?}");
            assert!(s.delete("FakeBackend").is_err());
            assert_eq!(std::fs::read(&file).unwrap(), bad, "the file was changed");
        }
        std::fs::write(&file, &valid).unwrap();
        assert_eq!(s.set(FAL_KEY, "sk-test-new").unwrap(), "file");
        assert_eq!((s.get("FakeBackend").as_deref(), s.get(FAL_KEY).as_deref()), (Some("sk-test-backend"), Some("sk-test-new")));
    }

    #[test]
    fn file_fallback_is_labelled_and_private() {
        let dir = tempfile::tempdir().unwrap();
        let s = KeyStore::file_only(dir.path().join("ai-keys.toml"));
        assert_eq!(s.set("Claude", "sk-test").unwrap(), "file");
        assert_eq!(s.get("Claude").as_deref(), Some("sk-test"));
        let text = std::fs::read_to_string(dir.path().join("ai-keys.toml")).unwrap();
        assert!(text.starts_with("# F-Builder AI keys: FALLBACK STORE."));
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt as _;
            assert_eq!(std::fs::metadata(dir.path().join("ai-keys.toml")).unwrap().permissions().mode() & 0o777, 0o600);
        }
        s.delete("Claude").unwrap();
        assert!(s.get("Claude").is_none());
        assert!(s.set("x", " ").is_err());
        assert_eq!(env_var(BackendKind::Openrouter), Some("OPENROUTER_API_KEY"));
        assert_eq!(env_var(BackendKind::Gemini), Some("GEMINI_API_KEY"));
    }
}
