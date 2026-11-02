//! Cài gói OHS dựng sẵn của fork vào `<thư mục pkm>/ohs` (nơi `pkm_core::ohs::OhsPackage::default_dir` tìm).
//!
//! Nguồn ghim (docs/DECISIONS.md, "Gói OHS: ghim GitHub Release của fork"): asset `ohs-<phiên bản>-<nền tảng>`
//! (`.tar.gz`, hoặc `.zip` cho Windows) của bản phát hành [`TAG`] ở repo công khai [`REPO`]. Tải qua HTTPS không cần
//! đăng nhập (bằng [`curl`], có sẵn trên Windows 10 trở lên, macOS, Linux), rồi kiểm sha256 của file với bản ghim
//! trong [`BUNDLES`], biên dịch sẵn vào binary.
//!
//! Không tự tải được (máy không ra mạng…) thì người dùng tải file gói ở trang phát hành rồi chỉ tới file bằng
//! `--ohs-package <file>`; file đó cũng phải khớp sha256 ghim. Cũng nhận gói đã giải nén (thư mục, không kiểm được).
//!
//! Đổi nguồn mà không cần bản `pkm` mới: URL `https://…` của file gói, kèm `<URL>.sha256` cùng chỗ (như asset của
//! GitHub Release), qua `--ohs-package <URL>` hoặc biến [`PACKAGE_ENV`]. Gói tải theo URL được kiểm với file `.sha256`
//! đó, không cần khớp bản ghim.

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use pkm_core::ohs::OhsPackage;
use sha2::{Digest, Sha256};

/// Repo fork của OHS (công khai).
pub const REPO: &str = "vantt/obsidian-hybrid-search";
/// Phiên bản OHS của gói.
pub const VERSION: &str = "0.17.2-vantt.4";
/// Tag của bản phát hành có gói ghim.
pub const TAG: &str = "v0.17.2-vantt.4";
/// Biến môi trường thay nguồn tự tải của bộ cài: đường dẫn (như `--ohs-package`) hoặc URL `https://` của file gói.
pub const PACKAGE_ENV: &str = "PKM_OHS_PACKAGE";

/// Một gói ghim cho một nền tảng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bundle {
    /// Nền tảng theo tên của fork (`win32-x64`, `linux-x64`…).
    pub platform: &'static str,
    /// sha256 của file gói (asset của bản phát hành; khớp file `.sha256` fork đăng kèm).
    pub sha256: &'static str,
}

/// Gói ghim của từng nền tảng trong bản phát hành [`TAG`].
pub const BUNDLES: [Bundle; 4] = [
    Bundle {
        platform: "win32-x64",
        sha256: "5cb480f81ded99abfb2ab3ef0c0582320de85430ebff0d7dcd2f582d53e94f12",
    },
    Bundle {
        platform: "darwin-arm64",
        sha256: "ac2c3175776415a07fe3a933569c81e71fcc532890b49a56b2ed00d8fbcc9952",
    },
    Bundle {
        platform: "darwin-x64",
        sha256: "37eed430ac9697e8db0bab411a017fedd32a6c7e7d3bd14da2b67396f95a2a22",
    },
    Bundle {
        platform: "linux-x64",
        sha256: "e6972863f284a254d1d6c8b33e778cd53df7246df2ac5794e06bcb92838110b4",
    },
];

impl Bundle {
    /// Gói cho máy đang chạy, nếu fork có dựng.
    pub fn current() -> Option<&'static Self> {
        let platform = match (std::env::consts::OS, std::env::consts::ARCH) {
            ("windows", "x86_64") => "win32-x64",
            ("macos", "aarch64") => "darwin-arm64",
            ("macos", "x86_64") => "darwin-x64",
            ("linux", "x86_64") => "linux-x64",
            _ => return None,
        };
        BUNDLES.iter().find(|b| b.platform == platform)
    }

    /// Tên file gói: `.zip` cho Windows, `.tar.gz` cho nơi khác.
    pub fn archive_name(&self) -> String {
        let ext = if self.platform.starts_with("win32") {
            "zip"
        } else {
            "tar.gz"
        };
        format!("ohs-{VERSION}-{}.{ext}", self.platform)
    }

    /// URL tải file gói (asset của bản phát hành [`TAG`]).
    pub fn url(&self) -> String {
        format!(
            "https://github.com/{REPO}/releases/download/{TAG}/{}",
            self.archive_name()
        )
    }

    /// Trang của bản phát hành [`TAG`].
    pub fn release_url() -> String {
        format!("https://github.com/{REPO}/releases/tag/{TAG}")
    }

    /// Việc người dùng làm khi bộ cài không tự tải được.
    pub fn manual_action(&self) -> String {
        format!(
            "tải {} (trang {}) rồi chạy lại với --ohs-package <file vừa tải>",
            self.url(),
            Self::release_url()
        )
    }
}

