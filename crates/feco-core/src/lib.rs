//! Kiểu dữ liệu dùng chung của F-Eco.
//!
//! - [`Vnd`]: số tiền VND, luôn là số nguyên (spec `payments`, "Phương thức thanh toán").
//! - [`Scope`]: danh mục scope dữ liệu cố định (spec `user-data-consent`).
//! - [`Environment`]: môi trường chạy (dev, staging, production).
//! - [`ui_palette`]: bảng màu 10 phong cách giao diện (F-Launcher và web).

mod environment;
mod money;
mod scope;
pub mod ui_palette;

pub use environment::{Environment, ParseEnvironmentError};
pub use money::{MoneyError, Vnd};
pub use scope::{ParseScopeError, Scope};
