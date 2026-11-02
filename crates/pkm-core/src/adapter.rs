//! Adapter của vault riêng: thêm, sửa, bỏ cạnh của mình sau khi lõi đã tính cạnh (`index.build`), ẩn trang hệ
//! thống khỏi bước đi giữa của tool.
//!
//! Đặc tả: phần adapter của `scripts/kg/index.py` (`Ctx`, `load_adapter`) ở sha ghim. Bản Python nạp adapter từ
//! tên ngắn hoặc từ file `.py` bất kỳ; Rust chỉ có adapter dựng sẵn (hiện là [`e11`]), không chạy được file `.py`.

pub mod e11;

use std::path::Path;

use crate::index::{Ctx, IndexError};

/// Một adapter của vault riêng. Mọi phương thức trừ [`Adapter::edges`] có mặc định như khi adapter Python không
/// có thuộc tính tương ứng.
pub trait Adapter: Send + Sync {
    /// `name` (ghi vào `meta.adapter`); `None` khi adapter không đặt tên.
    fn name(&self) -> Option<&str> {
        None
    }

    /// Chữ dùng để nạp adapter (`meta.adapter_spec`, để lần build sau không phải ghi lại); rỗng khi dựng trực tiếp.
    fn spec(&self) -> &str {
        ""
    }

    /// Tiền tố đường dẫn (kết thúc `/`) hoặc đường dẫn đủ của note hệ thống: tool không đi qua và không trả chúng
    /// trừ khi hỏi `system`.
    fn hidden(&self) -> &[&'static str] {
        &[]
    }

    /// Câu thêm vào `instructions` của MCP server.
    fn instructions(&self) -> Option<&str> {
        None
    }

    /// Chuỗi đổi khi dữ liệu riêng của adapter đổi. Có (`Some`) thì build không đổi gì khỏi tính lại cạnh; `None`
    /// thì mỗi lần build đều tính lại.
    fn digest(&self, _vault: &Path) -> Option<String> {
        None
    }

    /// Thêm, sửa, bỏ cạnh qua `ctx` (chạy trong cùng giao dịch ghi cạnh).
    fn edges(&self, ctx: &mut Ctx<'_>) -> Result<(), IndexError>;
}

/// Tên ngắn của các adapter dựng sẵn.
pub const NAMES: [&str; 1] = ["e11"];

/// `load_adapter`: adapter theo tên ngắn (`e11`: bản B). Rỗng hoặc `none` là không có adapter.
pub fn load(spec: &str) -> Result<Option<Box<dyn Adapter>>, IndexError> {
    match spec {
        "" | "none" => Ok(None),
        "e11" => Ok(Some(Box::new(e11::E11::loaded(spec)))),
        _ => Err(IndexError::Adapter(format!(
            "không có adapter {} (tên ngắn: {}; bản Rust không nạp adapter từ file .py)",
            crate::vault::yaml::repr::str_repr(spec),
            NAMES.join(", ")
        ))),
    }
}

#[cfg(test)]
mod tests;
