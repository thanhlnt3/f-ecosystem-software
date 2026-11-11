//! Questions F-Builder asks before building a project that was described in words only (tasks.md
//! 11.2, spec app-builder "Tạo từ mô tả"): which platform and which interface language, when the
//! request doesn't say. The answers are added to the request in words the stack and kind choice
//! already read ("website", "desktop app"), so the build follows them.

use f_builder_engine::workspace::{TargetRef, Workspace};

/// What one question asks about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Topic {
    Platform,
    Language,
}

/// One question: its card's title and choices (in the person's language), and what each choice
/// adds to the request (in English, the words the request is read by).
#[derive(Debug, Clone, PartialEq)]
pub struct Question {
    pub topic: Topic,
    pub title: String,
    pub choices: Vec<(String, &'static str)>,
}

/// Words that already name a platform, in the languages requests come in.
const PLATFORM_WORDS: &[&str] = &[
    "web",
    "website",
    "site",
    "trang web",
    "browser",
    "trình duyệt",
    "desktop",
    "máy tính",
    "windows",
    "macos",
    "mac ",
    "linux",
    "game",
    "trò chơi",
    "phone",
    "điện thoại",
    "android",
    "ios",
    "mobile",
    "di động",
    "console",
    "cli",
    "terminal",
    "dòng lệnh",
];

/// Words that already name the interface language.
const LANGUAGE_WORDS: &[&str] =
    &["tiếng việt", "vietnamese", "tiếng anh", "english", "song ngữ", "bilingual", "ngôn ngữ", "language", "đa ngôn ngữ", "multilingual"];

/// The questions to ask before the first build of `ws`, in `language` (the app's, e.g. "vi").
/// None when the project has something to go on besides its words: a target, the person's files
/// (an OpenSpec among them), a stack it was given, or a request that already says.
pub fn questions(ws: &Workspace, language: &str) -> Vec<Question> {
    let m = &ws.manifest;
    if m.target != TargetRef::None || m.app_in_root || !m.start_files.is_empty() || !m.stack.is_empty() {
        return Vec::new();
    }
    if !f_builder_engine::openspec::changes(&ws.root).is_empty() {
        return Vec::new();
    }
    let request = format!(" {} ", m.request.to_lowercase());
    let says = |words: &[&str]| words.iter().any(|w| request.contains(w));
    let vi = language.to_ascii_lowercase().starts_with("vi");
    let mut out = Vec::new();
    if !says(PLATFORM_WORDS) {
        out.push(Question {
            topic: Topic::Platform,
            title: if vi { "App chạy ở đâu?" } else { "Where will the app run?" }.into(),
            choices: vec![
                (if vi { "Ứng dụng máy tính (Windows, macOS)" } else { "A desktop app (Windows, macOS)" }.into(), "It is a desktop app for Windows and macOS."),
                (if vi { "Trang web, mở bằng trình duyệt" } else { "A website, opened in a browser" }.into(), "It is a website that runs in the browser."),
                (if vi { "Để F-Builder chọn" } else { "Let F-Builder choose" }.into(), ""),
            ],
        });
    }
    if !says(LANGUAGE_WORDS) {
        out.push(Question {
            topic: Topic::Language,
            title: if vi { "Giao diện của app dùng ngôn ngữ nào?" } else { "Which language should the app's interface use?" }.into(),
            choices: vec![
                (if vi { "Tiếng Việt" } else { "Vietnamese" }.into(), "Its interface is in Vietnamese."),
                (if vi { "Tiếng Anh" } else { "English" }.into(), "Its interface is in English."),
                (
                    if vi { "Cả hai, đổi được trong app" } else { "Both, switchable in the app" }.into(),
                    "Its interface is in Vietnamese and English, switchable in the app.",
                ),
            ],
        });
    }
    out
}

/// What an answer adds to the request: the chosen option's words, or what the person typed.
pub fn added(q: &Question, choice: Option<&str>, text: Option<&str>) -> String {
    if let Some(c) = choice
        && let Some((_, words)) = q.choices.iter().find(|(shown, _)| shown == c)
    {
        return (*words).to_string();
    }
    match (text.map(str::trim).filter(|t| !t.is_empty()), q.topic) {
        (Some(t), Topic::Platform) => format!("Platform: {t}."),
        (Some(t), Topic::Language) => format!("Interface language: {t}."),
        (None, _) => String::new(),
    }
}

/// The request with the answers added.
pub fn with_answers(request: &str, added: &[String]) -> String {
    let more: Vec<&str> = added.iter().map(|s| s.trim()).filter(|s| !s.is_empty()).collect();
    if more.is_empty() { request.to_string() } else { format!("{}\n\n{}", request.trim_end(), more.join(" ")) }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn project(request: &str) -> (tempfile::TempDir, Workspace) {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "p", request, TargetRef::None).unwrap();
        (d, ws)
    }

