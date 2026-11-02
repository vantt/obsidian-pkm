//! Gói OHS giả cho test của `pkm` (MCP server và CLI): `fake-ohs` của `pkm-core` đóng vai Node của gói OHS.

use std::fs;
use std::path::{Path, PathBuf};

#[path = "../../../pkm-core/tests/support/process.rs"]
pub mod process;

/// Gói OHS giả: Node đi kèm là `fake-ohs` (binary dev của `pkm-core`, server giả nói giao thức stdio của OHS), cùng
/// `cli.js` rỗng. `fake-ohs` lấy từ `PKM_FAKE_OHS`, hoặc cạnh binary `pkm` (có khi `cargo test --workspace` đã dựng
/// test của `pkm-core`); không có thì `None`.
pub fn fake_ohs_package(root: &Path) -> Option<PathBuf> {
    let exe = std::env::var_os("PKM_FAKE_OHS")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            Path::new(env!("CARGO_BIN_EXE_pkm"))
                .with_file_name(format!("fake-ohs{}", std::env::consts::EXE_SUFFIX))
        });
    if !exe.is_file() {
        eprintln!(
            "bỏ qua: không thấy fake-ohs ({}); chạy cargo test --workspace hoặc đặt PKM_FAKE_OHS",
            exe.display()
        );
        return None;
    }
    let pkg = root.join("ohs");
    let node = if cfg!(windows) {
        pkg.join("node").join("node.exe")
    } else {
        pkg.join("node").join("bin").join("node")
    };
    fs::create_dir_all(node.parent().unwrap()).unwrap();
    fs::copy(&exe, &node).unwrap();
    let cli = pkg.join("app").join("dist").join("src").join("cli.js");
    fs::create_dir_all(cli.parent().unwrap()).unwrap();
    fs::write(&cli, "").unwrap();
    Some(pkg)
}

/// PID của mọi lần khởi động `fake-ohs` trên vault này.
pub fn fake_pids(vault: &Path) -> Vec<u32> {
    fs::read_to_string(vault.join("fake-ohs.starts"))
        .unwrap_or_default()
        .lines()
        .filter_map(|l| l.trim().parse().ok())
        .collect()
}
