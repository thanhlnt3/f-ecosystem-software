use std::path::{Path, PathBuf};

/// Lỗi của chuỗi tin cậy. Mọi lỗi xác minh của `tough` (chữ ký sai, metadata cũ hơn, hết hạn,
/// sai hash) đi qua [`Error::Tuf`].
#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("TUF: {0}")]
    Tuf(Box<tough::error::Error>),
    #[error("metadata TUF: {0}")]
    Schema(Box<tough::schema::Error>),
    #[error("khóa {source_name}: {message}")]
    Key {
        source_name: String,
        message: String,
    },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("{0}")]
    Invalid(String),
}

impl From<tough::error::Error> for Error {
    fn from(e: tough::error::Error) -> Self {
        Error::Tuf(Box::new(e))
    }
}

impl From<tough::schema::Error> for Error {
    fn from(e: tough::schema::Error) -> Self {
        Error::Schema(Box::new(e))
    }
}

impl Error {
    /// Lỗi gốc của `tough`, để test và launcher phân loại (rollback, hết hạn, sai chữ ký...).
    pub fn tuf(&self) -> Option<&tough::error::Error> {
        match self {
            Error::Tuf(e) => Some(e),
            _ => None,
        }
    }
}

pub(crate) fn io_err(path: &Path) -> impl FnOnce(std::io::Error) -> Error + '_ {
    move |source| Error::Io {
        path: path.to_path_buf(),
        source,
    }
}

pub type Result<T> = std::result::Result<T, Error>;