    /// Spec scenario "Tạo từ mô tả": the platform and the language are asked before building.
    #[test]
    fn a_request_in_words_only_is_asked_its_platform_and_language() {
        let (_d, ws) = project("Ứng dụng quản lý chi tiêu gia đình, có biểu đồ theo tháng");
        let qs = questions(&ws, "vi");
        assert_eq!(qs.iter().map(|q| q.topic).collect::<Vec<_>>(), [Topic::Platform, Topic::Language]);
        assert_eq!(qs[0].title, "App chạy ở đâu?");
        let platform = added(&qs[0], Some("Trang web, mở bằng trình duyệt"), None);
        let language = added(&qs[1], Some("Tiếng Việt"), None);
        let request = with_answers(&ws.manifest.request, &[platform, language]);
        assert!(request.starts_with("Ứng dụng quản lý chi tiêu gia đình"), "{request}");
        assert!(request.contains(" website ") && request.contains("Vietnamese"), "{request}");
        assert_eq!(questions(&ws, "en")[0].title, "Where will the app run?");
    }

    #[test]
    fn what_the_request_already_says_is_not_asked() {
        let (_d, ws) = project("A budgeting website in Vietnamese");
        assert!(questions(&ws, "vi").is_empty());
        let (_d, ws) = project("Ứng dụng máy tính quản lý kho");
        assert_eq!(questions(&ws, "vi").iter().map(|q| q.topic).collect::<Vec<_>>(), [Topic::Language]);
    }

    #[test]
    fn a_project_with_something_to_go_on_is_not_asked() {
        let d = tempfile::tempdir().unwrap();
        let ws = Workspace::create(d.path(), "p", "Copy it", TargetRef::Website { url: "https://example.com".into() }).unwrap();
        assert!(questions(&ws, "vi").is_empty(), "a reference");
        let (_d, mut ws) = project("Notes");
        ws.manifest.stack = "Python".into();
        assert!(questions(&ws, "vi").is_empty(), "a stack it was given");
        let (_d, ws) = project("Xây theo spec này");
        let spec = ws.root.join("openspec/changes/x");
        std::fs::create_dir_all(&spec).unwrap();
        std::fs::write(spec.join("proposal.md"), "# x").unwrap();
        assert!(questions(&ws, "vi").is_empty(), "an OpenSpec");
    }

    #[test]
    fn a_typed_answer_and_no_answer() {
        let (_d, ws) = project("Sổ ghi chép");
        let qs = questions(&ws, "vi");
        assert_eq!(added(&qs[0], None, Some("Chạy trên máy tính bảng")), "Platform: Chạy trên máy tính bảng.");
        assert_eq!(added(&qs[0], Some("Để F-Builder chọn"), None), "");
        assert_eq!(with_answers("Sổ ghi chép", &[String::new()]), "Sổ ghi chép", "nothing added when nothing was said");
    }
}
