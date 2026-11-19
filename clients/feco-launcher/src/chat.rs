//! Tin nhắn trong F-Launcher (thay đổi chat-and-source-scan: spec launcher "Tin nhắn trong
//! F-Launcher", design C3 và C4).
//!
//! Mô-đun này chỉ có phần không cần giao diện, để kiểm thử được không cần mạng:
//!
//! - kiểu dữ liệu và hàm đọc JSON của `/v1/chat` ([`parse_conversations`], [`parse_messages`]...);
//! - gộp các trang tin `after` vào luồng tin ([`Thread`]);
//! - lịch hỏi máy chủ ([`Schedule`]): tin mới mỗi 5 giây khi đang mở hội thoại, số chưa đọc mỗi
//!   60 giây khi đã đăng nhập;
//! - mã lỗi của máy chủ thành câu dễ hiểu ([`error_message`]).
//!
//! Lời gọi HTTP nằm trong [`crate::auth::Auth`] (`chat_unread`, `chat_conversations`...).

use crate::auth::{web_link, AuthError};
use crate::i18n::t;
use serde_json::Value;
use std::time::{Duration, Instant};

/// Hỏi tin mới mỗi 5 giây khi đang mở một hội thoại (design C3).
pub const POLL_INTERVAL: Duration = Duration::from_secs(5);
/// Cập nhật số chưa đọc mỗi 60 giây khi đã đăng nhập (design C3).
pub const UNREAD_INTERVAL: Duration = Duration::from_secs(60);
/// Số tin tối đa mỗi trang `GET …/messages` (design C4).
pub const PAGE_LIMIT: usize = 200;
/// Độ dài tối đa của tin (ký tự, sau khi bỏ khoảng trắng hai đầu; design C2).
pub const MAX_BODY_CHARS: usize = 5000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Direct,
    Project,
}

/// Phía của người đang đăng nhập trong hội thoại.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Customer,
    Developer,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Conversation {
    pub id: String,
    pub kind: Kind,
    pub role: Role,
    pub peer_name: String,
    pub title: String,
    pub project_id: Option<String>,
    pub app_id: Option<String>,
    pub blocked: bool,
    pub last_message: Option<String>,
    pub last_message_at: String,
    pub unread: usize,
    /// Trang hội thoại trên web (gửi tệp đính kèm chỉ làm được ở đó).
    pub web_url: Option<String>,
}

impl Conversation {
    /// Người đang đăng nhập không gửi được tin: khách hàng của hội thoại trực tiếp đã bị chặn.
    pub fn send_blocked(&self) -> bool {
        self.blocked && self.kind == Kind::Direct && self.role == Role::Customer
    }
}

