//! Chỗ khác nhau giữa Windows, macOS và Linux mà lõi cần biết: thư mục của `pkm` cho mỗi người dùng, đổi tên file
//! không đè, trùng tên khác hoa thường hoặc khác dạng NFC/NFD, và thử lại khi chương trình khác đang giữ file.
//!
//! Đặc tả: phần không phải WSL của `scripts/platform_paths.py` (`cache_dir`, `name_taken`, `move`). Phần WSL
//! (dịch `/mnt/X`, liệt kê ổ Windows bằng PowerShell, chạy node qua `cmd.exe`) không cần vì binary chạy native.

use std::ffi::OsString;
use std::io;
use std::path::{Component, Path, PathBuf};
use std::thread;
use std::time::Duration;

use unicode_normalization::UnicodeNormalization;

/// Thư mục của `pkm` cho người dùng hiện tại, chứa cache và dữ liệu (index dựng sẵn, cấu hình của bộ cài):
/// `%LOCALAPPDATA%\pkm` trên Windows, `~/Library/Application Support/pkm` trên macOS,
/// `$XDG_CACHE_HOME/pkm` hoặc `~/.cache/pkm` trên Linux và các Unix khác.
pub fn pkm_dir() -> io::Result<PathBuf> {
    pkm_dir_from(&|key| std::env::var_os(key), std::env::home_dir())
}

/// Thư mục con `name` trong [`pkm_dir`], ví dụ `cache_dir("kg")`. Không tạo thư mục.
pub fn cache_dir(name: &str) -> io::Result<PathBuf> {
    Ok(pkm_dir()?.join(name))
}

/// [`pkm_dir`] với biến môi trường và thư mục home truyền vào, để test được mà không đổi môi trường của tiến trình.
/// Biến môi trường rỗng coi như không đặt (giống `os.environ.get(...) or ...` của Python).
fn pkm_dir_from(
    var: &dyn Fn(&str) -> Option<OsString>,
    home: Option<PathBuf>,
) -> io::Result<PathBuf> {
    let set = |key: &str| var(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    base_dir(&set, home)
        .map(|base| base.join("pkm"))
        .ok_or_else(|| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "không xác định được thư mục home của người dùng để đặt thư mục pkm",
            )
        })
}

#[cfg(windows)]
fn base_dir(set: &dyn Fn(&str) -> Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    set("LOCALAPPDATA").or_else(|| home.map(|h| h.join("AppData").join("Local")))
}

