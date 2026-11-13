//! Dữ liệu của app Ghi chú: danh sách ghi chú lưu trong `notes.json` ở thư mục dữ liệu mà
//! F-Launcher cấp cho app.

use jiff::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Note {
    pub id: u64,
    pub title: String,
    pub body: String,
    pub updated_at: Timestamp,
}

impl Note {
    /// Tiêu đề hiển thị: tiêu đề, hoặc dòng đầu của nội dung, hoặc "Ghi chú mới".
    pub fn display_title(&self) -> String {
        let title = self.title.trim();
        if !title.is_empty() {
            return title.to_owned();
        }
        self.body
            .lines()
            .map(str::trim)
            .find(|l| !l.is_empty())
            .map(|l| l.chars().take(40).collect())
            .unwrap_or_else(|| "Ghi chú mới".into())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Notebook {
    pub notes: Vec<Note>,
    #[serde(default)]
    next_id: u64,
}

impl Notebook {
    pub fn path(dir: &Path) -> PathBuf {
        dir.join("notes.json")
    }

    /// Đọc sổ ghi chú; chưa có file thì sổ trống. File hỏng được giữ lại với đuôi `.corrupt`.
    pub fn load(dir: &Path) -> std::io::Result<Self> {
        let path = Self::path(dir);
        match std::fs::read(&path) {
            Ok(bytes) => match serde_json::from_slice(&bytes) {
                Ok(book) => Ok(book),
                Err(_) => {
                    std::fs::rename(&path, path.with_extension("json.corrupt"))?;
                    Ok(Self::default())
                }
            },
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(Self::default()),
            Err(e) => Err(e),
        }
    }

    pub fn save(&self, dir: &Path) -> std::io::Result<()> {
        let bytes = serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?;
        feco_sample_common::write_atomic(&Self::path(dir), &bytes)
    }

    /// Thêm ghi chú trống lên đầu danh sách; trả mã của ghi chú mới.
    pub fn add(&mut self, now: Timestamp) -> u64 {
        self.next_id = self
            .next_id
            .max(self.notes.iter().map(|n| n.id).max().unwrap_or(0))
            + 1;
        self.notes.insert(
            0,
            Note {
                id: self.next_id,
                title: String::new(),
                body: String::new(),
                updated_at: now,
            },
        );
        self.next_id
    }

    pub fn remove(&mut self, id: u64) {
        self.notes.retain(|n| n.id != id);
    }

    pub fn get_mut(&mut self, id: u64) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    /// Ghi chú khớp chuỗi tìm (không phân biệt hoa thường), mới sửa trước.
    pub fn search(&self, text: &str) -> Vec<&Note> {
        let text = text.trim().to_lowercase();
        let mut out: Vec<&Note> = self
            .notes
            .iter()
            .filter(|n| {
                text.is_empty()
                    || n.title.to_lowercase().contains(&text)
                    || n.body.to_lowercase().contains(&text)
            })
            .collect();
        out.sort_by(|a, b| b.updated_at.cmp(&a.updated_at).then(b.id.cmp(&a.id)));
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: i64) -> Timestamp {
        Timestamp::from_second(s).unwrap()
    }

    #[test]
    fn add_edit_search_remove_and_persist() {
        let dir = std::env::temp_dir().join(format!("f-notes-test-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        let mut book = Notebook::load(&dir).unwrap();
        assert!(book.notes.is_empty());
        let a = book.add(at(1));
        let b = book.add(at(2));
        assert_ne!(a, b);
        let note = book.get_mut(a).unwrap();
        note.body = "\n  Mua sữa\nmua bánh".into();
        note.updated_at = at(3);
        assert_eq!(
            book.notes
                .iter()
                .find(|n| n.id == a)
                .unwrap()
                .display_title(),
            "Mua sữa"
        );
        assert_eq!(book.search("SỮA").len(), 1);
        assert_eq!(book.search("").first().unwrap().id, a, "mới sửa trước");
        book.save(&dir).unwrap();
        let mut again = Notebook::load(&dir).unwrap();
        assert_eq!(again, book);
        again.remove(a);
        // Mã không bị dùng lại sau khi xóa.
        let c = again.add(at(4));
        assert!(c > b);
        let _ = std::fs::remove_dir_all(dir);
    }

    #[test]
    fn corrupt_file_is_kept_aside() {
        let dir = std::env::temp_dir().join(format!("f-notes-bad-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(Notebook::path(&dir), b"{not json").unwrap();
        assert!(Notebook::load(&dir).unwrap().notes.is_empty());
        assert!(dir.join("notes.json.corrupt").exists());
        let _ = std::fs::remove_dir_all(dir);
    }
}
