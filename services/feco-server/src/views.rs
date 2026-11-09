//! Chuyển một luồng Kratos (JSON) thành dữ liệu hiển thị, và dịch thông báo sang
//! tiếng Việt theo mã thông báo của Kratos. Thông báo chưa có bản dịch giữ nguyên
//! văn bản gốc, để không bao giờ mất thông tin.

use serde_json::Value;

use crate::kratos::FlowKind;

/// Mã thông báo của Kratos dùng trong F-Eco.
pub mod ids {
    pub const CREDENTIALS_INVALID: i64 = 4000006;
    pub const DUPLICATE_IDENTIFIER: i64 = 4000007;
    pub const PASSWORD_TOO_SHORT: i64 = 4000032;
    pub const PASSWORD_BREACHED: i64 = 4000034;
}

/// Thông báo hiển thị cho người dùng.
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
}

#[derive(Debug, Clone)]
pub struct FlowView {
    pub kind: FlowKind,
    pub title: String,
    pub form_action: String,
    pub messages: Vec<Msg>,
    pub hidden: Vec<(String, String)>,
    pub fields: Vec<Field>,
    pub buttons: Vec<Button>,
    /// Ảnh (ví dụ mã QR của TOTP) dưới dạng data URI.
    pub images: Vec<String>,
    /// Đoạn chữ Kratos muốn hiện (ví dụ khóa bí mật TOTP).
    pub texts: Vec<String>,
}

pub fn title(kind: FlowKind) -> &'static str {
    match kind {
        FlowKind::Login => "Đăng nhập",
        FlowKind::Registration => "Đăng ký tài khoản F-Eco",
        FlowKind::Verification => "Xác minh email",
        FlowKind::Recovery => "Khôi phục tài khoản",
        FlowKind::Settings => "Cài đặt tài khoản",
    }
}

/// Dịch một thông báo của Kratos. `text` là bản gốc tiếng Anh, dùng khi chưa có bản dịch.
pub fn translate(id: i64, text: &str) -> String {
    let vi = match id {
        // Nhãn
        1010001 => "Đăng nhập",
        1040001 => "Đăng ký",
        1070001 => "Mật khẩu",
        1070003 => "Lưu",
        1070004 => "Email",
        1070005 => "Gửi",
        1070006 => "Email",
        1070007 => "Email",
        1070009 => "Tiếp tục",
        1070011 => "Gửi lại mã",
        1070013 => "Mã",
        // Thông tin
        1050001 => "Đã lưu thay đổi.",
        1060001 => "Bạn đã khôi phục tài khoản. Hãy đặt mật khẩu mới trong vòng 15 phút.",
        1060003 => "Nếu email này có tài khoản F-Eco, chúng tôi đã gửi mã khôi phục tới đó. Mã có hạn 30 phút.",
        1080001 => "Chúng tôi đã gửi email chứa liên kết xác minh. Liên kết có hạn 24 giờ.",
        1080002 => "Bạn đã xác minh email thành công.",
        1080003 => "Chúng tôi đã gửi email chứa liên kết xác minh. Liên kết có hạn 24 giờ.",
        // Lỗi
        ids::CREDENTIALS_INVALID => "Email hoặc mật khẩu không đúng.",
        ids::PASSWORD_BREACHED => "Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu. Hãy chọn mật khẩu khác.",
        ids::PASSWORD_TOO_SHORT => "Mật khẩu phải dài ít nhất 10 ký tự.",
        4000002 => "Trường này là bắt buộc.",
        4000031 => "Mật khẩu quá giống email. Hãy chọn mật khẩu khác.",
        4060004 => "Liên kết hoặc mã khôi phục không hợp lệ hoặc đã được dùng.",
        4060005 => "Phiên khôi phục đã hết hạn. Hãy yêu cầu mã mới.",
        4070005 => "Liên kết xác minh không hợp lệ hoặc đã được dùng.",
        4070006 => "Mã xác minh không hợp lệ hoặc đã được dùng.",
        _ => return text.to_owned(),
    };
    vi.to_owned()
}

fn label_of(node: &Value) -> String {
    let label = &node["meta"]["label"];
    let text = label["text"].as_str().unwrap_or_default();
    // Nhãn 1070002 là tiêu đề lấy từ schema danh tính (đã viết bằng tiếng Việt).
    match label["id"].as_i64() {
        Some(1070002) | None => text.to_owned(),
        Some(id) => translate(id, text),
    }
}

