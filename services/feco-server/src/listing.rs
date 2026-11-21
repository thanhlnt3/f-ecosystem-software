//! Lọc, sắp xếp, tìm kiếm cho các danh sách của bảng quản trị (thay đổi admin-list-tools,
//! design L1).
//!
//! Tham số nằm trên địa chỉ trang (`GET`): `q` (tìm kiếm), `status`, `sort`, `from`, `to`.
//! [`apply`] lọc và sắp xếp các dòng đã đọc từ cơ sở dữ liệu; [`Toolbar`] là dữ liệu cho template
//! `admin_toolbar.html`.

use std::collections::HashMap;

use time::{Date, OffsetDateTime};

use crate::i18n::Lang;

/// Độ dài tối đa của chuỗi tìm kiếm.
const MAX_QUERY: usize = 200;

/// Điều kiện lọc của một danh sách.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    pub q: String,
    /// `""` (mặc định), `new`, `old`, `sender`, `sender_desc`.
    pub sort: String,
    /// `""` (mặc định), `all`, hoặc một mã trạng thái.
    pub status: String,
    pub from: Option<Date>,
    pub to: Option<Date>,
}

const SORTS: [&str; 4] = ["new", "old", "sender", "sender_desc"];

fn date(s: Option<&String>) -> Option<Date> {
    let s = s?.trim();
    Date::parse(
        s,
        &time::macros::format_description!("[year]-[month]-[day]"),
    )
    .ok()
}

impl ListQuery {
    pub fn parse(p: &HashMap<String, String>) -> ListQuery {
        let q: String = p
            .get("q")
            .map(|s| s.trim())
            .unwrap_or_default()
            .chars()
            .take(MAX_QUERY)
            .collect();
        let sort = p
            .get("sort")
            .filter(|s| SORTS.contains(&s.as_str()))
            .cloned()
            .unwrap_or_default();
        let status: String = p
            .get("status")
            .map(|s| s.trim())
            .unwrap_or_default()
            .chars()
            .take(40)
            .collect();
        ListQuery {
            q,
            sort,
            status,
            from: date(p.get("from")),
            to: date(p.get("to")),
        }
    }

    /// Có điều kiện nào khác mặc định không.
    pub fn active(&self) -> bool {
        !self.q.is_empty()
            || !self.sort.is_empty()
            || !self.status.is_empty()
            || self.from.is_some()
            || self.to.is_some()
    }
}

/// Thông tin của một dòng mà bộ lọc cần.
pub struct Item {
    /// Thời gian để lọc ngày và sắp xếp; `None`: dòng không có thời gian.
    pub at: Option<OffsetDateTime>,
    pub sender: String,
    /// Nội dung để tìm kiếm (ngoài người gửi).
    pub text: String,
    pub status: String,
}

/// Chữ thường, bỏ dấu tiếng Việt (`Đ` → `d`).
pub fn fold(s: &str) -> String {
    s.chars()
        .flat_map(char::to_lowercase)
        .map(|c| match c {
            'à' | 'á' | 'ả' | 'ã' | 'ạ' | 'ă' | 'ằ' | 'ắ' | 'ẳ' | 'ẵ' | 'ặ' | 'â' | 'ầ' | 'ấ'
            | 'ẩ' | 'ẫ' | 'ậ' => 'a',
            'è' | 'é' | 'ẻ' | 'ẽ' | 'ẹ' | 'ê' | 'ề' | 'ế' | 'ể' | 'ễ' | 'ệ' => {
                'e'
            }
            'ì' | 'í' | 'ỉ' | 'ĩ' | 'ị' => 'i',
            'ò' | 'ó' | 'ỏ' | 'õ' | 'ọ' | 'ô' | 'ồ' | 'ố' | 'ổ' | 'ỗ' | 'ộ' | 'ơ' | 'ờ' | 'ớ'
            | 'ở' | 'ỡ' | 'ợ' => 'o',
            'ù' | 'ú' | 'ủ' | 'ũ' | 'ụ' | 'ư' | 'ừ' | 'ứ' | 'ử' | 'ữ' | 'ự' => {
                'u'
            }
            'ỳ' | 'ý' | 'ỷ' | 'ỹ' | 'ỵ' => 'y',
            'đ' => 'd',
            c => c,
        })
        .collect()
}