/// Câu báo khi fork không có gói dựng sẵn cho máy này.
pub fn unsupported_platform() -> String {
    let all: Vec<&str> = BUNDLES.iter().map(|b| b.platform).collect();
    format!(
        "fork chưa có gói OHS dựng sẵn cho nền tảng này ({}-{}); bản phát hành {TAG} chỉ có {}",
        std::env::consts::OS,
        std::env::consts::ARCH,
        all.join(", ")
    )
}

/// Nguồn của gói khi cài.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Tải gói ghim của máy này từ bản phát hành [`TAG`].
    Download,
    /// File gói đã tải, hoặc thư mục gói đã giải nén.
    Path(PathBuf),
    /// URL `https://` của file gói (`.tar.gz` hoặc `.zip`); sha256 lấy từ `<URL>.sha256`.
    Url(String),
}

impl Source {
    /// Nguồn người dùng chỉ ra (cờ `--ohs-package` hoặc biến [`PACKAGE_ENV`]): `https://…` là URL, chuỗi có `://`
    /// khác là lỗi (chỉ tải qua https), còn lại là đường dẫn.
    pub fn parse(text: &str) -> Result<Self, String> {
        if text.starts_with("https://") {
            Ok(Self::Url(text.to_owned()))
        } else if text.contains("://") {
            Err(format!("{text}: chỉ tải gói OHS qua https://"))
        } else {
            Ok(Self::Path(PathBuf::from(text)))
        }
    }
}

/// Cài gói từ `source` vào `dest` (thay gói cũ nếu có), trả gói đã kiểm. `log` nhận các dòng tiến độ. `bundle` là
/// gói ghim của máy này; tải và kiểm file cần nó, chép thư mục và nguồn URL thì không. `fetch(url, file)` tải qua
/// HTTPS (bản thật: [`curl`]).
pub fn install(
    source: &Source,
    bundle: Option<&Bundle>,
    dest: &Path,
    log: &mut dyn Write,
    fetch: &dyn Fn(&str, &Path) -> Result<(), String>,
) -> Result<OhsPackage, String> {
    let parent = dest
        .parent()
        .ok_or_else(|| format!("{} không có thư mục cha", dest.display()))?;
    fs::create_dir_all(parent).map_err(|e| format!("tạo {}: {e}", parent.display()))?;
    let staging = Staging::new(parent, "ohs.new")?;
    let pinned = || bundle.ok_or_else(unsupported_platform);
    let root = match source {
        Source::Download => {
            let bundle = pinned()?;
            let url = bundle.url();
            let file = staging.path.join(bundle.archive_name());
            let _ = writeln!(
                log,
                "tải gói OHS {VERSION} cho {} từ {url}…",
                bundle.platform
            );
            fetch(&url, &file).map_err(|e| format!("{e}; hoặc {}", bundle.manual_action()))?;
            unpack_file(&file, bundle, &staging.path, log)?
        }
        Source::Path(p) if p.is_dir() => {
            let root = package_root(p)?;
            let _ = writeln!(
                log,
                "chép gói OHS từ thư mục {} (không kiểm sha256 được)",
                root.display()
            );
            let out = staging.path.join("pkg");
            copy_tree(&root, &out).map_err(|e| format!("chép {}: {e}", root.display()))?;
            out
        }
        Source::Path(p) => unpack_file(p, pinned()?, &staging.path, log)?,
        Source::Url(url) => {
            let _ = writeln!(log, "tải gói OHS từ {url}…");
            let file = staging.path.join("download");
            fetch(url, &file)?;
            let sidecar = staging.path.join("download.sha256");
            fetch(&format!("{url}.sha256"), &sidecar)
                .map_err(|e| format!("{e}; nguồn URL cần file <URL>.sha256 cùng chỗ"))?;
            let expected = read_sha256(&sidecar)?;
            let got = sha256_file(&file).map_err(|e| format!("đọc gói vừa tải: {e}"))?;
            if got != expected {
                return Err(format!(
                    "sha256 của gói tải từ {url} là {got}, {url}.sha256 ghi {expected}: không cài"
                ));
            }
            let _ = writeln!(log, "sha256 của gói khớp {url}.sha256 ({got})");
            unpack_archive(&file, &staging.path)?
        }
    };
    OhsPackage::from_dir(&root).map_err(|e| format!("gói OHS không dùng được: {e}"))?;
    swap_into(&root, dest)?;
    drop(staging);
    OhsPackage::from_dir(dest).map_err(|e| format!("gói OHS sau khi cài: {e}"))
}

