//! `pkm --version` phải in phiên bản và đúng sha đặc tả đang ghim trong `spec/spec.lock`.

use std::path::Path;
use std::process::Command;

fn pinned_sha() -> String {
    let lock = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("spec")
        .join("spec.lock");
    let text = std::fs::read_to_string(&lock).expect("đọc được spec/spec.lock");
    text.lines()
        .map(str::trim)
        .find_map(|l| l.strip_prefix("sha = \""))
        .and_then(|rest| rest.split('"').next())
        .expect("spec.lock có [spec].sha")
        .to_owned()
}

#[test]
fn version_flag_prints_package_version_and_pinned_spec_sha() {
    let out = Command::new(env!("CARGO_BIN_EXE_pkm"))
        .arg("--version")
        .output()
        .expect("chạy được pkm");
    assert!(out.status.success(), "pkm --version thoát lỗi: {out:?}");
    let stdout = String::from_utf8(out.stdout).expect("stdout là UTF-8");
    let expected = format!("pkm {} (spec {})", env!("CARGO_PKG_VERSION"), pinned_sha());
    assert_eq!(stdout.trim(), expected);
}