#[cfg(target_os = "macos")]
fn base_dir(_set: &dyn Fn(&str) -> Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    home.map(|h| h.join("Library").join("Application Support"))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn base_dir(set: &dyn Fn(&str) -> Option<PathBuf>, home: Option<PathBuf>) -> Option<PathBuf> {
    set("XDG_CACHE_HOME").or_else(|| home.map(|h| h.join(".cache")))
}

#[cfg(not(any(windows, unix)))]
fn base_dir(_set: &dyn Fn(&str) -> Option<PathBuf>, _home: Option<PathBuf>) -> Option<PathBuf> {
    None
}

/// File trong thư mục của `dst` có tên bằng tên của `dst` khi bỏ qua hoa thường và dạng Unicode (NFC/NFD), trừ chính
/// `src`. Trên Windows và macOS, `Ý tưởng.md` và `ý tưởng.md` là một file: ghi file thứ hai là đè file thứ nhất.
/// Thư mục chưa có thì trả `None`.
pub fn name_taken(dst: &Path, src: Option<&Path>) -> io::Result<Option<PathBuf>> {
    let dst = abspath(dst)?;
    let (Some(folder), Some(base)) = (dst.parent(), dst.file_name()) else {
        return Ok(None);
    };
    let key = name_key(&base.to_string_lossy());
    let own = src.map(abspath).transpose()?.map(|p| normcase(&p));
    let entries = match std::fs::read_dir(folder) {
        Ok(entries) => entries,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    for entry in entries {
        let name = entry?.file_name();
        if name_key(&name.to_string_lossy()) != key {
            continue;
        }
        let path = folder.join(&name);
        if own.as_ref() != Some(&normcase(&path)) {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

/// Số lần thử và thời gian chờ của [`move_file_with`]. Lần chờ thứ `i` (từ 1) dài `wait * i`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Retry {
    /// Tổng số lần thử đổi tên; 0 được tính như 1.
    pub tries: u32,
    pub wait: Duration,
}

impl Default for Retry {
    fn default() -> Self {
        Self {
            tries: 5,
            wait: Duration::from_millis(200),
        }
    }
}

/// Đổi tên `src` thành `dst` không bao giờ đè, thử lại khi chương trình khác đang giữ file (Obsidian, OneDrive,
/// phần mềm diệt virus trên Windows). Xem [`move_file_with`].
pub fn move_file(src: &Path, dst: &Path) -> io::Result<()> {
    move_file_with(src, dst, Retry::default())
}

/// Như [`move_file`] với số lần thử tuỳ chọn.
///
/// Lỗi `AlreadyExists` (thông điệp nêu file đang giữ tên) khi một file khác `src` đã có tên của `dst`, kể cả khác hoa
/// thường hoặc khác NFC/NFD; đổi hoa thường của chính `src` thì được. Lỗi "file bị giữ" sau lần thử cuối được trả
/// nguyên; lỗi khác trả ngay.
pub fn move_file_with(src: &Path, dst: &Path, retry: Retry) -> io::Result<()> {
    move_using(src, dst, retry, &mut rename_no_replace)
}

fn move_using(
    src: &Path,
    dst: &Path,
    retry: Retry,
    rename: &mut dyn FnMut(&Path, &Path) -> io::Result<()>,
) -> io::Result<()> {
    if let Some(clash) = name_taken(dst, Some(src))? {
        return Err(io::Error::new(
            io::ErrorKind::AlreadyExists,
            format!("{}: already taken by {}", dst.display(), clash.display()),
        ));
    }
    let tries = retry.tries.max(1);
    for attempt in 1..=tries {
        match rename(src, dst) {
            Ok(()) => return Ok(()),
            Err(e) if is_held(&e) && attempt < tries => thread::sleep(retry.wait * attempt),
            Err(e) => return Err(e),
        }
    }
    unreachable!("vòng lặp luôn trả về ở lần thử cuối")
}

/// Lỗi khi chương trình khác giữ file. Windows báo `ERROR_ACCESS_DENIED`, `ERROR_SHARING_VIOLATION` (32) hoặc
/// `ERROR_LOCK_VIOLATION` (33); Python gộp cả ba vào `PermissionError`.
fn is_held(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::PermissionDenied
        || (cfg!(windows) && matches!(e.raw_os_error(), Some(32 | 33)))
}

/// `rename` của Windows (`MoveFileExW` không cờ, như `os.rename` của Python): báo lỗi khi đích đã có thay vì đè.
/// `std::fs::rename` trên Windows lại đè đích.
#[cfg(windows)]
fn rename_no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    use windows_sys::Win32::Storage::FileSystem::MoveFileExW;
    let (src, dst) = (verbatim_wide(src)?, verbatim_wide(dst)?);
    // SAFETY: hai con trỏ trỏ tới chuỗi UTF-16 kết thúc bằng 0, sống tới hết lời gọi.
    if unsafe { MoveFileExW(src.as_ptr(), dst.as_ptr(), 0) } == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// Đường dẫn tuyệt đối dạng `\\?\` (UTF-16, kết thúc bằng 0) để đường dẫn dài hơn 260 ký tự vẫn đổi tên được.
#[cfg(windows)]
fn verbatim_wide(p: &Path) -> io::Result<Vec<u16>> {
    use std::os::windows::ffi::OsStrExt;
    let abs: Vec<u16> = std::path::absolute(p)?.as_os_str().encode_wide().collect();
    let prefix = |s: &str| s.encode_utf16().collect::<Vec<u16>>();
    let mut out = if abs.starts_with(&prefix(r"\\?\")) || abs.starts_with(&prefix(r"\\.\")) {
        abs
    } else if abs.starts_with(&prefix(r"\\")) {
        [prefix(r"\\?\UNC\"), abs[2..].to_vec()].concat()
    } else {
        [prefix(r"\\?\"), abs].concat()
    };
    out.push(0);
    Ok(out)
}

/// Trên macOS và Linux, `rename` đè đích; [`name_taken`] đã kiểm trước (như Python).
#[cfg(not(windows))]
fn rename_no_replace(src: &Path, dst: &Path) -> io::Result<()> {
    std::fs::rename(src, dst)
}

/// Khoá so tên file: NFC rồi casefold (như `unicodedata.normalize('NFC', n).casefold()`).
fn name_key(name: &str) -> String {
    caseless::default_case_fold_str(&name.nfc().collect::<String>())
}

/// Như `os.path.abspath`: nối vào thư mục hiện tại và bỏ `.`, `..` theo chữ (không đi theo symlink).
fn abspath(p: &Path) -> io::Result<PathBuf> {
    let abs = std::path::absolute(p)?;
    let mut out = PathBuf::new();
    for c in abs.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other),
        }
    }
    Ok(out)
}

/// Như `os.path.normcase`: Windows so đường dẫn không phân biệt hoa thường và coi `/` là `\`; nơi khác giữ nguyên.
fn normcase(p: &Path) -> OsString {
    if cfg!(windows) {
        p.to_string_lossy().replace('/', "\\").to_lowercase().into()
    } else {
        p.as_os_str().to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;
    use std::fs;
    use std::path::PathBuf;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> {
        let pairs: Vec<(String, OsString)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), (*v).into()))
            .collect();
        move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[cfg(windows)]
    #[test]
    fn cache_dir_windows_uses_localappdata() {
        let local = r"C:\Users\u\AppData\Local";
        let got = pkm_dir_from(
            &vars(&[("LOCALAPPDATA", local)]),
            Some(PathBuf::from(r"C:\Users\u")),
        )
        .unwrap();
        assert_eq!(got.join("kg"), Path::new(local).join("pkm").join("kg"));
        let fallback = pkm_dir_from(
            &vars(&[("LOCALAPPDATA", "")]),
            Some(PathBuf::from(r"C:\Users\u")),
        )
        .unwrap();
        assert_eq!(fallback, Path::new(r"C:\Users\u\AppData\Local\pkm"));
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn cache_dir_macos_uses_application_support() {
        let got = pkm_dir_from(
            &vars(&[("XDG_CACHE_HOME", "/tmp/c")]),
            Some(PathBuf::from("/Users/u")),
        )
        .unwrap();
        assert_eq!(
            got.join("kg"),
            Path::new("/Users/u/Library/Application Support/pkm/kg")
        );
    }

    #[cfg(all(unix, not(target_os = "macos")))]
    #[test]
    fn cache_dir_linux_uses_xdg_cache_home() {
        let home = Some(PathBuf::from("/home/u"));
        let got = pkm_dir_from(&vars(&[("XDG_CACHE_HOME", "/tmp/c")]), home.clone()).unwrap();
        assert_eq!(got.join("kg"), Path::new("/tmp/c/pkm/kg"));
        let fallback = pkm_dir_from(&vars(&[("XDG_CACHE_HOME", "")]), home).unwrap();
        assert_eq!(fallback, Path::new("/home/u/.cache/pkm"));
    }

    #[test]
    fn cache_dir_without_home_is_an_error() {
        let err = pkm_dir_from(&vars(&[]), None).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::NotFound);
    }

    #[test]
    fn cache_dir_of_this_user_ends_in_pkm_name() {
        let got = cache_dir("kg").unwrap();
        assert!(got.ends_with(Path::new("pkm").join("kg")), "{got:?}");
    }

    struct Dir(tempfile::TempDir);

    impl Dir {
        fn new() -> Self {
            Self(tempfile::tempdir().unwrap())
        }

        fn path(&self, name: &str) -> PathBuf {
            self.0.path().join(name)
        }

        fn touch(&self, name: &str, text: &str) -> PathBuf {
            let p = self.path(name);
            fs::write(&p, text).unwrap();
            p
        }

        fn names(&self) -> Vec<String> {
            let mut names: Vec<String> = fs::read_dir(self.0.path())
                .unwrap()
                .map(|e| e.unwrap().file_name().to_string_lossy().into_owned())
                .collect();
            names.sort();
            names
        }
    }

    fn nfd(s: &str) -> String {
        s.nfd().collect()
    }

    #[test]
    fn move_never_overwrites() {
        let d = Dir::new();
        let (a, b) = (d.touch("a.md", "A"), d.touch("b.md", "B"));
        let err = move_file(&a, &b).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(err.to_string().contains("already taken by"), "{err}");
        assert_eq!(fs::read_to_string(&b).unwrap(), "B");
        assert_eq!(fs::read_to_string(&a).unwrap(), "A");
    }

    #[test]
    fn move_refuses_name_differing_only_in_case_or_unicode_form() {
        let d = Dir::new();
        let a = d.touch("a.md", "x");
        d.touch(&nfd("Ý tưởng.md"), "x");
        let err = move_file(&a, &d.path("ý TƯỞNG.md")).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::AlreadyExists);
        assert!(a.exists());
    }

    #[test]
    fn move_allows_case_only_rename_of_itself() {
        let d = Dir::new();
        let a = d.touch("ghi chú.md", "x");
        move_file(&a, &d.path("Ghi chú.md")).unwrap();
        assert_eq!(d.names(), ["Ghi chú.md"]);
    }

    #[test]
    fn move_retries_while_file_is_held() {
        let d = Dir::new();
        let a = d.touch("a.md", "x");
        let calls = Cell::new(0);
        let mut held_twice = |src: &Path, dst: &Path| {
            calls.set(calls.get() + 1);
            if calls.get() < 3 {
                return Err(io::Error::new(
                    io::ErrorKind::PermissionDenied,
                    "held by another program",
                ));
            }
            fs::rename(src, dst)
        };
        let retry = Retry {
            tries: 5,
            wait: Duration::ZERO,
        };
        move_using(&a, &d.path("b.md"), retry, &mut held_twice).unwrap();
        assert_eq!((calls.get(), d.names()), (3, vec!["b.md".to_string()]));
    }

    #[test]
    fn move_gives_up_after_tries() {
        let d = Dir::new();
        let a = d.touch("a.md", "x");
        let calls = Cell::new(0);
        let mut held = |_: &Path, _: &Path| {
            calls.set(calls.get() + 1);
            Err(io::Error::new(io::ErrorKind::PermissionDenied, "held"))
        };
        let retry = Retry {
            tries: 2,
            wait: Duration::ZERO,
        };
        let err = move_using(&a, &d.path("b.md"), retry, &mut held).unwrap_err();
        assert_eq!(
            (err.kind(), calls.get()),
            (io::ErrorKind::PermissionDenied, 2)
        );
        assert!(a.exists());
    }

    #[test]
    fn move_does_not_retry_other_errors() {
        let d = Dir::new();
        let calls = Cell::new(0);
        let mut missing = |_: &Path, _: &Path| {
            calls.set(calls.get() + 1);
            Err(io::Error::new(io::ErrorKind::NotFound, "gone"))
        };
        let err = move_using(
            &d.path("a.md"),
            &d.path("b.md"),
            Retry::default(),
            &mut missing,
        )
        .unwrap_err();
        assert_eq!((err.kind(), calls.get()), (io::ErrorKind::NotFound, 1));
    }

    #[test]
    fn move_with_zero_tries_still_tries_once() {
        let d = Dir::new();
        let a = d.touch("a.md", "x");
        move_file_with(
            &a,
            &d.path("b.md"),
            Retry {
                tries: 0,
                wait: Duration::ZERO,
            },
        )
        .unwrap();
        assert_eq!(d.names(), ["b.md"]);
    }

    #[test]
    fn move_renames_into_vietnamese_name() {
        let d = Dir::new();
        let a = d.touch("a.md", "nội dung");
        let dst = d.path("Bài viết mới.md");
        move_file(&a, &dst).unwrap();
        assert_eq!(fs::read_to_string(&dst).unwrap(), "nội dung");
        assert!(!a.exists());
    }

    #[test]
    fn name_taken_finds_case_and_form_variants_but_not_itself() {
        let d = Dir::new();
        let held = d.touch(&nfd("Ý tưởng.md"), "x");
        let got = name_taken(&d.path("ý TƯỞNG.md"), None).unwrap().unwrap();
        assert_eq!(got.file_name(), held.file_name());
        assert_eq!(
            name_taken(&d.path("ý TƯỞNG.md"), Some(&held)).unwrap(),
            None
        );
        assert_eq!(name_taken(&d.path("khác.md"), None).unwrap(), None);
    }

    #[test]
    fn name_taken_in_missing_folder_is_none() {
        let d = Dir::new();
        assert_eq!(
            name_taken(&d.path("không có").join("a.md"), None).unwrap(),
            None
        );
    }

    #[test]
    fn name_taken_resolves_dot_segments_of_src() {
        let d = Dir::new();
        fs::create_dir(d.path("x")).unwrap();
        let a = d.touch("a.md", "x");
        let src = d.path("x").join("..").join("a.md");
        assert_eq!(name_taken(&d.path("A.md"), Some(&src)).unwrap(), None);
        assert_eq!(
            name_taken(&d.path("A.md"), None)
                .unwrap()
                .unwrap()
                .file_name(),
            a.file_name()
        );
    }

    #[test]
    fn name_key_folds_case_and_unicode_form() {
        assert_eq!(name_key(&nfd("Ý TƯỞNG.md")), name_key("ý tưởng.md"));
        assert_ne!(name_key("y tuong.md"), name_key("ý tưởng.md"));
    }

    #[cfg(windows)]
    #[test]
    fn sharing_violation_counts_as_held() {
        assert!(is_held(&io::Error::from_raw_os_error(32)));
        assert!(is_held(&io::Error::from_raw_os_error(33)));
        assert!(is_held(&io::Error::from_raw_os_error(5)));
    }

    #[cfg(windows)]
    #[test]
    fn verbatim_prefixes_drive_and_unc_paths() {
        let text = |p: &str| String::from_utf16(&verbatim_wide(Path::new(p)).unwrap()).unwrap();
        assert_eq!(text(r"C:\obs\a.md"), "\\\\?\\C:\\obs\\a.md\0");
        assert_eq!(text(r"\\srv\share\a.md"), "\\\\?\\UNC\\srv\\share\\a.md\0");
        assert_eq!(text(r"\\?\C:\a.md"), "\\\\?\\C:\\a.md\0");
    }
}
