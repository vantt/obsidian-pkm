//! Tìm gói OHS dựng sẵn của fork: thư mục có Node đi kèm (`node/`), `app/dist/src/cli.js` và gói tiếng Việt
//! (`lang/ohs-lang-vi`, nếu có).

use std::ffi::OsString;
use std::fmt;
use std::io;
use std::path::{Path, PathBuf};

use crate::platform;

/// Biến môi trường chỉ tới gói OHS khi không có cờ `--ohs`.
pub const OHS_DIR_ENV: &str = "PKM_OHS_DIR";

/// Tên thư mục gói OHS trong [`platform::pkm_dir`], nơi bộ cài đặt gói.
pub const DEFAULT_DIR_NAME: &str = "ohs";

/// Đường dẫn của `cli.js` trong gói.
const CLI_REL: &str = "app/dist/src/cli.js";

/// Thư mục gói ngôn ngữ tiếng Việt trong gói.
const PACK_REL: &str = "lang/ohs-lang-vi";

/// Biến môi trường OHS đọc để nạp gói ngôn ngữ. Mọi tiến trình dùng chung một index phải cùng một giá trị, nếu không
/// OHS dựng lại bảng view qua lại.
pub const LANGUAGE_PACK_ENV: &str = "OHS_LANGUAGE_PACK";

/// Biến môi trường OHS đọc cho mẫu đường dẫn bỏ qua khi index.
pub const IGNORE_PATTERNS_ENV: &str = "OBSIDIAN_IGNORE_PATTERNS";

/// Mẫu bỏ qua đi cùng gói tiếng Việt: mặc định của upstream thêm `_ohs/**`, thư mục gói vi đọc từ điển
/// (`_ohs/lexicon.md`, không phải note). Như `OHS_IGNORE` của `platform_paths.ohs_settings` (Python, `24f193a`): mọi
/// tiến trình trên một index phải cùng giá trị, không thì OHS báo lệch content policy.
pub const PACK_IGNORE_PATTERNS: &str = ".obsidian/**,templates/**,*.canvas,_ohs/**";

/// Có gửi [`PACK_IGNORE_PATTERNS`] không. Bật từ khi bản B được index lại với mẫu này (E12.9.0 ở repo đặc tả,
/// 2026-11-02), như `OHS_IGNORE_ON = True` của `platform_paths` (Python, `8f89403`). Chỉ tắt cùng lúc `reindex --force`
/// và mọi bên gọi khác: một bên gửi mẫu khác làm bên làm chủ index lệch content policy.
pub const PACK_IGNORE_ON: bool = true;

/// File mô tả gói của fork, có `version`.
const MANIFEST: &str = "MANIFEST.json";

/// Không tìm thấy gói OHS, hoặc gói thiếu file. Thông điệp nêu thư mục đã thử.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PackageError(pub String);

impl fmt::Display for PackageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for PackageError {}

/// Gói OHS đã kiểm đủ file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OhsPackage {
    /// Thư mục gói; OHS chạy với thư mục hiện tại là đây.
    pub root: PathBuf,
    /// Node đi kèm gói.
    pub node: PathBuf,
    /// `app/dist/src/cli.js`.
    pub cli: PathBuf,
    /// Gói tiếng Việt `lang/ohs-lang-vi` khi gói có; OHS chạy với [`LANGUAGE_PACK_ENV`] trỏ tới đây.
    pub pack: Option<PathBuf>,
}

impl OhsPackage {
    /// Tìm gói theo thứ tự: `flag` (cờ `--ohs`), biến [`OHS_DIR_ENV`], rồi [`OhsPackage::default_dir`]. Nguồn nào
    /// được chỉ ra mà sai thì báo lỗi ngay, không lặng lẽ thử nguồn sau.
    pub fn locate(flag: Option<&Path>) -> Result<Self, PackageError> {
        locate_from(flag, std::env::var_os(OHS_DIR_ENV), Self::default_dir())
    }

    /// Thư mục gói mặc định: `<pkm_dir>/ohs` (xem [`platform::pkm_dir`]).
    pub fn default_dir() -> io::Result<PathBuf> {
        platform::cache_dir(DEFAULT_DIR_NAME)
    }

    /// Kiểm một thư mục gói có đủ Node đi kèm và `cli.js`. Đường dẫn trả về là tuyệt đối, vì OHS chạy với thư mục
    /// hiện tại là thư mục gói.
    pub fn from_dir(dir: &Path) -> Result<Self, PackageError> {
        let dir = &std::path::absolute(dir)
            .map_err(|e| PackageError(format!("{}: {e}", dir.display())))?;
        if !dir.is_dir() {
            return Err(PackageError(format!(
                "{} không phải thư mục gói OHS (không tồn tại)",
                dir.display()
            )));
        }
        let candidates = node_candidates();
        let node = candidates
            .iter()
            .map(|rel| dir.join(rel))
            .find(|p| p.is_file())
            .ok_or_else(|| {
                PackageError(format!(
                    "{} không có Node đi kèm ({})",
                    dir.display(),
                    candidates.join(" hoặc ")
                ))
            })?;
        let cli = dir.join(CLI_REL);
        if !cli.is_file() {
            return Err(PackageError(format!(
                "{} không có {CLI_REL}",
                dir.display()
            )));
        }
        let pack = Some(dir.join(PACK_REL)).filter(|p| p.is_dir());
        Ok(Self {
            root: dir.to_path_buf(),
            node,
            cli,
            pack,
        })
    }
}