fn messages_of(v: &Value) -> Vec<Msg> {
    v.as_array()
        .into_iter()
        .flatten()
        .map(|m| Msg {
            kind: m["type"].as_str().unwrap_or("info").to_owned(),
            text: translate(
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

/// Dựng dữ liệu hiển thị từ một luồng. Biểu mẫu luôn gửi về chính feco-server
/// (không gửi thẳng tới Kratos), để các bước của F-Eco luôn được áp dụng.
pub fn flow_view(kind: FlowKind, flow: &Value) -> FlowView {
    let id = flow["id"].as_str().unwrap_or_default();
    let ui = &flow["ui"];
    let mut view = FlowView {
        kind,
        title: title(kind).to_owned(),
        form_action: format!("/{}?flow={}", kind.path(), urlencode(id)),
        messages: messages_of(&ui["messages"]),
        hidden: vec![],
        fields: vec![],
        buttons: vec![],
        images: vec![],
        texts: vec![],
    };
    for node in ui["nodes"].as_array().into_iter().flatten() {
        let a = &node["attributes"];
        match node["type"].as_str() {
            Some("input") => {
                let name = a["name"].as_str().unwrap_or_default().to_owned();
                let input_type = a["type"].as_str().unwrap_or("text").to_owned();
                let value = value_string(&a["value"]);
                match input_type.as_str() {
                    "hidden" => view.hidden.push((name, value)),
                    "submit" | "button" => view.buttons.push(Button {
                        name,
                        value,
                        label: label_of(node),
                    }),
                    _ => {
                        let options = if name == "traits.locale" {
                            [("vi", "Tiếng Việt"), ("en", "English")]
                                .into_iter()
                                .map(|(v, l)| SelectOption {
                                    value: v.into(),
                                    label: l.into(),
                                    selected: value == v || (value.is_empty() && v == "vi"),
                                })
                                .collect()
                        } else {
                            vec![]
                        };
                        view.fields.push(Field {
                            label: label_of(node),
                            required: a["required"].as_bool().unwrap_or(false),
                            autocomplete: a["autocomplete"].as_str().unwrap_or_default().to_owned(),
                            messages: messages_of(&node["messages"]),
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
                    view.images.push(src.to_owned());
                }
            }
            Some("text") => {
                let t = &a["text"];
                let text = translate(
                    t["id"].as_i64().unwrap_or_default(),
                    t["text"].as_str().unwrap_or_default(),
                );
                if !text.is_empty() {
                    view.texts.push(text);
                }
            }
            // Nút passkey (script) được thêm ở task 2.4.
            _ => {}
        }
    }
    view
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
                "action": "http://127.0.0.1:4433/self-service/registration?flow=abc-123",
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
        let v = flow_view(FlowKind::Registration, &registration_flow_with(json!([])));
        assert_eq!(v.form_action, "/registration?flow=abc-123");
        assert_eq!(
            v.hidden,
            vec![("csrf_token".to_string(), "tok".to_string())]
        );
    }

    #[test]
    fn labels_and_buttons_are_vietnamese() {
        let v = flow_view(FlowKind::Registration, &registration_flow_with(json!([])));
        let labels: Vec<&str> = v.fields.iter().map(|f| f.label.as_str()).collect();
        assert_eq!(labels, ["Email", "Mật khẩu", "Ngôn ngữ"]);
        assert_eq!(v.buttons[0].label, "Đăng ký");
        assert_eq!(v.buttons[0].name, "method");
        assert_eq!(v.buttons[0].value, "password");
    }

    #[test]
    fn locale_is_a_select_defaulting_to_vietnamese() {
        let v = flow_view(FlowKind::Registration, &registration_flow_with(json!([])));
        let locale = v.fields.iter().find(|f| f.name == "traits.locale").unwrap();
        assert_eq!(locale.options.len(), 2);
        assert!(locale.options[0].selected && locale.options[0].value == "vi");
    }

    /// Spec account-identity, scenario "Mật khẩu đã bị lộ".
    #[test]
    fn breached_password_message_matches_spec() {
        let flow = registration_flow_with(
            json!([{"id": 4000034, "type": "error", "text": "The password has been found in data breaches and must no longer be used."}]),
        );
        let v = flow_view(FlowKind::Registration, &flow);
        let pw = v.fields.iter().find(|f| f.name == "password").unwrap();
        assert_eq!(pw.messages[0].kind, "error");
        assert!(pw.messages[0]
            .text
            .starts_with("Mật khẩu này đã xuất hiện trong các vụ rò rỉ dữ liệu"));
    }

    #[test]
    fn unknown_messages_keep_original_text() {
        assert_eq!(translate(9999999, "Something new"), "Something new");
    }

    #[test]
    fn flow_ids_are_url_encoded_in_action() {
        let flow = json!({"id": "a b&c", "ui": {"nodes": []}});
        assert_eq!(
            flow_view(FlowKind::Login, &flow).form_action,
            "/login?flow=a+b%26c"
        );
    }
}
