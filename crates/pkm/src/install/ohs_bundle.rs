//! Cài gói OHS dựng sẵn của fork vào `<thư mục pkm>/ohs` (nơi `pkm_core::ohs::OhsPackage::default_dir` tìm).
//!
//! Nguồn ghim (docs/DECISIONS.md, "Gói OHS: ghim artifact của một run CI của fork"): artifact `bundle-<nền tảng>`
//! của run [`RUN_ID`] ở repo [`REPO`]. Mỗi artifact là một file zip chứa gói (`.tar.gz`, hoặc `.zip` cho Windows)
//! cùng file `.sha256` của gói. Kiểm hai lớp: sha256 của file zip artifact phải bằng digest GitHub ghi cho artifact
//! (ghim trong [`BUNDLES`]), rồi sha256 của gói bên trong phải bằng file `.sha256` đi kèm.
//!
//! Tải artifact của GitHub Actions cần đăng nhập GitHub, kể cả repo công khai. Có `gh` đã đăng nhập thì bộ cài tự tải;
//! nếu không, người dùng tải `bundle-<nền tảng>.zip` ở trang run (đã đăng nhập) rồi chỉ tới file bằng
//! `--ohs-package <file>`. Cũng nhận gói đã giải nén (thư mục) hoặc chính file gói khi biết sha256 ghim của nó.
//!
//! Đổi nguồn mà không cần bản `pkm` mới (ví dụ khi artifact ghim hết hạn): URL `https://…` của file gói, kèm
//! `<URL>.sha256` cùng chỗ (như asset của GitHub Release), qua `--ohs-package <URL>` hoặc biến [`PACKAGE_ENV`]. Gói
//! tải theo URL được kiểm với file `.sha256` đó, không cần khớp bản ghim.

use std::fs::{self, File};
use std::io::{self, BufReader, Read, Seek, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use pkm_core::ohs::OhsPackage;
use sha2::{Digest, Sha256};

/// Repo fork của OHS.
pub const REPO: &str = "vantt/obsidian-hybrid-search";
/// Run CI có artifact được ghim.
pub const RUN_ID: u64 = 41_398_112_909;
/// Phiên bản OHS của gói.
pub const VERSION: &str = "0.17.2-vantt.1";
/// Biến môi trường thay nguồn tự tải của bộ cài: đường dẫn (như `--ohs-package`) hoặc URL `https://` của file gói.
pub const PACKAGE_ENV: &str = "PKM_OHS_PACKAGE";

/// Một gói ghim cho một nền tảng.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bundle {
    /// Nền tảng theo tên của fork (`win32-x64`, `linux-x64`…).
    pub platform: &'static str,
    /// Id artifact `bundle-<platform>` của run [`RUN_ID`].
    pub artifact_id: u64,
    /// sha256 của file zip artifact (digest GitHub ghi cho artifact).
    pub artifact_sha256: &'static str,
    /// sha256 của file gói bên trong, khi đã đo được; dùng để nhận file gói trần.
    pub archive_sha256: Option<&'static str>,
}