/// Thư mục tạm cạnh đích, xoá khi xong (kể cả khi lỗi).
struct Staging {
    path: PathBuf,
}

impl Staging {
    fn new(parent: &Path, prefix: &str) -> Result<Self, String> {
        let path = parent.join(format!("{prefix}-{}", std::process::id()));
        if path.exists() {
            fs::remove_dir_all(&path).map_err(|e| format!("dọn {}: {e}", path.display()))?;
        }
        fs::create_dir_all(&path).map_err(|e| format!("tạo {}: {e}", path.display()))?;
        Ok(Self { path })
    }
}

impl Drop for Staging {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

/// Tải `url` bằng `curl` (có sẵn trên Windows 10 trở lên, macOS, Linux), chỉ qua https, kể cả khi chuyển hướng;
/// không cần đăng nhập.
pub fn curl(url: &str, out: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args(["--proto", "=https", "--tlsv1.2", "-fsSL", "--retry", "2"])
        .args(["--connect-timeout", "20", "-o"])
        .arg(out)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| match e.kind() {
            io::ErrorKind::NotFound => {
                format!("không có curl để tải {url} (Windows 10 trở lên, macOS, Linux có sẵn)")
            }
            _ => format!("không chạy được curl để tải {url}: {e}"),
        })?;
    if output.status.success() {
        Ok(())
    } else {
        Err(format!(
            "curl không tải được {url} ({})",
            String::from_utf8_lossy(&output.stderr).trim()
        ))
    }
}

/// Chữ đầu của file `.sha256` (dạng `sha256sum`): 64 ký tự hex, đưa về chữ thường.
fn read_sha256(path: &Path) -> Result<String, String> {
    let text = fs::read_to_string(path).map_err(|e| format!("đọc {}: {e}", path.display()))?;
    text.split_whitespace()
        .next()
        .map(str::to_lowercase)
        .filter(|h| h.len() == 64 && h.bytes().all(|b| b.is_ascii_hexdigit()))
        .ok_or_else(|| "file .sha256 không bắt đầu bằng sha256 hex".to_owned())
}

/// Kiểm sha256 của file gói `file` với bản ghim của `bundle` rồi giải nén vào `staging`; trả thư mục gốc của gói.
fn unpack_file(
    file: &Path,
    bundle: &Bundle,
    staging: &Path,
    log: &mut dyn Write,
) -> Result<PathBuf, String> {
    let digest = sha256_file(file).map_err(|e| format!("đọc {}: {e}", file.display()))?;
    if digest == bundle.sha256 {
        let _ = writeln!(log, "sha256 của gói khớp bản ghim ({digest})");
        return unpack_archive(file, staging);
    }
    if let Some(other) = other_platform(&digest, bundle, &BUNDLES) {
        return Err(format!(
            "{} là gói OHS cho {other}, máy này cần {}",
            file.display(),
            bundle.platform
        ));
    }
    Err(format!(
        "{}: sha256 {digest} không khớp gói OHS {VERSION} ghim cho {} ({}), không cài; {}",
        file.display(),
        bundle.platform,
        bundle.sha256,
        bundle.manual_action()
    ))
}

/// Nền tảng khác `bundle` mà `digest` là gói ghim của nó (người dùng tải nhầm gói).
fn other_platform(digest: &str, bundle: &Bundle, all: &[Bundle]) -> Option<&'static str> {
    all.iter()
        .find(|b| b.platform != bundle.platform && b.sha256 == digest)
        .map(|b| b.platform)
}

