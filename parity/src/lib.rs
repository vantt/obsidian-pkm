//! Bộ so khớp của dự án (docs/PRD.md mục 6): tiêu chuẩn "khớp" duy nhất giữa bản Rust và bản Python.
//!
//! Python chạy trên máy (`parity/dump-python.py`, `parity/parity.py`) và ghi dump; phần so ở đây là
//! Rust để CI chạy được không cần Python (Q5). Cách dùng: `parity/README.md`.

pub mod compare;
pub mod dump;
pub mod exceptions;
pub mod jsonpos;
pub mod normalize;
pub mod report;
pub mod rust_side;

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};

/// Thư mục gốc của repo (cha của `parity/`).
pub fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .map_or_else(|| PathBuf::from("."), Path::to_path_buf)
}

fn spec_lock() -> Result<toml::Table> {
    let path = repo_root().join("spec").join("spec.lock");
    let text = std::fs::read_to_string(&path)
        .with_context(|| format!("không đọc được {}", path.display()))?;
    toml::from_str(&text).with_context(|| format!("{} sai định dạng", path.display()))
}

fn lock_str(lock: &toml::Table, section: &str, key: &str) -> Result<String> {
    let v = lock.get(section).and_then(|s| s.get(key));
    match v {
        Some(toml::Value::String(s)) => Ok(s.clone()),
        Some(toml::Value::Integer(i)) => Ok(i.to_string()),
        _ => anyhow::bail!("spec/spec.lock thiếu [{section}].{key}"),
    }
}

/// Sha đặc tả đang ghim trong `spec/spec.lock`.
pub fn pinned_spec_sha() -> Result<String> {
    lock_str(&spec_lock()?, "spec", "sha")
}

/// Tham số `npx` dựng vault fixture theo `[fixture]` của `spec/spec.lock` (thiếu `--out`):
/// `github:OWNER/REPO#TAG generate --preset PRESET --seed SEED`.
pub fn fixture_args() -> Result<Vec<String>> {
    let lock = spec_lock()?;
    let repo = lock_str(&lock, "fixture", "repo")?;
    let repo = repo
        .strip_prefix("https://github.com/")
        .with_context(|| format!("[fixture].repo {repo:?} không phải repo GitHub"))?;
    Ok(vec![
        format!("github:{repo}#{}", lock_str(&lock, "fixture", "tag")?),
        "generate".into(),
        "--preset".into(),
        lock_str(&lock, "fixture", "preset")?,
        "--seed".into(),
        lock_str(&lock, "fixture", "seed")?,
    ])
}

/// Thư mục golden của một sha.
pub fn golden_dir(sha: &str) -> PathBuf {
    repo_root().join("parity").join("golden").join(sha)
}