/// Gói ghim của từng nền tảng. Artifact hết hạn ngày 2027-01-31; trước đó phải chuyển sang GitHub Release của fork.
pub const BUNDLES: [Bundle; 4] = [
    Bundle {
        platform: "win32-x64",
        artifact_id: 13_120_671_079,
        artifact_sha256: "f7c2ed058d1d8aa49b003d6e5802aa0c0b8ebb0e5f326b03edae6e80c6c428c9",
        archive_sha256: Some("02c277df69e599d1c804fb0715f6d1f5f1307fba7ccc93c102aed54c1c00dd8c"),
    },
    Bundle {
        platform: "darwin-arm64",
        artifact_id: 13_120_615_810,
        artifact_sha256: "6c306aea697d00e6287fbe1f62b172a397818d65f8a676364fa69e04c1abb63f",
        archive_sha256: None,
    },
    Bundle {
        platform: "darwin-x64",
        artifact_id: 13_119_819_922,
        artifact_sha256: "d8209d3c64ae6fbedbea0a02deba6287f6a570a66ac349eb9736a5c7512d8c07",
        archive_sha256: None,
    },
    Bundle {
        platform: "linux-x64",
        artifact_id: 13_120_251_817,
        artifact_sha256: "3df23d8bf0813117f154efd44cc9ba9f20b77a52205e06d4903a5efcf159df6d",
        archive_sha256: Some("9160d45e59c4ef5776568024a5b1f0f5b7643ffc64f2210db8af2d5705278b95"),
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

    /// Trang run trên GitHub, nơi tải artifact bằng tay.
    pub fn run_url() -> String {
        format!("https://github.com/{REPO}/actions/runs/{RUN_ID}")
    }

    /// Việc người dùng làm khi bộ cài không tự tải được.
    pub fn manual_action(&self) -> String {
        format!(
            "đăng nhập GitHub, tải artifact bundle-{} ở {} rồi chạy lại với --ohs-package <file zip vừa tải>",
            self.platform,
            Self::run_url()
        )
    }
}

/// Nguồn của gói khi cài.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Source {
    /// Tải artifact ghim bằng `gh`.
    Download,
    /// File zip artifact, file gói, hoặc thư mục gói đã giải nén.
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
/// gói ghim của máy này; tải và kiểm file cần nó, chép thư mục và nguồn URL thì không. `fetch(url, file)` tải nguồn
/// URL (bản thật: [`curl`]).
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
    let pinned = || bundle.ok_or("fork chưa có gói OHS dựng sẵn cho nền tảng này".to_owned());
    let root = match source {
        Source::Download => {
            let bundle = pinned()?;
            let zip = staging.path.join(format!("bundle-{}.zip", bundle.platform));
            let _ = writeln!(
                log,
                "tải artifact bundle-{} của run {RUN_ID} bằng gh…",
                bundle.platform
            );
            download(bundle, &zip)?;
            unpack_file(&zip, bundle, &staging.path, log)?
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

/// Tải artifact bằng `gh api` (đi theo chuyển hướng tới kho blob) vào `out`.
fn download(bundle: &Bundle, out: &Path) -> Result<(), String> {
    let file = File::create(out).map_err(|e| format!("tạo {}: {e}", out.display()))?;
    let api = format!("repos/{REPO}/actions/artifacts/{}/zip", bundle.artifact_id);
    let result = Command::new("gh")
        .args(["api", &api])
        .stdin(Stdio::null())
        .stdout(file)
        .stderr(Stdio::piped())
        .output();
    let output = match result {
        Ok(o) => o,
        Err(e) if e.kind() == io::ErrorKind::NotFound => {
            return Err(format!(
                "không có GitHub CLI (gh) để tải gói OHS; {}",
                bundle.manual_action()
            ));
        }
        Err(e) => return Err(format!("chạy gh: {e}")),
    };
    if !output.status.success() {
        let err = String::from_utf8_lossy(&output.stderr);
        return Err(format!(
            "gh không tải được artifact ({}); chạy `gh auth login` rồi thử lại, hoặc {}",
            err.trim(),
            bundle.manual_action()
        ));
    }
    Ok(())
}

/// Tải `url` bằng `curl` (có sẵn trên Windows 10 trở lên, macOS, Linux), chỉ qua https, kể cả khi chuyển hướng.
pub fn curl(url: &str, out: &Path) -> Result<(), String> {
    let output = Command::new("curl")
        .args(["--proto", "=https", "--tlsv1.2", "-fsSL", "--retry", "2"])
        .args(["--connect-timeout", "20", "-o"])
        .arg(out)
        .arg(url)
        .stdin(Stdio::null())
        .output()
        .map_err(|e| format!("không chạy được curl để tải {url}: {e}"))?;
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

/// Kiểm sha256 của `file` rồi giải nén vào `staging`; trả thư mục gốc của gói. Nhận file zip artifact (digest ghim)
/// hoặc file gói (sha256 ghim, khi có).
fn unpack_file(
    file: &Path,
    bundle: &Bundle,
    staging: &Path,
    log: &mut dyn Write,
) -> Result<PathBuf, String> {
    let digest = sha256_file(file).map_err(|e| format!("đọc {}: {e}", file.display()))?;
    if digest == bundle.artifact_sha256 {
        let _ = writeln!(log, "sha256 của artifact khớp bản ghim ({digest})");
        let archive = extract_from_artifact(file, bundle, staging)?;
        let _ = writeln!(log, "sha256 của gói khớp file .sha256 đi kèm");
        return unpack_archive(&archive, staging);
    }
    if bundle.archive_sha256 == Some(digest.as_str()) {
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
        "{}: sha256 {digest} không khớp gói OHS ghim cho {}; {}",
        file.display(),
        bundle.platform,
        bundle.manual_action()
    ))
}

/// Nền tảng khác `bundle` mà `digest` là gói ghim của nó (người dùng tải nhầm gói).
fn other_platform(digest: &str, bundle: &Bundle, all: &[Bundle]) -> Option<&'static str> {
    all.iter()
        .filter(|b| b.platform != bundle.platform)
        .find(|b| b.artifact_sha256 == digest || b.archive_sha256 == Some(digest))
        .map(|b| b.platform)
}

/// Lấy file gói và file `.sha256` từ zip artifact, kiểm khớp; trả đường dẫn file gói đã lấy ra.
fn extract_from_artifact(
    artifact: &Path,
    bundle: &Bundle,
    staging: &Path,
) -> Result<PathBuf, String> {
    let name = bundle.archive_name();
    let open = || -> Result<zip::ZipArchive<BufReader<File>>, String> {
        let f = File::open(artifact).map_err(|e| format!("mở {}: {e}", artifact.display()))?;
        zip::ZipArchive::new(BufReader::new(f))
            .map_err(|e| format!("{} không phải zip hợp lệ: {e}", artifact.display()))
    };
    let mut zip = open()?;
    let expected = {
        let mut entry = zip
            .by_name(&format!("{name}.sha256"))
            .map_err(|e| format!("artifact thiếu {name}.sha256: {e}"))?;
        let mut text = String::new();
        entry
            .read_to_string(&mut text)
            .map_err(|e| format!("đọc {name}.sha256: {e}"))?;
        text.split_whitespace()
            .next()
            .map(str::to_lowercase)
            .ok_or_else(|| format!("{name}.sha256 rỗng"))?
    };
    let out = staging.join(&name);
    let mut entry = zip
        .by_name(&name)
        .map_err(|e| format!("artifact thiếu {name}: {e}"))?;
    let mut file = File::create(&out).map_err(|e| format!("tạo {}: {e}", out.display()))?;
    let mut hasher = Sha256::new();
    let mut buf = vec![0u8; 1 << 16];
    loop {
        let n = entry
            .read(&mut buf)
            .map_err(|e| format!("giải nén {name}: {e}"))?;
        if n == 0 {
            break;
        }
        hasher.update(&buf[..n]);
        file.write_all(&buf[..n])
            .map_err(|e| format!("ghi {}: {e}", out.display()))?;
    }
    let got = hex(&hasher.finalize());
    if got != expected {
        return Err(format!(
            "sha256 của {name} là {got}, file .sha256 ghi {expected}: artifact hỏng"
        ));
    }
    Ok(out)
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

    /// Cài với `curl` thật (chỉ dùng cho nguồn không phải URL, hoặc test `#[ignore]`).
    fn install(
        source: &Source,
        bundle: Option<&Bundle>,
        dest: &Path,
        log: &mut dyn Write,
    ) -> Result<OhsPackage, String> {
        super::install(source, bundle, dest, log, &curl)
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

    /// Bundle thử cho Linux-như với artifact là zip chứa `archive` và `.sha256` (`sha_text` thay được để thử hỏng).
    fn artifact(archive: &[u8], sha_text: Option<String>) -> (Bundle, Vec<u8>) {
        let mut bundle = Bundle {
            platform: "linux-x64",
            artifact_id: 1,
            artifact_sha256: "",
            archive_sha256: None,
        };
        let name = bundle.archive_name();
        let sha_text = sha_text.unwrap_or_else(|| format!("{}  {name}\n", sha(archive)));
        let zip = zip_of(&[
            (name.as_str(), archive),
            (format!("{name}.sha256").as_str(), sha_text.as_bytes()),
        ]);
        bundle.artifact_sha256 = leak(sha(&zip));
        (bundle, zip)
    }

    #[test]
    fn installs_from_artifact_zip_and_replaces_old_package() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundle, zip) = artifact(&tar_gz("ohs-x"), None);
        let file = tmp.path().join("bundle.zip");
        fs::write(&file, zip).unwrap();
        let dest = tmp.path().join("pkm").join("ohs");
        fs::create_dir_all(dest.join("stale")).unwrap();
        let mut log = Vec::new();
        let pkg = install(&Source::Path(file), Some(&bundle), &dest, &mut log).unwrap();
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
    fn rejects_archive_whose_sha256_file_disagrees() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundle, zip) = artifact(&tar_gz("ohs-x"), Some(format!("{}  x\n", "0".repeat(64))));
        let file = tmp.path().join("bundle.zip");
        fs::write(&file, zip).unwrap();
        let dest = tmp.path().join("ohs");
        let err = install(&Source::Path(file), Some(&bundle), &dest, &mut Vec::new()).unwrap_err();
        assert!(err.contains("artifact hỏng"), "{err}");
        assert!(!dest.exists());
    }

    #[test]
    fn rejects_file_not_matching_any_pin() {
        let tmp = tempfile::tempdir().unwrap();
        let (bundle, _) = artifact(&tar_gz("ohs-x"), None);
        let file = tmp.path().join("other.zip");
        fs::write(&file, b"PK not the pinned artifact").unwrap();
        let err = install(
            &Source::Path(file),
            Some(&bundle),
            &tmp.path().join("ohs"),
            &mut Vec::new(),
        )
        .unwrap_err();
        assert!(err.contains("không khớp gói OHS ghim"), "{err}");
        assert!(err.contains(&Bundle::run_url()), "{err}");
    }

    #[test]
    fn installs_bare_archive_with_known_sha_including_zip() {
        let tmp = tempfile::tempdir().unwrap();
        let archive = zip_of(&[
            (&format!("ohs-x/{}", node_rel()), b"node"),
            ("ohs-x/app/dist/src/cli.js", b"cli"),
        ]);
        let bundle = Bundle {
            platform: "win32-x64",
            artifact_id: 1,
            artifact_sha256: "",
            archive_sha256: Some(leak(sha(&archive))),
        };
        let file = tmp.path().join("ohs.zip");
        fs::write(&file, &archive).unwrap();
        let dest = tmp.path().join("ohs");
        install(&Source::Path(file), Some(&bundle), &dest, &mut Vec::new()).unwrap();
        assert!(dest.join(node_rel()).is_file());
    }

    #[test]
    fn recognises_bundle_for_another_platform() {
        let all = [
            Bundle {
                artifact_sha256: "aa",
                ..BUNDLES[0]
            },
            Bundle {
                artifact_sha256: "bb",
                archive_sha256: Some("cc"),
                ..BUNDLES[3]
            },
        ];
        assert_eq!(other_platform("aa", &all[1], &all), Some("win32-x64"));
        assert_eq!(other_platform("cc", &all[0], &all), Some("linux-x64"));
        assert_eq!(other_platform("cc", &all[1], &all), None);
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
    fn pins_cover_every_platform_once() {
        let mut names: Vec<_> = BUNDLES.iter().map(|b| b.platform).collect();
        names.dedup();
        assert_eq!(names.len(), 4);
        assert!(BUNDLES.iter().all(|b| b.artifact_sha256.len() == 64));
        assert_eq!(
            BUNDLES[0].archive_name(),
            "ohs-0.17.2-vantt.1-win32-x64.zip"
        );
        assert_eq!(
            BUNDLES[3].archive_name(),
            "ohs-0.17.2-vantt.1-linux-x64.tar.gz"
        );
    }

    /// Cài từ artifact thật đã tải (`PKM_TEST_OHS_ARTIFACT=<bundle-<nền tảng>.zip>`), vào thư mục tạm.
    #[test]
    #[ignore = "cần file artifact thật khoảng 100 MB"]
    fn installs_real_pinned_artifact() {
        let file = PathBuf::from(
            std::env::var_os("PKM_TEST_OHS_ARTIFACT").expect("PKM_TEST_OHS_ARTIFACT"),
        );
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ohs");
        let bundle = Bundle::current().expect("fork có gói cho máy này");
        let mut log = Vec::new();
        let pkg = install(&Source::Path(file), Some(bundle), &dest, &mut log).unwrap();
        assert!(pkg.node.is_file() && pkg.cli.is_file());
        let v = Command::new(&pkg.node).arg("--version").output().unwrap();
        assert!(v.status.success(), "Node của gói chạy được");
        eprintln!("{}", String::from_utf8_lossy(&log));
    }

    /// Tải artifact ghim bằng `gh` đã đăng nhập rồi cài vào thư mục tạm.
    #[test]
    #[ignore = "tải khoảng 100 MB qua gh đã đăng nhập"]
    fn downloads_and_installs_pinned_artifact_with_gh() {
        let tmp = tempfile::tempdir().unwrap();
        let dest = tmp.path().join("ohs");
        let mut log = Vec::new();
        let pkg = install(&Source::Download, Bundle::current(), &dest, &mut log).unwrap();
        assert!(pkg.node.is_file() && pkg.cli.is_file());
        eprintln!("{}", String::from_utf8_lossy(&log));
    }
}