/// Giải nén file gói (`.tar.gz` hoặc `.zip`) vào `staging/pkg`; trả thư mục gốc của gói.
fn unpack_archive(archive: &Path, staging: &Path) -> Result<PathBuf, String> {
    let out = staging.join("pkg");
    fs::create_dir_all(&out).map_err(|e| format!("tạo {}: {e}", out.display()))?;
    let f = File::open(archive).map_err(|e| format!("mở {}: {e}", archive.display()))?;
    let mut reader = BufReader::new(f);
    let mut magic = [0u8; 2];
    reader
        .read_exact(&mut magic)
        .map_err(|e| format!("đọc {}: {e}", archive.display()))?;
    reader
        .rewind()
        .map_err(|e| format!("đọc {}: {e}", archive.display()))?;
    match magic {
        [0x1f, 0x8b] => tar::Archive::new(flate2::read::GzDecoder::new(reader))
            .unpack(&out)
            .map_err(|e| format!("giải nén {}: {e}", archive.display()))?,
        [b'P', b'K'] => {
            unzip(reader, &out).map_err(|e| format!("giải nén {}: {e}", archive.display()))?
        }
        _ => {
            return Err(format!("{} không phải .tar.gz hay .zip", archive.display()));
        }
    }
    package_root(&out)
}

/// Giải nén zip vào `out`, bỏ mục có đường dẫn thoát ra ngoài `out`; giữ quyền chạy trên Unix.
fn unzip(reader: impl Read + Seek, out: &Path) -> io::Result<()> {
    let mut zip = zip::ZipArchive::new(reader).map_err(io::Error::other)?;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(io::Error::other)?;
        let Some(rel) = entry.enclosed_name() else {
            return Err(io::Error::other(format!(
                "mục zip có đường dẫn không an toàn: {}",
                entry.name()
            )));
        };
        let path = out.join(rel);
        if entry.is_dir() {
            fs::create_dir_all(&path)?;
            continue;
        }
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut f = File::create(&path)?;
        io::copy(&mut entry, &mut f)?;
        #[cfg(unix)]
        if let Some(mode) = entry.unix_mode() {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(mode & 0o777))?;
        }
    }
    Ok(())
}

/// Thư mục gốc của gói trong `dir`: chính `dir` nếu đã là gói, hoặc thư mục con duy nhất của nó (gói của fork có một
/// thư mục `ohs-<phiên bản>-<nền tảng>/` ở đầu).
fn package_root(dir: &Path) -> Result<PathBuf, String> {
    if OhsPackage::from_dir(dir).is_ok() {
        return Ok(dir.to_path_buf());
    }
    let subdirs: Vec<PathBuf> = fs::read_dir(dir)
        .map_err(|e| format!("đọc {}: {e}", dir.display()))?
        .filter_map(Result::ok)
        .map(|e| e.path())
        .filter(|p| p.is_dir())
        .collect();
    match subdirs.as_slice() {
        [one] => OhsPackage::from_dir(one)
            .map(|_| one.clone())
            .map_err(|e| format!("không thấy gói OHS trong {}: {e}", dir.display())),
        _ => OhsPackage::from_dir(dir)
            .map(|_| dir.to_path_buf())
            .map_err(|e| format!("không thấy gói OHS trong {}: {e}", dir.display())),
    }
}

