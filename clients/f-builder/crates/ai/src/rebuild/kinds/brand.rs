//! The reference product's name in the new app (tasks.md 11.4, spec app-builder "Không sao chép
//! thương hiệu và tài sản", scenario "Tên gốc trong giao diện"): the Check step looks through the
//! app's interface files for the name of the website or program it was made from, unless the person
//! confirmed the reference is theirs or that they hold the rights.

use std::path::{Path, PathBuf};

use f_builder_engine::workspace::{CheckResult, TargetRef, Workspace};
use f_builder_evidence::ToolError;

use super::{Check, CheckCx};

/// Files whose text can reach the interface (pages, components, window titles, app manifests).
const UI_EXTENSIONS: &[&str] = &[
    "html", "htm", "js", "jsx", "ts", "tsx", "mjs", "vue", "svelte", "rs", "py", "json", "toml", "xml", "plist", "xaml", "cs", "kt", "swift", "dart", "gd",
    "tscn", "txt",
];
/// Folders that are build output, downloaded packages or version control, not the app's own text.
const SKIP_DIRS: &[&str] = &["node_modules", "target", ".git", "dist", "build", ".next", "out", "__pycache__", ".venv", "vendor", "Library"];
/// Names too common to mean the reference (a site at app.example.com isn't called "app").
const GENERIC: &[&str] = &["www", "app", "apps", "web", "home", "site", "localhost", "example", "index", "main", "test", "demo", "login", "portal", "online"];
const MAX_FILES: usize = 5_000;
const MAX_FILE_BYTES: u64 = 2 * 1024 * 1024;

pub struct BrandCheck;

impl BrandCheck {
    pub const ID: &'static str = "reference_name";
}

impl Check for BrandCheck {
    fn id(&self) -> &'static str {
        Self::ID
    }

    fn applies(&self, cx: &CheckCx) -> bool {
        cx.ws.manifest.target != TargetRef::None
    }

    fn run(&self, cx: &mut CheckCx) -> Result<CheckResult, ToolError> {
        let vi = cx.language().starts_with("vi");
        let names = reference_names(cx.ws);
        if cx.ws.manifest.rights_confirmed_ms.is_some() {
            let note = if vi {
                "Bạn đã xác nhận có quyền với sản phẩm tham khảo, nên tên của nó được phép dùng."
            } else {
                "You confirmed you hold the rights to the reference, so its name may be used."
            };
            return Ok(CheckResult::new(Self::ID, cx.round, true, note.to_string()));
        }
        let found = find_names(&cx.ws.app_dir(), &names);
        let Some((name, file, line)) = found.first() else {
            let note =
                if vi { "Giao diện không có tên của sản phẩm tham khảo." } else { "The interface doesn't show the reference product's name." };
            return Ok(CheckResult::new(Self::ID, cx.round, true, note.to_string()));
        };
        let rel = file.strip_prefix(&cx.ws.root).unwrap_or(file).display().to_string();
        let more = found.len().saturating_sub(1);
        let note = if vi {
            format!(
                "Giao diện chứa tên của sản phẩm tham khảo: \"{name}\" ở {rel}:{line}{}. Hãy dùng tên riêng của app, trừ khi bạn xác nhận có quyền dùng tên này.",
                if more > 0 { format!(" và {more} chỗ khác") } else { String::new() }
            )
        } else {
            format!(
                "The interface contains the reference product's name: \"{name}\" in {rel}:{line}{}. Give the app its own name, unless the person confirms they may use this one.",
                if more > 0 { format!(" and {more} more places") } else { String::new() }
            )
        };
        Ok(CheckResult::new(Self::ID, cx.round, false, note))
    }
}

/// The names the reference goes by: a website's own name (its address, its page title from the
/// capture) or a program's file name. Generic words and very short names are left out.
pub fn reference_names(ws: &Workspace) -> Vec<String> {
    let mut names = Vec::new();
    match &ws.manifest.target {
        TargetRef::Website { url } => {
            let host = url.split("://").nth(1).unwrap_or(url).split(['/', ':', '?', '#']).next().unwrap_or_default().to_lowercase();
            let labels: Vec<&str> = host.split('.').filter(|l| !l.is_empty()).collect();
            // The label before the top-level domain: "notion" in www.notion.so, "shop" in shop.co.uk is skipped as generic only if listed.
            if let Some(label) = labels.iter().rev().nth(1).or(labels.first()) {
                names.push((*label).to_string());
            }
            if let Some(title) = page_title(&ws.capture_dir().join("page.html")) {
                names.push(title);
            }
        }
        TargetRef::Local { path } => {
            if let Some(stem) = path.file_stem() {
                names.push(stem.to_string_lossy().into_owned());
            }
        }
        TargetRef::None => {}
    }
    let mut out: Vec<String> = Vec::new();
    for n in names {
        let n = n.trim().to_string();
        let lower = n.to_lowercase();
        if n.chars().count() >= 4 && !GENERIC.contains(&lower.as_str()) && !out.iter().any(|o| o.to_lowercase() == lower) {
            out.push(n);
        }
    }
    out
}

