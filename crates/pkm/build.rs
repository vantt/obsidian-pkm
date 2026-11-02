//! Đọc sha đặc tả đang ghim trong `spec/spec.lock` lúc build, đưa vào binary qua biến
//! `PKM_SPEC_SHA` để `pkm --version` in được bản Python mà Rust đã khớp.

use std::path::PathBuf;
use std::{env, fs};

fn main() {
    let manifest_dir = PathBuf::from(
        env::var_os("CARGO_MANIFEST_DIR").expect("cargo luôn đặt CARGO_MANIFEST_DIR"),
    );
    let lock = manifest_dir
        .join("..")
        .join("..")
        .join("spec")
        .join("spec.lock");
    println!("cargo:rerun-if-changed={}", lock.display());

    let text = fs::read_to_string(&lock)
        .unwrap_or_else(|e| panic!("không đọc được {}: {e}", lock.display()));
    let sha = spec_sha(&text).unwrap_or_else(|e| panic!("{} không hợp lệ: {e}", lock.display()));
    println!("cargo:rustc-env=PKM_SPEC_SHA={sha}");
}

/// Lấy `sha` trong bảng `[spec]`. Chỉ hiểu dạng `key = "value"` mà `spec.lock` dùng, đủ để
/// không phải kéo một crate TOML vào build-dependency.
fn spec_sha(text: &str) -> Result<String, String> {
    let mut in_spec = false;
    for line in text.lines() {
        let line = line.split('#').next().unwrap_or("").trim();
        if line.starts_with('[') {
            in_spec = line == "[spec]";
            continue;
        }
        if !in_spec {
            continue;
        }
        let Some((key, value)) = line.split_once('=') else {
            continue;
        };
        if key.trim() != "sha" {
            continue;
        }
        let sha = value.trim().trim_matches('"');
        if sha.len() != 40 || !sha.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Err(format!("[spec].sha phải là 40 ký tự hex, gặp {sha:?}"));
        }
        return Ok(sha.to_ascii_lowercase());
    }
    Err("thiếu [spec].sha".to_owned())
}
