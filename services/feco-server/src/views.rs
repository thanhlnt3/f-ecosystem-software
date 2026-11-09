//! Chuyển một luồng Kratos (JSON) thành dữ liệu hiển thị.
//!
//! - Mỗi nhóm phương thức (mật khẩu, TOTP, mã khôi phục, passkey...) là một biểu mẫu
//!   riêng; các nút thuộc nhóm `default` (token CSRF, định danh) có mặt trong mọi biểu mẫu.
//! - Thông báo được dịch sang tiếng Việt theo mã của Kratos. Ở giao diện tiếng Anh,
//!   văn bản gốc của Kratos (vốn là tiếng Anh) được giữ nguyên.

use serde_json::Value;

use crate::{i18n::Lang, kratos::FlowKind};

/// Mã thông báo của Kratos dùng trong F-Eco.
pub mod ids {
    pub const CREDENTIALS_INVALID: i64 = 4000006;
    pub const DUPLICATE_IDENTIFIER: i64 = 4000007;
    pub const PASSWORD_TOO_SHORT: i64 = 4000032;
    pub const PASSWORD_BREACHED: i64 = 4000034;
}

/// Thứ tự các nhóm trên trang.
const GROUP_ORDER: &[&str] = &[
    "password",
    "profile",
    "code",
    "totp",
    "lookup_secret",
    "webauthn",
    "oidc",
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Msg {
    /// `error`, `info` hoặc `success`.
    pub kind: String,
    pub text: String,
}

impl Msg {
    pub fn error(text: impl Into<String>) -> Self {
        Msg {
            kind: "error".into(),
            text: text.into(),
        }
    }
    pub fn info(text: impl Into<String>) -> Self {
        Msg {
            kind: "info".into(),
            text: text.into(),
        }
    }
    pub fn success(text: impl Into<String>) -> Self {
        Msg {
            kind: "success".into(),
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub selected: bool,
}

#[derive(Debug, Clone)]
pub struct Field {
    pub name: String,
    pub input_type: String,
    pub label: String,
    pub value: String,
    pub required: bool,
    pub autocomplete: String,
    pub messages: Vec<Msg>,
    /// Khác rỗng thì hiển thị dạng danh sách chọn.
    pub options: Vec<SelectOption>,
}

#[derive(Debug, Clone)]
pub struct Button {
    pub name: String,
    pub value: String,
    pub label: String,
    /// `submit`, hoặc `button` cho nút chạy JavaScript (passkey).
    pub button_type: String,
    /// Mã JavaScript Kratos cung cấp cho nút passkey.
    pub onclick: String,
}

#[derive(Debug, Clone)]
pub struct Script {
    pub src: String,
    pub id: String,
    pub integrity: String,
    pub crossorigin: String,
    pub referrerpolicy: String,
    pub is_async: bool,
}

/// Một đoạn chữ Kratos muốn hiện, ví dụ khóa bí mật TOTP hoặc danh sách mã khôi phục.
#[derive(Debug, Clone)]
pub struct TextNode {
    pub id: String,
    pub label: String,
    pub lines: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct FormGroup {
    pub group: String,
    pub title: String,
    pub action: String,
    pub hidden: Vec<(String, String)>,
    pub fields: Vec<Field>,
    pub buttons: Vec<Button>,
    pub images: Vec<String>,
    pub texts: Vec<TextNode>,
}

#[derive(Debug, Clone)]
pub struct FlowView {
    pub kind: FlowKind,
    pub title: String,
    pub messages: Vec<Msg>,
    pub forms: Vec<FormGroup>,
    pub scripts: Vec<Script>,
}

pub fn title(lang: Lang, kind: FlowKind, aal2: bool) -> &'static str {
    lang.t(match kind {
        FlowKind::Login if aal2 => "flow.login_aal2",
        FlowKind::Login => "flow.login",
        FlowKind::Registration => "flow.registration",
        FlowKind::Verification => "flow.verification",
        FlowKind::Recovery => "flow.recovery",
        FlowKind::Settings => "flow.settings",
    })
}

/// Dịch một thông báo của Kratos. Ở tiếng Anh hoặc khi chưa có bản dịch, giữ văn bản gốc.
pub fn translate(lang: Lang, id: i64, text: &str) -> String {
    if lang == Lang::En {
        return text.to_owned();
    }
    let vi = match id {
        // Nhãn
        1010001 => "Đăng nhập",
        1010022 => "Đăng nhập bằng mật khẩu",
        1040001 => "Đăng ký",
        1050005 => "Mã QR cho ứng dụng xác thực",
        1050017 => "Khóa bí mật (nếu không quét được mã QR)",
        1050008 => "Tạo mã khôi phục mới",
        1050009 => "Hiện mã khôi phục",
        1050010 => "Lưu các mã khôi phục này",
        1050011 => "Xác nhận đã lưu mã khôi phục",
        1050012 => "Thêm passkey",
        1050013 => "Tên passkey",
        1050014 => "Đã bật mã khôi phục",
        1050015 => "Các mã khôi phục của bạn (mỗi mã dùng một lần)",
        1050016 => "Tắt ứng dụng xác thực",
        1050018 => "Đã bật mã khôi phục",
        1070001 => "Mật khẩu",
        1070003 => "Lưu",
        1070004 => "Email",
        1070005 => "Gửi",
        1070006 => "Mã xác thực",
        1070007 => "Email",
        1070009 => "Tiếp tục",
        1070011 => "Gửi lại mã",
        1070013 => "Mã",
        1010006 => "Mã xác thực",
        1010009 => "Dùng mã xác thực",
        1010010 => "Dùng mã khôi phục",
        1010015 => "Mã khôi phục",
        // Thông tin
        1010004 => "Hãy hoàn tất bước xác thực thứ hai.",
        1050001 => "Đã lưu thay đổi.",
        1060001 => "Bạn đã khôi phục tài khoản. Hãy đặt mật khẩu mới trong vòng 15 phút.",
        1060003 => "Nếu email này có tài khoản F-Eco, chúng tôi đã gửi mã khôi phục tới đó. Mã có hạn 30 phút.",
        1080001 | 1080003 => "Chúng tôi đã gửi email chứa liên kết xác minh. Liên kết có hạn 24 giờ.",
        1080002 => "Bạn đã xác minh email thành công.",
        // Lỗi
        ids::CREDENTIALS_INVALID => "Email hoặc mật khẩu không đúng.",
        ids::PASSWORD_BREACHED => "Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu. Hãy chọn mật khẩu khác.",
        ids::PASSWORD_TOO_SHORT => "Mật khẩu phải dài ít nhất 10 ký tự.",
        4000002 => "Trường này là bắt buộc.",
        4000008 => "Mã xác thực không đúng.",
        4000016 => "Mã khôi phục không đúng.",
        4000031 => "Mật khẩu quá giống email. Hãy chọn mật khẩu khác.",
        4000012 => "Mã khôi phục này đã được dùng.",
        4060004 => "Liên kết hoặc mã khôi phục không hợp lệ hoặc đã được dùng.",
        4060005 => "Phiên khôi phục đã hết hạn. Hãy yêu cầu mã mới.",
        4070005 => "Liên kết xác minh không hợp lệ hoặc đã được dùng.",
        4070006 => "Mã xác minh không hợp lệ hoặc đã được dùng.",
        _ => return text.to_owned(),
    };
    vi.to_owned()
}

fn label_of(lang: Lang, node: &Value) -> String {
    let label = &node["meta"]["label"];
    let text = label["text"].as_str().unwrap_or_default();
    // Nhãn 1070002 là tiêu đề lấy từ schema danh tính.
    match label["id"].as_i64() {
        Some(1070002) | None => schema_label(lang, text),
        Some(id) => translate(lang, id, text),
    }
}

/// Tiêu đề trường trong schema danh tính được viết bằng tiếng Việt; dịch sang tiếng Anh.
fn schema_label(lang: Lang, text: &str) -> String {
    match (lang, text) {
        (Lang::En, "Tên hiển thị") => "Display name".into(),
        (Lang::En, "Ngôn ngữ") => "Language".into(),
        (Lang::En, "Ảnh đại diện (URL)") => "Profile picture (URL)".into(),
        (Lang::En, "Múi giờ") => "Time zone".into(),
        (Lang::En, "Số điện thoại") => "Phone number".into(),
        (Lang::En, "Tên trên hóa đơn") => "Billing name".into(),
        (Lang::En, "Mã số thuế") => "Tax code".into(),
        (Lang::En, "Địa chỉ xuất hóa đơn") => "Billing address".into(),
        _ => text.to_owned(),
    }
}

fn messages_of(lang: Lang, v: &Value) -> Vec<Msg> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|m| Msg {
            kind: m["type"].as_str().unwrap_or("info").to_owned(),
            text: translate(
                lang,
                m["id"].as_i64().unwrap_or_default(),
                m["text"].as_str().unwrap_or_default(),
            ),
        })
        .collect()
}

fn value_string(v: &Value) -> String {
    match v {
        Value::String(s) => s.clone(),
        Value::Null => String::new(),
        other => other.to_string(),
    }
}

fn group_title(lang: Lang, group: &str) -> String {
    match group {
        "password" | "profile" | "totp" | "lookup_secret" | "webauthn" | "code" | "oidc" => {
            lang.t(&format!("group.{group}")).to_owned()
        }
        _ => String::new(),
    }
}

/// Thêm một nút (không phải script) vào biểu mẫu.
fn add_node(lang: Lang, form: &mut FormGroup, node: &Value) {
    let a = &node["attributes"];
    match node["type"].as_str() {
        Some("input") => {
            let name = a["name"].as_str().unwrap_or_default().to_owned();
            let input_type = a["type"].as_str().unwrap_or("text").to_owned();
            let value = value_string(&a["value"]);
            match input_type.as_str() {
                "hidden" => form.hidden.push((name, value)),
                "submit" | "button" => form.buttons.push(Button {
                    label: label_of(lang, node),
                    button_type: if input_type == "button" {
                        "button".into()
                    } else {
                        "submit".into()
                    },
                    onclick: a["onclick"].as_str().unwrap_or_default().to_owned(),
                    name,
                    value,
                }),
                _ => {
                    let options = if name == "traits.locale" {
                        [("vi", "Tiếng Việt"), ("en", "English")]
                            .into_iter()
                            .map(|(v, l)| SelectOption {
                                value: v.into(),
                                label: l.into(),
                                selected: value == v || (value.is_empty() && v == lang.code()),
                            })
                            .collect()
                    } else {
                        vec![]
                    };
                    form.fields.push(Field {
                        label: label_of(lang, node),
                        required: a["required"].as_bool().unwrap_or(false),
                        autocomplete: a["autocomplete"].as_str().unwrap_or_default().to_owned(),
                        messages: messages_of(lang, &node["messages"]),
                        name,
                        input_type,
                        value,
                        options,
                    });
                }
            }
        }
        Some("img") => {
            if let Some(src) = a["src"].as_str() {
                form.images.push(src.to_owned());
            }
        }
        Some("text") => {
            let t = &a["text"];
            // Mã khôi phục nằm trong context.secrets, khóa TOTP trong context.secret:
            // đó là dữ liệu, không được dịch.
            let lines: Vec<String> = match (
                t["context"]["secrets"].as_array(),
                t["context"]["secret"].as_str(),
            ) {
                (Some(secrets), _) => secrets
                    .iter()
                    .filter_map(|s| s["text"].as_str().map(str::to_owned))
                    .collect(),
                (None, Some(secret)) => vec![secret.to_owned()],
                (None, None) => vec![translate(
                    lang,
                    t["id"].as_i64().unwrap_or_default(),
                    t["text"].as_str().unwrap_or_default(),
                )],
            };
            form.texts.push(TextNode {
                id: a["id"].as_str().unwrap_or_default().to_owned(),
                label: label_of(lang, node),
                lines,
            });
        }
        _ => {}
    }
}

/// Dựng dữ liệu hiển thị từ một luồng. Biểu mẫu luôn gửi về chính feco-server
/// (không gửi thẳng tới Kratos), để các bước của F-Eco luôn được áp dụng.
pub fn flow_view(lang: Lang, kind: FlowKind, flow: &Value) -> FlowView {
    let id = flow["id"].as_str().unwrap_or_default();
    let ui = &flow["ui"];
    let aal2 = flow["requested_aal"] == "aal2";
    let action = format!("/{}?flow={}", kind.path(), urlencode(id));
    let nodes: Vec<&Value> = ui["nodes"].as_array().into_iter().flatten().collect();

    let scripts = nodes
        .iter()
        .filter(|n| n["type"] == "script")
        .map(|n| {
            let a = &n["attributes"];
            let s = |k: &str| a[k].as_str().unwrap_or_default().to_owned();
            Script {
                src: s("src"),
                id: s("id"),
                integrity: s("integrity"),
                crossorigin: s("crossorigin"),
                referrerpolicy: s("referrerpolicy"),
                is_async: a["async"].as_bool().unwrap_or(false),
            }
        })
        .collect();

    let default_nodes: Vec<&&Value> = nodes.iter().filter(|n| n["group"] == "default").collect();
    let mut groups: Vec<String> = nodes
        .iter()
        .filter_map(|n| n["group"].as_str())
        .filter(|g| *g != "default")
        .map(str::to_owned)
        .collect();
    groups.sort_by_key(|g| {
        GROUP_ORDER
            .iter()
            .position(|o| o == g)
            .unwrap_or(usize::MAX)
    });
    groups.dedup();

    let mut forms = Vec::new();
    for group in groups {
        let mut form = FormGroup {
            title: group_title(lang, &group),
            group: group.clone(),
            action: action.clone(),
            hidden: vec![],
            fields: vec![],
            buttons: vec![],
            images: vec![],
            texts: vec![],
        };
        for n in &default_nodes {
            add_node(lang, &mut form, n);
        }
        for n in nodes
            .iter()
            .filter(|n| n["group"] == group.as_str() && n["type"] != "script")
            .filter(|n| kind != FlowKind::Registration || registration_field(n))
        {
            add_node(lang, &mut form, n);
        }
        forms.push(form);
    }

    FlowView {
        kind,
        title: title(lang, kind, aal2).to_owned(),
        messages: messages_of(lang, &ui["messages"]),
        forms,
        scripts,
    }
}

/// Trang đăng ký chỉ hỏi những gì cần để tạo tài khoản; phần hồ sơ còn lại (ảnh, số
/// điện thoại, thông tin hóa đơn) điền sau ở trang cài đặt.
fn registration_field(node: &Value) -> bool {
    match node["attributes"]["name"].as_str() {
        Some(name) if name.starts_with("traits.") => {
            matches!(name, "traits.email" | "traits.name" | "traits.locale")
        }
        _ => true,
    }
}

fn urlencode(s: &str) -> String {
    form_urlencoded::byte_serialize(s.as_bytes()).collect()
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn registration_flow_with(errors: Value) -> Value {
        json!({
            "id": "abc-123",
            "ui": {
                "messages": [],
                "nodes": [
                    {"type": "input", "group": "default", "attributes": {"name": "csrf_token", "type": "hidden", "value": "tok"}, "messages": [], "meta": {}},
                    {"type": "input", "group": "password", "attributes": {"name": "traits.email", "type": "email", "value": "lan@example.com", "required": true, "autocomplete": "email"}, "messages": [], "meta": {"label": {"id": 1070002, "text": "Email"}}},
                    {"type": "input", "group": "password", "attributes": {"name": "password", "type": "password", "required": true}, "messages": errors, "meta": {"label": {"id": 1070001, "text": "Password"}}},
                    {"type": "input", "group": "password", "attributes": {"name": "traits.locale", "type": "text"}, "messages": [], "meta": {"label": {"id": 1070002, "text": "Ngôn ngữ"}}},
                    {"type": "input", "group": "password", "attributes": {"name": "method", "type": "submit", "value": "password"}, "messages": [], "meta": {"label": {"id": 1040001, "text": "Sign up"}}}
                ]
            }
        })
    }

    #[test]
    fn form_posts_back_to_feco_not_kratos() {
        let v = flow_view(
            Lang::Vi,
            FlowKind::Registration,
            &registration_flow_with(json!([])),
        );
        assert_eq!(v.forms.len(), 1);
        assert_eq!(v.forms[0].action, "/registration?flow=abc-123");
        assert_eq!(
            v.forms[0].hidden,
            vec![("csrf_token".to_string(), "tok".to_string())]
        );
    }

    #[test]
    fn labels_and_buttons_follow_language() {
        let flow = registration_flow_with(json!([]));
        let vi = flow_view(Lang::Vi, FlowKind::Registration, &flow);
        let labels: Vec<&str> = vi.forms[0]
            .fields
            .iter()
            .map(|f| f.label.as_str())
            .collect();
        assert_eq!(labels, ["Email", "Mật khẩu", "Ngôn ngữ"]);
        assert_eq!(vi.forms[0].buttons[0].label, "Đăng ký");
        assert_eq!(vi.title, "Đăng ký tài khoản F-Eco");

        let en = flow_view(Lang::En, FlowKind::Registration, &flow);
        let labels: Vec<&str> = en.forms[0]
            .fields
            .iter()
            .map(|f| f.label.as_str())
            .collect();
        assert_eq!(labels, ["Email", "Password", "Language"]);
        assert_eq!(en.forms[0].buttons[0].label, "Sign up");
        assert_eq!(en.title, "Create your F-Eco account");
    }

    #[test]
    fn locale_select_defaults_to_ui_language() {
        let flow = registration_flow_with(json!([]));
        let pick = |lang| {
            flow_view(lang, FlowKind::Registration, &flow).forms[0]
                .fields
                .iter()
                .find(|f| f.name == "traits.locale")
                .unwrap()
                .options
                .iter()
                .find(|o| o.selected)
                .unwrap()
                .value
                .clone()
        };
        assert_eq!(pick(Lang::Vi), "vi");
        assert_eq!(pick(Lang::En), "en");
    }

    /// Spec account-identity, scenario "Mật khẩu đã bị lộ".
    #[test]
    fn breached_password_message_matches_spec() {
        let flow = registration_flow_with(
            json!([{"id": 4000034, "type": "error", "text": "The password has been found in data breaches and must no longer be used."}]),
        );
        let v = flow_view(Lang::Vi, FlowKind::Registration, &flow);
        let pw = v.forms[0]
            .fields
            .iter()
            .find(|f| f.name == "password")
            .unwrap();
        assert_eq!(pw.messages[0].kind, "error");
        assert!(pw.messages[0]
            .text
            .starts_with("Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu"));
    }

    #[test]
    fn unknown_messages_keep_original_text() {
        assert_eq!(
            translate(Lang::Vi, 9999999, "Something new"),
            "Something new"
        );
    }

    #[test]
    fn flow_ids_are_url_encoded_in_action() {
        let flow = json!({"id": "a b&c", "ui": {"nodes": [
            {"type": "input", "group": "password", "attributes": {"name": "method", "type": "submit", "value": "password"}}
        ]}});
        assert_eq!(
            flow_view(Lang::Vi, FlowKind::Login, &flow).forms[0].action,
            "/login?flow=a+b%26c"
        );
    }

    /// Trang cài đặt: mỗi nhóm một biểu mẫu, token CSRF có trong mọi biểu mẫu,
    /// mã QR, khóa TOTP, nút và script passkey đều được hiển thị.
    #[test]
    fn settings_groups_become_separate_forms() {
        let flow = json!({"id": "s1", "ui": {"nodes": [
            {"type": "input", "group": "default", "attributes": {"name": "csrf_token", "type": "hidden", "value": "tok"}},
            {"type": "input", "group": "profile", "attributes": {"name": "traits.email", "type": "email", "value": "a@b.c"}, "meta": {"label": {"id": 1070002, "text": "Email"}}},
            {"type": "input", "group": "profile", "attributes": {"name": "method", "type": "submit", "value": "profile"}, "meta": {"label": {"id": 1070003, "text": "Save"}}},
            {"type": "input", "group": "password", "attributes": {"name": "password", "type": "password"}, "meta": {"label": {"id": 1070001, "text": "Password"}}},
            {"type": "input", "group": "password", "attributes": {"name": "method", "type": "submit", "value": "password"}, "meta": {"label": {"id": 1070003, "text": "Save"}}},
            {"type": "img", "group": "totp", "attributes": {"id": "totp_qr", "src": "data:image/png;base64,AAAA"}},
            {"type": "text", "group": "totp", "attributes": {"id": "totp_secret_key", "text": {"id": 1050006, "text": "JBSWY3DPEHPK3PXP", "context": {"secret": "JBSWY3DPEHPK3PXP"}}}, "meta": {"label": {"id": 1050017, "text": "This is your authenticator app secret."}}},
            {"type": "input", "group": "totp", "attributes": {"name": "totp_code", "type": "text"}, "meta": {"label": {"id": 1070006, "text": "Verify code"}}},
            {"type": "input", "group": "totp", "attributes": {"name": "method", "type": "submit", "value": "totp"}, "meta": {"label": {"id": 1070003, "text": "Save"}}},
            {"type": "input", "group": "webauthn", "attributes": {"name": "webauthn_register_trigger", "type": "button", "value": "", "onclick": "window.__oryWebAuthnRegistration({})"}, "meta": {"label": {"id": 1050012, "text": "Add security key"}}},
            {"type": "script", "group": "webauthn", "attributes": {"src": "http://127.0.0.1:4433/.well-known/ory/webauthn.js", "id": "webauthn_script", "async": true, "integrity": "sha512-x", "crossorigin": "anonymous", "referrerpolicy": "no-referrer"}}
        ]}});
        let v = flow_view(Lang::Vi, FlowKind::Settings, &flow);
        let groups: Vec<&str> = v.forms.iter().map(|f| f.group.as_str()).collect();
        assert_eq!(groups, ["password", "profile", "totp", "webauthn"]);
        assert!(v
            .forms
            .iter()
            .all(|f| f.hidden.iter().any(|(k, _)| k == "csrf_token")));
        let totp = &v.forms[2];
        assert_eq!(totp.images, ["data:image/png;base64,AAAA"]);
        assert_eq!(totp.texts[0].id, "totp_secret_key");
        assert_eq!(
            totp.texts[0].lines,
            ["JBSWY3DPEHPK3PXP"],
            "khóa bí mật là dữ liệu, không được dịch"
        );
        assert_eq!(
            totp.texts[0].label,
            "Khóa bí mật (nếu không quét được mã QR)"
        );
        let pk = &v.forms[3].buttons[0];
        assert_eq!(pk.button_type, "button");
        assert!(pk.onclick.contains("oryWebAuthnRegistration"));
        assert_eq!(v.scripts.len(), 1);
        assert!(v.scripts[0].is_async);
    }

    #[test]
    fn lookup_secret_codes_are_listed() {
        let flow = json!({"id": "s1", "ui": {"nodes": [
            {"type": "text", "group": "lookup_secret", "attributes": {"id": "lookup_secret_codes", "text": {"id": 1050015, "text": "a, b", "context": {"secrets": [{"text": "abcd1234"}, {"text": "efgh5678"}]}}}}
        ]}});
        let v = flow_view(Lang::Vi, FlowKind::Settings, &flow);
        assert_eq!(v.forms[0].texts[0].lines, ["abcd1234", "efgh5678"]);
    }
}