/// Phía gửi tin.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    Customer,
    Developer,
    Admin,
    System,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScanStatus {
    /// Đang quét mã độc: chưa tải được.
    Pending,
    Clean,
    /// Có mã độc: chặn tải.
    Infected,
    /// Không quét được: chưa tải được.
    Error,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Attachment {
    pub name: String,
    pub size: u64,
    pub status: ScanStatus,
    /// Địa chỉ tải trên web (chỉ `https://`, hoặc máy dev).
    pub url: Option<String>,
}

impl Attachment {
    /// Địa chỉ mở bằng trình duyệt: chỉ khi tệp đã quét sạch.
    pub fn open_url(&self) -> Option<&str> {
        match self.status {
            ScanStatus::Clean => self.url.as_deref(),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Message {
    pub id: i64,
    pub side: Side,
    /// Tin của người đang đăng nhập.
    pub mine: bool,
    pub body: String,
    pub at: String,
    pub attachment: Option<Attachment>,
}

/// Mã hội thoại dùng trong đường dẫn URL: chỉ chữ, số, `-` và `_` (UUID của máy chủ).
pub fn safe_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn opt_str(v: &Value) -> Option<String> {
    v.as_str().map(str::to_owned)
}

fn link(v: &Value) -> Option<String> {
    v.as_str().filter(|u| web_link(u)).map(str::to_owned)
}

/// `{"unread": 3}` của `GET /v1/chat/unread`.
pub fn parse_unread(body: &Value) -> usize {
    body["unread"]
        .as_u64()
        .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX))
}

/// Một `Conversation` (design C4); mục sai dạng trả `None`.
pub fn parse_conversation(c: &Value) -> Option<Conversation> {
    let id = c["id"].as_str().filter(|id| safe_id(id))?.to_owned();
    let kind = match c["kind"].as_str()? {
        "direct" => Kind::Direct,
        "project" => Kind::Project,
        _ => return None,
    };
    let role = match c["role"].as_str()? {
        "customer" => Role::Customer,
        "developer" => Role::Developer,
        _ => return None,
    };
    let peer_name = c["peer_name"].as_str().unwrap_or_default().to_owned();
    let title = c["title"]
        .as_str()
        .filter(|s| !s.is_empty())
        .map_or_else(|| peer_name.clone(), str::to_owned);
    Some(Conversation {
        id,
        kind,
        role,
        peer_name,
        title,
        project_id: opt_str(&c["project_id"]),
        app_id: opt_str(&c["app_id"]),
        blocked: c["blocked"].as_bool().unwrap_or(false),
        last_message: opt_str(&c["last_message"]),
        last_message_at: c["last_message_at"].as_str().unwrap_or_default().to_owned(),
        unread: c["unread"]
            .as_u64()
            .map_or(0, |n| usize::try_from(n).unwrap_or(usize::MAX)),
        web_url: link(&c["web_url"]),
    })
}

/// `{"conversations": [...]}` của `GET /v1/chat/conversations` (mới nhất trước, như máy chủ trả).
pub fn parse_conversations(body: &Value) -> Vec<Conversation> {
    body["conversations"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_conversation)
        .collect()
}

fn parse_attachment(a: &Value) -> Option<Attachment> {
    let name = a["name"].as_str()?.to_owned();
    let status = match a["status"].as_str() {
        Some("pending") => ScanStatus::Pending,
        Some("clean") => ScanStatus::Clean,
        Some("infected") => ScanStatus::Infected,
        // Trạng thái lạ: coi như chưa kiểm tra được, không cho tải.
        _ => ScanStatus::Error,
    };
    Some(Attachment {
        name,
        size: a["size"].as_u64().unwrap_or(0),
        status,
        url: link(&a["url"]),
    })
}

/// Một `Message` (design C4); mục sai dạng trả `None`.
pub fn parse_message(m: &Value) -> Option<Message> {
    let id = m["id"].as_i64().filter(|id| *id > 0)?;
    let side = match m["side"].as_str()? {
        "customer" => Side::Customer,
        "developer" => Side::Developer,
        "admin" => Side::Admin,
        "system" => Side::System,
        _ => return None,
    };
    Some(Message {
        id,
        side,
        mine: m["mine"].as_bool().unwrap_or(false),
        body: m["body"].as_str().unwrap_or_default().to_owned(),
        at: m["at"].as_str().unwrap_or_default().to_owned(),
        attachment: parse_attachment(&m["attachment"]),
    })
}

/// `{"messages": [...]}` của `GET …/messages` (cũ trước).
pub fn parse_messages(body: &Value) -> Vec<Message> {
    body["messages"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(parse_message)
        .collect()
}

/// Gộp `new` vào `messages`: không trùng (tin cùng `id` lấy bản mới, ví dụ trạng thái quét tệp
/// đính kèm đã đổi), xếp theo `id` tăng dần. Trả số tin mới thêm vào.
pub fn merge_messages(messages: &mut Vec<Message>, new: Vec<Message>) -> usize {
    let mut added = 0;
    for m in new {
        match messages.binary_search_by_key(&m.id, |x| x.id) {
            Ok(i) => messages[i] = m,
            Err(i) => {
                messages.insert(i, m);
                added += 1;
            }
        }
    }
    added
}

/// Luồng tin của hội thoại đang mở.
#[derive(Clone, Debug, Default)]
pub struct Thread {
    pub conversation: String,
    pub messages: Vec<Message>,
    /// Đã tải xong lần đầu.
    pub loaded: bool,
    /// Tin cuối cùng đã nhận qua lần tải hoặc lần hỏi (mọi tin tới đây đã có). Tin vừa gửi
    /// không đẩy mốc này, để tin của bên kia gửi xen giữa không bị bỏ sót.
    pub synced: i64,
    /// Tin cuối cùng đã báo máy chủ là đã đọc.
    pub read_up_to: i64,
}

impl Thread {
    pub fn new(conversation: &str) -> Self {
        Thread {
            conversation: conversation.to_owned(),
            ..Default::default()
        }
    }

    pub fn last_id(&self) -> i64 {
        self.messages.last().map_or(0, |m| m.id)
    }

    /// `after` cho lần hỏi tiếp theo: tin cuối đã nhận, hoặc ngay trước tệp đính kèm cũ nhất còn
    /// đang quét (để thấy khi quét xong).
    pub fn poll_after(&self) -> i64 {
        let pending = self
            .messages
            .iter()
            .find(|m| {
                m.attachment
                    .as_ref()
                    .is_some_and(|a| a.status == ScanStatus::Pending)
            })
            .map(|m| m.id - 1);
        pending.map_or(self.synced, |p| p.min(self.synced))
    }

    /// Thêm các tin máy chủ trả cho lần tải hoặc lần hỏi (mọi tin sau `poll_after`). Trả số tin
    /// mới.
    pub fn apply(&mut self, page: Vec<Message>) -> usize {
        self.loaded = true;
        if let Some(max) = page.iter().map(|m| m.id).max() {
            self.synced = self.synced.max(max);
        }
        merge_messages(&mut self.messages, page)
    }

    /// Thêm tin vừa gửi (máy chủ trả về): hiện ngay ở cuối luồng.
    pub fn push_sent(&mut self, message: Message) {
        merge_messages(&mut self.messages, vec![message]);
    }

    /// Có tin chưa báo đã đọc: trả `up_to` cần gửi cho `POST …/read` (tới tin cuối đã nhận).
    pub fn read_due(&self) -> Option<i64> {
        (self.synced > self.read_up_to).then_some(self.synced)
    }
}

/// Kiểm tra trước khi gửi (máy chủ vẫn kiểm lại): trả nội dung đã bỏ khoảng trắng hai đầu, hoặc
/// mã lỗi `chat_empty` / `chat_too_long`.
pub fn validate_body(body: &str) -> Result<String, &'static str> {
    let body = body.trim();
    if body.is_empty() {
        Err("chat_empty")
    } else if body.chars().count() > MAX_BODY_CHARS {
        Err("chat_too_long")
    } else {
        Ok(body.to_owned())
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ChatError {
    #[error(transparent)]
    Auth(AuthError),
    /// Lỗi nghiệp vụ `{"error": "<mã>"}` của máy chủ.
    #[error("{code} ({status})")]
    Api { status: u16, code: String },
}

impl ChatError {
    /// Câu báo lỗi đã dịch.
    pub fn message(&self) -> String {
        match self {
            ChatError::Auth(AuthError::Expired) => {
                t("Phiên đăng nhập đã hết hạn, hãy đăng nhập lại.").to_owned()
            }
            ChatError::Auth(AuthError::Offline(_)) => {
                t("Không kết nối được máy chủ. Tin nhắn sẽ được tải lại khi có mạng.").to_owned()
            }
            ChatError::Auth(e) => e.to_string(),
            ChatError::Api { code, .. } => error_message(code).to_owned(),
        }
    }
}

/// Mã lỗi chat của máy chủ (design C2, C4) thành câu dễ hiểu đã dịch.
pub fn error_message(code: &str) -> &'static str {
    t(error_key(code))
}

/// Câu tiếng Việt gốc (khóa dịch) của mã lỗi chat.
pub fn error_key(code: &str) -> &'static str {
    match code {
        "chat_blocked" => "Nhà phát triển đã chặn hội thoại này. Bạn không gửi được tin.",
        "chat_too_long" => "Tin quá dài: tối đa 5.000 ký tự.",
        "chat_empty" => "Hãy nhập nội dung tin nhắn.",
        "chat_rate_limited" => "Bạn gửi quá nhanh. Hãy chờ một phút rồi gửi lại.",
        "chat_forbidden" => "Bạn không tham gia hội thoại này.",
        "chat_not_found" => "Không tìm thấy hội thoại.",
        _ => "Không gửi hoặc tải được tin nhắn. Hãy thử lại.",
    }
}

/// Mã lỗi có câu riêng (còn lại dùng câu chung).
pub const ERROR_CODES: [&str; 6] = [
    "chat_blocked",
    "chat_too_long",
    "chat_empty",
    "chat_rate_limited",
    "chat_forbidden",
    "chat_not_found",
];

/// Việc cần làm theo lịch.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Due {
    /// Hỏi số chưa đọc (và tải lại danh sách nếu đang mở trang Tin nhắn).
    Unread,
    /// Hỏi tin mới của hội thoại.
    Poll(String),
}

/// Lịch hỏi máy chủ của trang Tin nhắn (design C3). Không chạy chồng: lần hỏi trước chưa xong thì
/// chưa hỏi tiếp.
#[derive(Clone, Debug, Default)]
pub struct Schedule {
    open: Option<String>,
    next_poll: Option<Instant>,
    next_unread: Option<Instant>,
    polling: bool,
    fetching_unread: bool,
}

impl Schedule {
    /// Mở hội thoại `id` (hỏi ngay) hoặc đóng (`None`: thôi hỏi).
    pub fn set_open(&mut self, id: Option<&str>, now: Instant) {
        if self.open.as_deref() == id {
            return;
        }
        self.open = id.map(str::to_owned);
        self.next_poll = id.map(|_| now);
        // Kết quả của lần hỏi hội thoại cũ vẫn về, nhưng bị bỏ qua (khác hội thoại).
        self.polling = false;
    }

    pub fn open(&self) -> Option<&str> {
        self.open.as_deref()
    }

    /// Hỏi số chưa đọc ngay ở lần kiểm tra tới (sau khi đăng nhập, đồng bộ, đọc tin).
    pub fn refresh_unread_now(&mut self, now: Instant) {
        self.next_unread = Some(now);
    }

    /// Việc tới hạn lúc `now`; đánh dấu đang chạy. `signed_in = false` thì không hỏi gì.
    pub fn due(&mut self, now: Instant, signed_in: bool) -> Vec<Due> {
        let mut out = Vec::new();
        if !signed_in {
            return out;
        }
        if !self.fetching_unread && self.next_unread.is_none_or(|t| now >= t) {
            self.fetching_unread = true;
            out.push(Due::Unread);
        }
        if let (Some(id), Some(at)) = (&self.open, self.next_poll) {
            if !self.polling && now >= at {
                self.polling = true;
                out.push(Due::Poll(id.clone()));
            }
        }
        out
    }

    /// Lần hỏi số chưa đọc đã xong (thành công hay không).
    pub fn unread_done(&mut self, now: Instant) {
        self.fetching_unread = false;
        self.next_unread = Some(now + UNREAD_INTERVAL);
    }

    /// Lần hỏi tin mới của `id` đã xong.
    pub fn poll_done(&mut self, id: &str, now: Instant) {
        if self.open.as_deref() == Some(id) {
            self.polling = false;
            self.next_poll = Some(now + POLL_INTERVAL);
        }
    }

    /// Thời gian tới việc kế tiếp (để hẹn vẽ lại giao diện).
    pub fn wake_in(&self, now: Instant, signed_in: bool) -> Option<Duration> {
        if !signed_in {
            return None;
        }
        [self.next_unread, self.open.as_ref().and(self.next_poll)]
            .into_iter()
            .flatten()
            .map(|t| t.saturating_duration_since(now))
            .min()
    }
}

/// Dung lượng tệp: "820 B", "1,2 KB", "3,4 MB" (dấu thập phân theo ngôn ngữ).
pub fn file_size(bytes: u64) -> String {
    file_size_in(crate::i18n::language(), bytes)
}

pub fn file_size_in(lang: crate::i18n::Language, bytes: u64) -> String {
    let (value, unit) = if bytes < 1024 {
        return format!("{bytes} B");
    } else if bytes < 1024 * 1024 {
        (bytes as f64 / 1024.0, "KB")
    } else {
        (bytes as f64 / 1_048_576.0, "MB")
    };
    let text = format!("{value:.1}");
    let text = if matches!(lang, crate::i18n::Language::Vi | crate::i18n::Language::Fr) {
        text.replace('.', ",")
    } else {
        text
    };
    format!("{text} {unit}")
}

/// Nhãn trạng thái quét của tệp đính kèm.
pub fn scan_label(status: ScanStatus) -> &'static str {
    match status {
        ScanStatus::Pending => t("Đang kiểm tra"),
        ScanStatus::Clean => t("Đã quét, an toàn"),
        ScanStatus::Infected => t("Có mã độc, đã chặn tải"),
        ScanStatus::Error => t("Chưa kiểm tra được, chưa tải được"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn msg(id: i64, mine: bool, body: &str) -> Message {
        Message {
            id,
            side: if mine {
                Side::Customer
            } else {
                Side::Developer
            },
            mine,
            body: body.into(),
            at: "2026-11-19T03:00:00Z".into(),
            attachment: None,
        }
    }

    /// JSON của C4: hội thoại; mục sai dạng hoặc mã không an toàn cho URL bị bỏ.
    #[test]
    fn conversations_follow_the_contract() {
        let body = json!({"conversations": [
            {"id": "6f1c2a9e-0000-4000-8000-000000000001", "kind": "project", "role": "customer",
             "peer_name": "Công ty Phần mềm A", "title": "Ứng dụng quản lý kho",
             "project_id": "6f1c2a9e-0000-4000-8000-0000000000aa", "app_id": null, "blocked": false,
             "last_message": "Em gửi bản sửa nhé", "last_message_at": "2026-11-19T03:00:00Z",
             "unread": 2, "web_url": "https://f-eco.vn/messages/6f1c2a9e-0000-4000-8000-000000000001"},
            {"id": "c2", "kind": "direct", "role": "customer", "peer_name": "Lan", "title": "",
             "project_id": null, "app_id": "mentor", "blocked": true, "last_message": null,
             "last_message_at": "2026-11-18T03:00:00Z", "unread": 0, "web_url": "javascript:alert(1)"},
            {"id": "../admin", "kind": "direct", "role": "customer"},
            {"id": "c4", "kind": "group", "role": "customer"},
            {"id": "c5", "kind": "direct", "role": "admin"},
            "rác"
        ]});
        let list = parse_conversations(&body);
        assert_eq!(list.len(), 2);
        assert_eq!(list[0].kind, Kind::Project);
        assert_eq!(list[0].unread, 2);
        assert_eq!(list[0].title, "Ứng dụng quản lý kho");
        assert!(list[0].web_url.as_deref().unwrap().starts_with("https://"));
        assert_eq!(
            list[1].title, "Lan",
            "không có tiêu đề thì dùng tên bên kia"
        );
        assert_eq!(list[1].app_id.as_deref(), Some("mentor"));
        assert_eq!(list[1].web_url, None, "chỉ mở liên kết web");
        assert!(list[1].send_blocked());
        assert!(!list[0].send_blocked());
        assert_eq!(parse_unread(&json!({"unread": 3})), 3);
        assert_eq!(parse_unread(&json!({"unread": -1})), 0);
        assert_eq!(parse_unread(&json!({})), 0);
    }

    /// JSON của C4: tin và tệp đính kèm; chỉ tệp sạch mới mở được.
    #[test]
    fn messages_and_attachments_follow_the_contract() {
        let body = json!({"messages": [
            {"id": 40, "side": "system", "mine": false, "body": "Dự án đã bắt đầu", "at": "2026-11-19T01:00:00Z", "attachment": null},
            {"id": 41, "side": "developer", "mine": false, "body": "bao-gia.pdf", "at": "2026-11-19T02:00:00Z",
             "attachment": {"name": "bao-gia.pdf", "size": 1234, "status": "clean", "url": "https://f-eco.vn/messages/c/files/a"}},
            {"id": 42, "side": "admin", "mine": false, "body": "Quản trị đã giải quyết", "at": "2026-11-19T02:30:00Z",
             "attachment": {"name": "x.exe", "size": 99, "status": "infected", "url": "https://f-eco.vn/messages/c/files/b"}},
            {"id": 43, "side": "customer", "mine": true, "body": "Cảm ơn", "at": "2026-11-19T03:00:00Z",
             "attachment": {"name": "a.zip", "size": 5, "status": "lạ", "url": "https://f-eco.vn/f"}},
            {"id": 0, "side": "customer", "body": "id sai"},
            {"id": 44, "side": "robot", "body": "phía lạ"}
        ]});
        let m = parse_messages(&body);
        assert_eq!(m.iter().map(|m| m.id).collect::<Vec<_>>(), [40, 41, 42, 43]);
        assert_eq!(m[0].side, Side::System);
        assert_eq!(m[2].side, Side::Admin);
        assert!(m[3].mine);
        let clean = m[1].attachment.as_ref().unwrap();
        assert_eq!(
            clean.open_url(),
            Some("https://f-eco.vn/messages/c/files/a")
        );
        let infected = m[2].attachment.as_ref().unwrap();
        assert_eq!(infected.status, ScanStatus::Infected);
        assert_eq!(infected.open_url(), None, "tệp có mã độc không tải được");
        assert_eq!(m[3].attachment.as_ref().unwrap().status, ScanStatus::Error);
        assert_eq!(m[3].attachment.as_ref().unwrap().open_url(), None);
    }

    /// Các trang `after` gộp vào luồng tin: không trùng, đúng thứ tự, bản mới thay bản cũ.
    #[test]
    fn after_pages_merge_in_order_without_duplicates() {
        let mut thread = Thread::new("c1");
        assert_eq!(thread.apply(vec![msg(1, false, "a"), msg(2, true, "b")]), 2);
        // Tin vừa gửi (POST trả về) hiện ngay; bên kia gửi xen giữa (tin 3) vẫn được lần hỏi
        // 5 giây lấy về vì mốc hỏi vẫn là tin 2; lần hỏi trả lại cả tin vừa gửi mà không trùng.
        thread.push_sent(msg(4, true, "Em gửi bản sửa nhé"));
        assert_eq!(thread.messages.last().unwrap().body, "Em gửi bản sửa nhé");
        assert_eq!(thread.poll_after(), 2);
        assert_eq!(thread.read_due(), Some(2));
        assert_eq!(
            thread.apply(vec![
                msg(3, false, "c"),
                msg(4, true, "Em gửi bản sửa nhé"),
                msg(5, false, "d")
            ]),
            2
        );
        let ids: Vec<i64> = thread.messages.iter().map(|m| m.id).collect();
        assert_eq!(ids, [1, 2, 3, 4, 5]);
        assert_eq!(thread.last_id(), 5);
        assert_eq!(thread.read_due(), Some(5));
        thread.read_up_to = 5;
        assert_eq!(thread.read_due(), None);

        // Tệp đang quét: hỏi lại từ trước tin đó để thấy trạng thái mới.
        let mut pending = msg(6, false, "a.pdf");
        pending.attachment = Some(Attachment {
            name: "a.pdf".into(),
            size: 1,
            status: ScanStatus::Pending,
            url: None,
        });
        thread.apply(vec![pending.clone(), msg(7, false, "e")]);
        assert_eq!(thread.poll_after(), 5);
        pending.attachment.as_mut().unwrap().status = ScanStatus::Clean;
        assert_eq!(thread.apply(vec![pending, msg(7, false, "e")]), 0);
        assert_eq!(thread.poll_after(), 7);
        assert_eq!(thread.messages.len(), 7);
    }

    /// Mã lỗi của máy chủ thành câu dễ hiểu, có đủ bốn thứ tiếng (không phụ thuộc ngôn ngữ
    /// đang chọn, vì test khác có thể đổi ngôn ngữ cùng lúc).
    #[test]
    fn error_codes_have_friendly_messages() {
        use crate::i18n::{translate, Language};
        let generic = error_key("unknown");
        let mut seen = std::collections::HashSet::new();
        for code in ERROR_CODES.into_iter().chain(["unknown"]) {
            let key = error_key(code);
            assert!(seen.insert(key), "{code} trùng câu");
            assert!(!key.contains("chat_"), "{code}: {key}");
            for lang in [Language::En, Language::Fr, Language::Ja] {
                assert_ne!(translate(lang, key), key, "{code} chưa dịch {lang:?}");
            }
        }
        assert!(!ERROR_CODES.iter().any(|c| error_key(c) == generic));
        assert!(error_key("chat_blocked").contains("chặn"));
        assert!(error_key("chat_too_long").contains("5.000"));
        assert_ne!(
            ChatError::Auth(AuthError::Expired).message(),
            ChatError::Auth(AuthError::Offline("x".into())).message()
        );
    }

    /// Kiểm tra trước khi gửi: tin rỗng, quá 5.000 ký tự (đếm ký tự, không đếm byte).
    #[test]
    fn body_is_checked_before_sending() {
        assert_eq!(validate_body("   \n "), Err("chat_empty"));
        assert_eq!(validate_body("  Chào anh \n").as_deref(), Ok("Chào anh"));
        assert!(validate_body(&"ệ".repeat(5000)).is_ok());
        assert_eq!(validate_body(&"ệ".repeat(5001)), Err("chat_too_long"));
    }

    /// Lịch hỏi: số chưa đọc ngay khi đăng nhập rồi mỗi 60 giây; tin mới mỗi 5 giây khi đang mở
    /// hội thoại; không chạy chồng; đóng hội thoại thì thôi hỏi; chưa đăng nhập thì không hỏi.
    #[test]
    fn polling_schedule() {
        let t0 = Instant::now();
        let mut s = Schedule::default();
        assert!(s.due(t0, false).is_empty(), "chưa đăng nhập");
        assert_eq!(s.wake_in(t0, false), None);
        assert_eq!(s.due(t0, true), [Due::Unread]);
        assert!(s.due(t0, true).is_empty(), "đang hỏi thì không hỏi chồng");
        s.unread_done(t0);
        assert!(s.due(t0 + Duration::from_secs(59), true).is_empty());
        assert_eq!(s.wake_in(t0, true), Some(UNREAD_INTERVAL));

        s.set_open(Some("c1"), t0 + Duration::from_secs(10));
        assert_eq!(
            s.due(t0 + Duration::from_secs(10), true),
            [Due::Poll("c1".into())]
        );
        assert!(s.due(t0 + Duration::from_secs(16), true).is_empty());
        s.poll_done("c1", t0 + Duration::from_secs(11));
        assert_eq!(
            s.wake_in(t0 + Duration::from_secs(11), true),
            Some(POLL_INTERVAL)
        );
        assert!(s.due(t0 + Duration::from_secs(15), true).is_empty());
        assert_eq!(
            s.due(t0 + Duration::from_secs(16), true),
            [Due::Poll("c1".into())]
        );
        // Chuyển sang hội thoại khác: kết quả cũ không làm lệch lịch, hội thoại mới hỏi ngay.
        s.set_open(Some("c2"), t0 + Duration::from_secs(17));
        s.poll_done("c1", t0 + Duration::from_secs(17));
        assert_eq!(
            s.due(t0 + Duration::from_secs(17), true),
            [Due::Poll("c2".into())]
        );
        s.poll_done("c2", t0 + Duration::from_secs(18));
        assert_eq!(
            s.due(t0 + Duration::from_secs(60), true),
            [Due::Unread, Due::Poll("c2".into())]
        );
        s.unread_done(t0 + Duration::from_secs(60));
        s.set_open(None, t0 + Duration::from_secs(61));
        assert!(s.due(t0 + Duration::from_secs(100), true).is_empty());
        s.refresh_unread_now(t0 + Duration::from_secs(101));
        assert_eq!(s.due(t0 + Duration::from_secs(101), true), [Due::Unread]);
    }

    #[test]
    fn file_sizes() {
        use crate::i18n::Language;
        assert_eq!(file_size_in(Language::En, 820), "820 B");
        assert_eq!(file_size_in(Language::En, 1536), "1.5 KB");
        assert_eq!(file_size_in(Language::Vi, 1536), "1,5 KB");
        assert_eq!(file_size_in(Language::Ja, 3 * 1_048_576), "3.0 MB");
    }
}