/// Lọc và sắp xếp `rows` theo `q`. `hidden` là các trạng thái bị ẩn khi chưa lọc trạng thái
/// (ví dụ đơn đã xong). Khi `sort` là mặc định, giữ thứ tự sẵn có.
pub fn apply<T>(rows: Vec<T>, q: &ListQuery, hidden: &[&str], item: impl Fn(&T) -> Item) -> Vec<T> {
    let terms: Vec<String> = fold(&q.q).split_whitespace().map(str::to_owned).collect();
    let mut kept: Vec<(Item, T)> = rows
        .into_iter()
        .map(|r| (item(&r), r))
        .filter(|(i, _)| match q.status.as_str() {
            "" => !hidden.contains(&i.status.as_str()),
            "all" => true,
            s => i.status == s,
        })
        .filter(|(i, _)| match i.at {
            Some(at) => {
                let d = at.date();
                q.from.is_none_or(|f| d >= f) && q.to.is_none_or(|t| d <= t)
            }
            None => true,
        })
        .filter(|(i, _)| {
            if terms.is_empty() {
                return true;
            }
            let hay = fold(&format!("{} {}", i.text, i.sender));
            terms.iter().all(|t| hay.contains(t.as_str()))
        })
        .collect();
    match q.sort.as_str() {
        "new" => kept.sort_by_key(|k| std::cmp::Reverse(k.0.at)),
        "old" => kept.sort_by_key(|k| k.0.at),
        "sender" | "sender_desc" => {
            let desc = q.sort == "sender_desc";
            kept.sort_by(|a, b| {
                let (x, y) = (fold(&a.0.sender), fold(&b.0.sender));
                let by_sender = if desc { y.cmp(&x) } else { x.cmp(&y) };
                by_sender.then_with(|| b.0.at.cmp(&a.0.at))
            })
        }
        _ => {}
    }
    kept.into_iter().map(|(_, r)| r).collect()
}

/// Một lựa chọn trong ô chọn của thanh công cụ.
#[derive(Debug, Clone)]
pub struct Opt {
    pub code: String,
    pub label: String,
    pub selected: bool,
}

/// Dữ liệu cho template `admin_toolbar.html`.
#[derive(Debug, Clone, Default)]
pub struct Toolbar {
    /// Có hiện thanh công cụ không (trang quản trị dùng chung một template cho nhiều mục).
    pub show: bool,
    /// Địa chỉ của trang (form gửi `GET` về đây; "Xóa lọc" trỏ về đây).
    pub action: String,
    pub q: String,
    pub from: String,
    pub to: String,
    pub placeholder: String,
    pub sorts: Vec<Opt>,
    pub statuses: Vec<Opt>,
    pub shown: usize,
    pub total: usize,
    pub active: bool,
}