/// The first part of a page's `<title>` ("Notion – Your wiki" → "Notion").
fn page_title(path: &Path) -> Option<String> {
    let html = std::fs::read_to_string(path).ok()?;
    let lower = html.to_lowercase();
    let start = lower.find("<title")?;
    let open_end = lower.get(start..)?.find('>')? + start + 1;
    let end = lower.get(open_end..)?.find("</title>")? + open_end;
    let title = html.get(open_end..end)?.trim();
    let first = title.split([' ', '|', '-', '–', '—', ':', '·']).find(|p| !p.trim().is_empty())?.trim();
    (first.chars().count() <= 40).then(|| first.to_string())
}

/// Where in the app's interface files any of `names` appears as a whole word, case ignored: (name,
/// file, line), in file order. Bounded to [`MAX_FILES`] files of at most [`MAX_FILE_BYTES`].
pub fn find_names(app: &Path, names: &[String]) -> Vec<(String, PathBuf, usize)> {
    let mut found = Vec::new();
    if names.is_empty() {
        return found;
    }
    let lowered: Vec<(String, String)> = names.iter().map(|n| (n.clone(), n.to_lowercase())).collect();
    let mut stack = vec![app.to_path_buf()];
    let mut files = 0usize;
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else { continue };
        let mut entries: Vec<_> = entries.flatten().collect();
        entries.sort_by_key(|e| e.file_name());
        for e in entries {
            let path = e.path();
            let Ok(meta) = e.metadata() else { continue };
            if meta.is_dir() {
                if !SKIP_DIRS.iter().any(|s| e.file_name() == *s) {
                    stack.push(path);
                }
                continue;
            }
            let ext = path.extension().map(|x| x.to_string_lossy().to_lowercase()).unwrap_or_default();
            if !UI_EXTENSIONS.contains(&ext.as_str()) || meta.len() > MAX_FILE_BYTES {
                continue;
            }
            files += 1;
            if files > MAX_FILES {
                return found;
            }
            let Ok(text) = std::fs::read_to_string(&path) else { continue };
            for (i, line) in text.lines().enumerate() {
                let l = line.to_lowercase();
                for (name, low) in &lowered {
                    if contains_word(&l, low) {
                        found.push((name.clone(), path.clone(), i + 1));
                    }
                }
            }
        }
    }
    found
}

/// `word` in `text` with no letter or digit right before or after it.
fn contains_word(text: &str, word: &str) -> bool {
    let mut from = 0;
    while let Some(pos) = text.get(from..).and_then(|t| t.find(word)) {
        let at = from + pos;
        let before = text.get(..at).and_then(|t| t.chars().next_back());
        let after = text.get(at + word.len()..).and_then(|t| t.chars().next());
        if !before.is_some_and(char::is_alphanumeric) && !after.is_some_and(char::is_alphanumeric) {
            return true;
        }
        from = at + word.len().max(1);
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    fn site(url: &str) -> (tempfile::TempDir, Workspace) {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "p", "Make one like it", TargetRef::Website { url: url.into() }).unwrap();
        (d, ws)
    }

    #[test]
    fn a_websites_names_come_from_its_address_and_title() {
        let (_d, ws) = site("https://www.notion.so/product");
        std::fs::create_dir_all(ws.capture_dir()).unwrap();
        std::fs::write(ws.capture_dir().join("page.html"), "<html><head><title>Notion – Your connected workspace</title></head></html>").unwrap();
        assert_eq!(reference_names(&ws), ["notion"], "the same name once");
        let (_d, ws) = site("https://app.example.com/");
        assert!(reference_names(&ws).is_empty(), "generic words aren't names");
        let (_d, ws) = site("https://quanlykho.vn");
        assert_eq!(reference_names(&ws), ["quanlykho"]);
    }

    /// Spec scenario "Tên gốc trong giao diện": the name on the title bar is found.
    #[test]
    fn the_reference_name_in_the_title_bar_is_found_as_a_word() {
        let d = tempfile::tempdir().unwrap();
        let app = d.path();
        std::fs::create_dir_all(app.join("src")).unwrap();
        std::fs::create_dir_all(app.join("node_modules/x")).unwrap();
        std::fs::write(app.join("src/main.rs"), "fn main() {\n    let title = \"Notion Clone\";\n}\n").unwrap();
        std::fs::write(app.join("node_modules/x/index.js"), "Notion").unwrap();
        std::fs::write(app.join("src/notes.rs"), "// notional value, not the name\n").unwrap();
        let found = find_names(app, &["notion".into()]);
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!((found[0].1.ends_with("main.rs"), found[0].2), (true, 2));
        assert!(find_names(app, &["kho".into()]).is_empty());
    }

    #[test]
    fn whole_words_only() {
        assert!(contains_word("my notion app", "notion"));
        assert!(contains_word("notion", "notion"));
        assert!(contains_word("(notion)", "notion"));
        assert!(!contains_word("notional", "notion"));
        assert!(!contains_word("xnotion", "notion"));
        assert!(contains_word("notional notion", "notion"));
    }
}