/// Chép cây thư mục, tạo lại symlink trên Unix (Node của gói có `bin/npm` là symlink).
fn copy_tree(src: &Path, dst: &Path) -> io::Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let (from, to) = (entry.path(), dst.join(entry.file_name()));
        let kind = entry.file_type()?;
        if kind.is_dir() {
            copy_tree(&from, &to)?;
        } else if kind.is_symlink() {
            #[cfg(unix)]
            std::os::unix::fs::symlink(fs::read_link(&from)?, &to)?;
            #[cfg(not(unix))]
            {
                fs::copy(&from, &to)?;
            }
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

/// Đặt gói mới `root` vào `dest`: đổi tên gói cũ sang một bên, đổi tên gói mới vào chỗ, rồi xoá gói cũ. Lỗi giữa
/// chừng thì trả gói cũ về chỗ.
fn swap_into(root: &Path, dest: &Path) -> Result<(), String> {
    let old = dest.with_file_name(format!(
        "{}.old-{}",
        dest.file_name()
            .map_or_else(|| "ohs".into(), |n| n.to_string_lossy()),
        std::process::id()
    ));
    let had_old = dest.exists();
    if had_old {
        fs::rename(dest, &old).map_err(|e| {
            format!(
                "không thay được gói OHS cũ ở {} ({e}); có thể OHS đang chạy: thoát Claude Desktop và mọi `pkm` \
                 đang chạy rồi thử lại",
                dest.display()
            )
        })?;
    }
    if let Err(e) = fs::rename(root, dest) {
        if had_old {
            let _ = fs::rename(&old, dest);
        }
        return Err(format!("đặt gói OHS vào {}: {e}", dest.display()));
    }
    if had_old {
        let _ = fs::remove_dir_all(&old);
    }
    Ok(())
}

/// sha256 của file, dạng hex thường.
pub fn sha256_file(path: &Path) -> io::Result<String> {
    let mut f = BufReader::new(File::open(path)?);
    let mut hasher = Sha256::new();
    io::copy(&mut f, &mut hasher)?;
    Ok(hex(&hasher.finalize()))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Cài không tải gì (nguồn đường dẫn): hàm tải nào được gọi là lỗi.
    fn install(
        source: &Source,
        bundle: Option<&Bundle>,
        dest: &Path,
        log: &mut dyn Write,
    ) -> Result<OhsPackage, String> {
        super::install(source, bundle, dest, log, &|url, _| {
            Err(format!("không được tải {url}"))
        })
    }

    /// Gói giả nhỏ: thư mục gốc `ohs-x/` có Node và `cli.js` như gói thật.
    fn node_rel() -> &'static str {
        if cfg!(windows) {
            "node/node.exe"
        } else {
            "node/bin/node"
        }
    }

    fn tar_gz(top: &str) -> Vec<u8> {
        let mut builder = tar::Builder::new(flate2::write::GzEncoder::new(
            Vec::new(),
            flate2::Compression::fast(),
        ));
        for (rel, body) in [(node_rel(), "node"), ("app/dist/src/cli.js", "cli")] {
            let mut header = tar::Header::new_gnu();
            header.set_size(body.len() as u64);
            header.set_mode(0o755);
            header.set_cksum();
            builder
                .append_data(&mut header, format!("{top}/{rel}"), body.as_bytes())
                .unwrap();
        }
        builder.into_inner().unwrap().finish().unwrap()
    }

    fn zip_of(files: &[(&str, &[u8])]) -> Vec<u8> {
        let mut w = zip::ZipWriter::new(io::Cursor::new(Vec::new()));
        for (name, body) in files {
            w.start_file(
                *name,
                zip::write::SimpleFileOptions::default().unix_permissions(0o755),
            )
            .unwrap();
            w.write_all(body).unwrap();
        }
        w.finish().unwrap().into_inner()
    }

    fn sha(bytes: &[u8]) -> String {
        hex(&Sha256::digest(bytes))
    }

    fn leak(s: String) -> &'static str {
        Box::leak(s.into_boxed_str())
    }

    /// Bundle thử cho Linux-như, ghim sha256 của `archive`.
    fn pinned_for(archive: &[u8]) -> Bundle {
        Bundle {
            platform: "linux-x64",
            sha256: leak(sha(archive)),
        }
    }

    /// Hàm tải giả: trả nội dung theo URL từ bảng, URL lạ là lỗi như `curl -f` gặp 404.
    fn served(files: Vec<(String, Vec<u8>)>) -> impl Fn(&str, &Path) -> Result<(), String> {
        move |url, out| {
            let body = files
                .iter()
                .find(|(u, _)| u == url)
                .map(|(_, b)| b)
                .ok_or_else(|| format!("404 {url}"))?;
            fs::write(out, body).map_err(|e| e.to_string())
        }
    }

    #[test]
    fn downloads_pinned_release_asset_and_replaces_old_package() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tar_gz("ohs-x");
        let bundle = pinned_for(&archive);
        let fetch = served(vec![(bundle.url(), archive)]);
        let dest = tmp.path().join("pkm").join("ohs");
        fs::create_dir_all(dest.join("stale")).unwrap();
        let mut log = Vec::new();
        let pkg =
            super::install(&Source::Download, Some(&bundle), &dest, &mut log, &fetch).unwrap();
        assert_eq!(pkg.root, dest);
        assert!(!dest.join("stale").exists());
        assert_eq!(
            fs::read_to_string(dest.join("app/dist/src/cli.js")).unwrap(),
            "cli"
        );
        let names: Vec<_> = fs::read_dir(tmp.path().join("pkm"))
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect();
        assert_eq!(names, ["ohs"], "không còn thư mục tạm");
        let log = String::from_utf8(log).unwrap();
        assert!(log.contains(&bundle.url()), "{log}");
        assert!(log.contains("khớp bản ghim"), "{log}");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let mode = fs::metadata(dest.join(node_rel()))
                .unwrap()
                .permissions()
                .mode();
            assert_eq!(mode & 0o111, 0o111, "node phải chạy được");
        }
    }

    #[test]
    fn rejects_downloaded_asset_not_matching_the_pin() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = pinned_for(b"something else");
        let fetch = served(vec![(bundle.url(), tar_gz("ohs-x"))]);
        let dest = tmp.path().join("ohs");
        let err = super::install(
            &Source::Download,
            Some(&bundle),
            &dest,
            &mut Vec::new(),
            &fetch,
        )
        .unwrap_err();
        assert!(err.contains("không khớp gói OHS"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn download_failure_explains_the_manual_route() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = BUNDLES[3];
        let err = super::install(
            &Source::Download,
            Some(&bundle),
            &tmp.path().join("ohs"),
            &mut Vec::new(),
            &served(Vec::new()),
        )
        .unwrap_err();
        assert!(err.starts_with(&format!("404 {}", bundle.url())), "{err}");
        assert!(err.contains("--ohs-package"), "{err}");
        assert!(err.contains(&Bundle::release_url()), "{err}");
    }

    #[test]
    fn download_without_a_pinned_bundle_names_the_supported_platforms() {
        let tmp = tempfile::tempdir().unwrap();
        let err = super::install(
            &Source::Download,
            None,
            &tmp.path().join("ohs"),
            &mut Vec::new(),
            &served(Vec::new()),
        )
        .unwrap_err();
        assert!(err.contains("chưa có gói OHS dựng sẵn"), "{err}");
        assert!(err.contains(TAG) && err.contains("darwin-arm64"), "{err}");
    }

    #[test]
    fn installs_downloaded_file_with_pinned_sha_including_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = zip_of(&[
            (&format!("ohs-x/{}", node_rel()), b"node"),
            ("ohs-x/app/dist/src/cli.js", b"cli"),
        ]);
        let bundle = Bundle {
            platform: "win32-x64",
            sha256: leak(sha(&archive)),
        };
        let file = tmp.path().join("ohs.zip");
        fs::write(&file, &archive).unwrap();
        let dest = tmp.path().join("ohs");
        install(&Source::Path(file), Some(&bundle), &dest, &mut Vec::new()).unwrap();
        assert!(dest.join(node_rel()).is_file());
    }

    #[test]
    fn rejects_file_not_matching_any_pin() {
        let tmp = tempfile::tempdir().unwrap();
        let bundle = pinned_for(&tar_gz("ohs-x"));
        let file = tmp.path().join("other.tar.gz");
        fs::write(&file, b"not the pinned package").unwrap();
        let err = install(
            &Source::Path(file),
            Some(&bundle),
            &tmp.path().join("ohs"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("không khớp gói OHS"), "{err}");
        assert!(err.contains(&bundle.url()), "{err}");
    }

    #[test]
    fn recognises_bundle_for_another_platform() {
        let all = [
            Bundle {
                sha256: "aa",
                ..BUNDLES[0]
            },
            Bundle {
                sha256: "bb",
                ..BUNDLES[3]
            },
        ];
        assert_eq!(other_platform("aa", &all[1], &all), Some("win32-x64"));
        assert_eq!(other_platform("bb", &all[0], &all), Some("linux-x64"));
        assert_eq!(other_platform("bb", &all[1], &all), None);
        assert_eq!(other_platform("dd", &all[0], &all), None);
    }

    #[test]
    fn installs_from_extracted_directory() {
        let tmp = tempfile::tempdir().unwrap();
        let src = tmp.path().join("src").join("ohs-x");
        fs::create_dir_all(src.join(Path::new(node_rel()).parent().unwrap())).unwrap();
        fs::write(src.join(node_rel()), "node").unwrap();
        fs::create_dir_all(src.join("app/dist/src")).unwrap();
        fs::write(src.join("app/dist/src/cli.js"), "cli").unwrap();
        let dest = tmp.path().join("ohs");
        // Thư mục cha chứa đúng một gói cũng được nhận.
        install(
            &Source::Path(tmp.path().join("src")),
            None,
            &dest,
            &mut Vec::new(),
        )
        .unwrap();
        assert!(dest.join("app/dist/src/cli.js").is_file());
    }

    #[test]
    fn rejects_directory_without_package() {
        let tmp = tempfile::tempdir().unwrap();
        fs::create_dir_all(tmp.path().join("empty")).unwrap();
        let err = install(
            &Source::Path(tmp.path().join("empty")),
            None,
            &tmp.path().join("ohs"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("không thấy gói OHS"), "{err}");
    }

    #[test]
    fn installs_from_https_url_checked_against_its_sha256_file() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = tar_gz("ohs-x");
        let url = "https://example.test/ohs-linux-x64.tar.gz";
        let fetch = served(vec![
            (url.to_owned(), archive.clone()),
            (
                format!("{url}.sha256"),
                format!("{}  ohs-linux-x64.tar.gz\n", sha(&archive).to_uppercase()).into_bytes(),
            ),
        ]);
        let dest = tmp.path().join("ohs");
        let mut log = Vec::new();
        // Không cần gói ghim: nguồn URL kiểm bằng file .sha256 của chính nó.
        super::install(&Source::Url(url.into()), None, &dest, &mut log, &fetch).unwrap();
        assert!(dest.join("app/dist/src/cli.js").is_file());
        assert!(String::from_utf8(log).unwrap().contains("khớp"));
    }

    #[test]
    fn rejects_url_package_with_wrong_or_missing_sha256() {
        let tmp = tempfile::tempdir().unwrap();
        let url = "https://example.test/ohs.tar.gz";
        let dest = tmp.path().join("ohs");
        let wrong = served(vec![
            (url.to_owned(), tar_gz("ohs-x")),
            (format!("{url}.sha256"), "0".repeat(64).into_bytes()),
        ]);
        let err = super::install(
            &Source::Url(url.into()),
            None,
            &dest,
            &mut Vec::new(),
            &wrong,
        )
        .unwrap_err();
        assert!(err.contains("không cài"), "{err}");
        let missing = served(vec![(url.to_owned(), tar_gz("ohs-x"))]);
        let err = super::install(
            &Source::Url(url.into()),
            None,
            &dest,
            &mut Vec::new(),
            &missing,
        )
        .unwrap_err();
        assert!(err.contains("<URL>.sha256"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn source_parse_accepts_https_urls_and_paths_only() {
        assert_eq!(
            Source::parse("https://h/x.zip"),
            Ok(Source::Url("https://h/x.zip".into()))
        );
        assert_eq!(
            Source::parse("D:\\ohs.zip"),
            Ok(Source::Path(PathBuf::from("D:\\ohs.zip")))
        );
        assert!(
            Source::parse("http://h/x.zip")
                .unwrap_err()
                .contains("https")
        );
        assert!(Source::parse("file:///x.zip").is_err());
    }

    #[test]
    fn pins_cover_every_platform_once_with_release_urls() {
        let mut names: Vec<_> = BUNDLES.iter().map(|b| b.platform).collect();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), 4);
        assert!(
            BUNDLES
                .iter()
                .all(|b| b.sha256.len() == 64 && b.sha256.bytes().all(|c| c.is_ascii_hexdigit()))
        );
        assert_eq!(
            BUNDLES[0].url(),
            "https://github.com/vantt/obsidian-hybrid-search/releases/download/v0.17.2-vantt.4/\
             ohs-0.17.2-vantt.4-win32-x64.zip"
        );
        assert_eq!(
            BUNDLES[3].archive_name(),
            "ohs-0.17.2-vantt.4-linux-x64.tar.gz"
        );
        assert_eq!(TAG, format!("v{VERSION}"));
    }

    /// Tải gói ghim của máy này từ bản phát hành thật bằng `curl` (không cần đăng nhập) rồi cài vào thư mục tạm.
    #[test]
    #[ignore = "tải khoảng 100 MB từ GitHub Release của fork"]
    fn downloads_and_installs_pinned_release_asset() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ohs");
        let mut log = Vec::new();
        let pkg =
            super::install(&Source::Download, Bundle::current(), &dest, &mut log, &curl).unwrap();
        assert!(pkg.node.is_file() && pkg.cli.is_file());
        assert!(pkg.pack.is_some(), "gói có lang/ohs-lang-vi");
        let v = Command::new(&pkg.node).arg("--version").output().unwrap();
        assert!(v.status.success(), "Node của gói chạy được");
        eprintln!("{}", String::from_utf8_lossy(&log));
    }
}