/// DLL của Microsoft Visual C++ Redistributable (x64) mà `onnxruntime-node` (tìm theo nghĩa) trong gói win32-x64 cần.
/// Thiếu thì OHS không nạp được model và báo "`@huggingface/transformers` is not installed".
pub const VC_RUNTIME_DLLS: [&str; 3] = ["vcruntime140.dll", "vcruntime140_1.dll", "msvcp140.dll"];

/// Lệnh cài Visual C++ Redistributable (x64) trên Windows.
pub const VC_RUNTIME_FIX: &str = "winget install --id Microsoft.VCRedist.2015+.x64";

/// Thư mục binding gốc của `onnxruntime-node` trong gói win32-x64; DLL đặt cạnh nó cũng được Windows tìm thấy.
const ONNX_BINDING_DIR: &str = "app/node_modules/onnxruntime-node/bin/napi-v6/win32/x64";

/// DLL trong [`VC_RUNTIME_DLLS`] không có ở `binding_dir` lẫn `system_dir`. Rỗng khi không có `binding_dir` (gói
/// không dùng binding gốc win32-x64 của `onnxruntime-node`, như gói linux hay macOS).
pub fn missing_vc_runtime_in(binding_dir: &Path, system_dir: Option<&Path>) -> Vec<String> {
    if !binding_dir.is_dir() {
        return Vec::new();
    }
    VC_RUNTIME_DLLS
        .iter()
        .filter(|dll| {
            !binding_dir.join(dll).is_file() && !system_dir.is_some_and(|d| d.join(dll).is_file())
        })
        .map(|dll| (*dll).to_owned())
        .collect()
}

impl OhsPackage {
    /// Trên Windows: DLL của Visual C++ Redistributable mà `onnxruntime-node` của gói cần nhưng máy không có (không ở
    /// `%SystemRoot%\System32`, không cạnh binding). Nơi khác luôn rỗng.
    pub fn missing_vc_runtime(&self) -> Vec<String> {
        if !cfg!(windows) {
            return Vec::new();
        }
        let system = std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32"));
        missing_vc_runtime_in(&self.root.join(ONNX_BINDING_DIR), system.as_deref())
    }

    /// Phiên bản OHS ghi trong `MANIFEST.json` của gói (`0.17.2-vantt.3`…); `None` khi thiếu file hay khoá.
    pub fn version(&self) -> Option<String> {
        let text = std::fs::read_to_string(self.root.join(MANIFEST)).ok()?;
        let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
        manifest.get("version")?.as_str().map(str::to_owned)
    }
}

/// Node trong gói: `node/node.exe` trên Windows, `node/bin/node` (hoặc `node/node`) nơi khác.
fn node_candidates() -> &'static [&'static str] {
    if cfg!(windows) {
        &["node/node.exe"]
    } else {
        &["node/bin/node", "node/node"]
    }
}