impl Toolbar {
    /// `statuses`: (mã, nhãn) của các trạng thái; rỗng thì không có ô lọc trạng thái.
    /// `default_label`: nhãn của lựa chọn trạng thái mặc định (ví dụ "Đang mở"; khi không có trạng
    /// thái ẩn thì là "Tất cả").
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        lang: Lang,
        action: &str,
        q: &ListQuery,
        placeholder: &str,
        default_label: &str,
        statuses: &[(&str, String)],
        shown: usize,
        total: usize,
    ) -> Toolbar {
        let fmt = time::macros::format_description!("[year]-[month]-[day]");
        let sorts = [
            ("", "list.sort.default"),
            ("new", "list.sort.new"),
            ("old", "list.sort.old"),
            ("sender", "list.sort.sender"),
            ("sender_desc", "list.sort.sender_desc"),
        ]
        .into_iter()
        .map(|(code, key)| Opt {
            code: code.to_owned(),
            label: lang.t(key).to_owned(),
            selected: q.sort == code,
        })
        .collect();
        let statuses = if statuses.is_empty() {
            Vec::new()
        } else {
            let mut out = vec![Opt {
                code: String::new(),
                label: default_label.to_owned(),
                selected: q.status.is_empty(),
            }];
            if default_label != lang.t("list.status.all") {
                out.push(Opt {
                    code: "all".into(),
                    label: lang.t("list.status.all").to_owned(),
                    selected: q.status == "all",
                });
            }
            out.extend(statuses.iter().map(|(code, label)| Opt {
                code: (*code).to_owned(),
                label: label.clone(),
                selected: q.status == *code,
            }));
            out
        };
        Toolbar {
            show: true,
            action: action.to_owned(),
            q: q.q.clone(),
            from: q.from.and_then(|d| d.format(&fmt).ok()).unwrap_or_default(),
            to: q.to.and_then(|d| d.format(&fmt).ok()).unwrap_or_default(),
            placeholder: placeholder.to_owned(),
            sorts,
            statuses,
            shown,
            total,
            active: q.active(),
        }
    }

    /// "Đang hiện N/M".
    pub fn count(&self, lang: &Lang) -> String {
        lang.t("list.count")
            .replace("{shown}", &self.shown.to_string())
            .replace("{total}", &self.total.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn query(pairs: &[(&str, &str)]) -> ListQuery {
        ListQuery::parse(
            &pairs
                .iter()
                .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
                .collect(),
        )
    }

    struct Row(&'static str, &'static str, &'static str, i64);

    fn rows() -> Vec<Row> {
        vec![
            Row("Công ty Âu Lạc", "Sổ tay 1.0", "open", 1_763_700_000),
            Row("beta studio", "Kibi 2.0", "handled", 1_763_800_000),
            Row("Đỗ Minh", "Đơn DH-1", "completed", 1_763_600_000),
        ]
    }

    fn run(q: &ListQuery, hidden: &[&str]) -> Vec<&'static str> {
        apply(rows(), q, hidden, |r| Item {
            at: Some(OffsetDateTime::from_unix_timestamp(r.3).unwrap()),
            sender: r.0.into(),
            text: r.1.into(),
            status: r.2.into(),
        })
        .into_iter()
        .map(|r| r.0)
        .collect()
    }

    #[test]
    fn fold_removes_vietnamese_marks() {
        assert_eq!(fold("Công ty ÂU LẠC"), "cong ty au lac");
        assert_eq!(fold("Đỗ Minh Đức"), "do minh duc");
        assert_eq!(fold("Nguyễn Thị Hương"), "nguyen thi huong");
    }

    #[test]
    fn parse_keeps_known_values_only() {
        let q = query(&[
            ("q", "  au lac "),
            ("sort", "drop table"),
            ("status", "open"),
            ("from", "2026-11-20"),
            ("to", "21/11/2026"),
        ]);
        assert_eq!(q.q, "au lac");
        assert_eq!(q.sort, "");
        assert_eq!(q.status, "open");
        assert_eq!(q.from, Some(time::macros::date!(2026 - 11 - 20)));
        assert_eq!(q.to, None);
        assert!(q.active());
        assert!(!query(&[]).active());
        assert_eq!(query(&[("q", &"x".repeat(500))]).q.len(), MAX_QUERY);
    }

    #[test]
    fn search_ignores_case_and_marks() {
        assert_eq!(run(&query(&[("q", "au lac")]), &[]), ["Công ty Âu Lạc"]);
        assert_eq!(run(&query(&[("q", "KIBI beta")]), &[]), ["beta studio"]);
        assert_eq!(run(&query(&[("q", "dh-1 do")]), &[]), ["Đỗ Minh"]);
        assert!(run(&query(&[("q", "khong co")]), &[]).is_empty());
    }

    #[test]
    fn default_order_is_kept_and_sorts_work() {
        assert_eq!(
            run(&query(&[]), &[]),
            ["Công ty Âu Lạc", "beta studio", "Đỗ Minh"]
        );
        assert_eq!(
            run(&query(&[("sort", "new")]), &[]),
            ["beta studio", "Công ty Âu Lạc", "Đỗ Minh"]
        );
        assert_eq!(
            run(&query(&[("sort", "old")]), &[]),
            ["Đỗ Minh", "Công ty Âu Lạc", "beta studio"]
        );
        assert_eq!(
            run(&query(&[("sort", "sender")]), &[]),
            ["beta studio", "Công ty Âu Lạc", "Đỗ Minh"]
        );
        assert_eq!(
            run(&query(&[("sort", "sender_desc")]), &[]),
            ["Đỗ Minh", "Công ty Âu Lạc", "beta studio"]
        );
    }

    #[test]
    fn status_and_date_filters() {
        let hidden = ["completed"];
        assert_eq!(run(&query(&[]), &hidden).len(), 2);
        assert_eq!(run(&query(&[("status", "all")]), &hidden).len(), 3);
        assert_eq!(
            run(&query(&[("status", "handled")]), &hidden),
            ["beta studio"]
        );
        // 1_763_700_000 = 21/11/2025 (UTC).
        let day = OffsetDateTime::from_unix_timestamp(1_763_700_000)
            .unwrap()
            .date()
            .to_string();
        assert_eq!(
            run(&query(&[("from", &day), ("to", &day)]), &[]),
            ["Công ty Âu Lạc"]
        );
    }
}