/// [`OhsPackage::locate`] với giá trị biến môi trường và thư mục mặc định truyền vào, để test không đổi môi trường
/// của tiến trình. Biến rỗng coi như không đặt.
fn locate_from(
    flag: Option<&Path>,
    env: Option<OsString>,
    default: io::Result<PathBuf>,
) -> Result<OhsPackage, PackageError> {
    if let Some(dir) = flag {
        return OhsPackage::from_dir(dir).map_err(|e| PackageError(format!("cờ --ohs: {e}")));
    }
    if let Some(dir) = env.filter(|v| !v.is_empty()) {
        return OhsPackage::from_dir(Path::new(&dir))
            .map_err(|e| PackageError(format!("{OHS_DIR_ENV}: {e}")));
    }
    let dir = default.map_err(|e| {
        PackageError(format!(
            "chưa chỉ gói OHS (cờ --ohs, {OHS_DIR_ENV}) và không xác định được thư mục mặc định: {e}"
        ))
    })?;
    OhsPackage::from_dir(&dir).map_err(|e| {
        PackageError(format!(
            "chưa chỉ gói OHS (cờ --ohs, {OHS_DIR_ENV}) và thư mục mặc định không dùng được: {e}"
        ))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn make_package(dir: &Path) {
        let node = dir.join(node_candidates()[0]);
        fs::create_dir_all(node.parent().unwrap()).unwrap();
        fs::write(&node, b"").unwrap();
        fs::create_dir_all(dir.join("app/dist/src")).unwrap();
        fs::write(dir.join(CLI_REL), b"").unwrap();
    }

    #[test]
    fn vc_runtime_is_found_in_the_system_dir_or_beside_the_binding() {
        let tmp = tempfile::tempdir().unwrap();
        let binding = tmp.path().join(ONNX_BINDING_DIR);
        let system = tmp.path().join("System32");
        assert!(
            missing_vc_runtime_in(&binding, Some(&system)).is_empty(),
            "gói không có binding win32-x64 thì không cần runtime"
        );
        fs::create_dir_all(&binding).unwrap();
        fs::create_dir_all(&system).unwrap();
        assert_eq!(
            missing_vc_runtime_in(&binding, Some(&system)),
            VC_RUNTIME_DLLS
        );
        assert_eq!(missing_vc_runtime_in(&binding, None), VC_RUNTIME_DLLS);
        fs::write(system.join("vcruntime140.dll"), b"").unwrap();
        fs::write(binding.join("msvcp140.dll"), b"").unwrap();
        assert_eq!(
            missing_vc_runtime_in(&binding, Some(&system)),
            ["vcruntime140_1.dll"]
        );
    }

    #[test]
    fn package_requires_node_and_cli() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        fs::create_dir_all(dir.join("app/dist/src")).unwrap();
        let err = OhsPackage::from_dir(dir).unwrap_err();
        assert!(err.0.contains("Node đi kèm"), "{err}");
        let node = dir.join(node_candidates()[0]);
        fs::create_dir_all(node.parent().unwrap()).unwrap();
        fs::write(&node, b"").unwrap();
        let err = OhsPackage::from_dir(dir).unwrap_err();
        assert!(err.0.contains(CLI_REL), "{err}");
        fs::write(dir.join(CLI_REL), b"").unwrap();
        let pkg = OhsPackage::from_dir(dir).unwrap();
        assert_eq!(
            (pkg.node, pkg.cli, pkg.pack),
            (node, dir.join(CLI_REL), None)
        );
    }

    #[test]
    fn vietnamese_pack_is_found_only_as_a_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path();
        make_package(dir);
        fs::create_dir_all(dir.join("lang")).unwrap();
        fs::write(dir.join(PACK_REL), b"").unwrap();
        assert_eq!(OhsPackage::from_dir(dir).unwrap().pack, None);
        fs::remove_file(dir.join(PACK_REL)).unwrap();
        fs::create_dir(dir.join(PACK_REL)).unwrap();
        assert_eq!(
            OhsPackage::from_dir(dir).unwrap().pack,
            Some(dir.join(PACK_REL))
        );
    }

    #[test]
    fn reads_manifest_version() {
        let tmp = tempfile::tempdir().unwrap();
        make_package(tmp.path());
        let pkg = OhsPackage::from_dir(tmp.path()).unwrap();
        assert_eq!(pkg.version(), None);
        fs::write(
            tmp.path().join(MANIFEST),
            r#"{"name":"ohs","version":"0.17.2-vantt.3"}"#,
        )
        .unwrap();
        assert_eq!(pkg.version().as_deref(), Some("0.17.2-vantt.3"));
    }

    #[test]
    fn missing_package_dir_is_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let err = OhsPackage::from_dir(&tmp.path().join("không có")).unwrap_err();
        assert!(err.0.contains("không tồn tại"), "{err}");
    }

    #[test]
    fn flag_wins_over_env_and_default() {
        let (a, b, c) = (
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
            tempfile::tempdir().unwrap(),
        );
        for d in [&a, &b, &c] {
            make_package(d.path());
        }
        let got = locate_from(
            Some(a.path()),
            Some(b.path().into()),
            Ok(c.path().to_path_buf()),
        )
        .unwrap();
        assert_eq!(got.root, a.path());
        let got = locate_from(None, Some(b.path().into()), Ok(c.path().to_path_buf())).unwrap();
        assert_eq!(got.root, b.path());
        let got = locate_from(None, Some(OsString::new()), Ok(c.path().to_path_buf())).unwrap();
        assert_eq!(got.root, c.path());
    }

    #[test]
    fn a_wrong_source_is_reported_not_skipped() {
        let good = tempfile::tempdir().unwrap();
        make_package(good.path());
        let empty = tempfile::tempdir().unwrap();
        let err = locate_from(Some(empty.path()), None, Ok(good.path().to_path_buf())).unwrap_err();
        assert!(err.0.starts_with("cờ --ohs:"), "{err}");
        let err = locate_from(
            None,
            Some(empty.path().into()),
            Ok(good.path().to_path_buf()),
        )
        .unwrap_err();
        assert!(err.0.starts_with(OHS_DIR_ENV), "{err}");
    }

    #[test]
    fn missing_default_names_every_source() {
        let tmp = tempfile::tempdir().unwrap();
        let err = locate_from(None, None, Ok(tmp.path().join(DEFAULT_DIR_NAME))).unwrap_err();
        assert!(
            err.0.contains("--ohs") && err.0.contains(OHS_DIR_ENV),
            "{err}"
        );
        let err = locate_from(
            None,
            None,
            Err(io::Error::new(io::ErrorKind::NotFound, "no home")),
        )
        .unwrap_err();
        assert!(err.0.contains("no home"), "{err}");
    }

    #[test]
    fn default_dir_is_inside_pkm_dir() {
        let got = OhsPackage::default_dir().unwrap();
        assert!(
            got.ends_with(Path::new("pkm").join(DEFAULT_DIR_NAME)),
            "{got:?}"
        );
    }
}
