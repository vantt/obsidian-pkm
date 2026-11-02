//! File cấu hình của Claude Desktop (`claude_desktop_config.json`): tìm file, đọc, sửa đúng mục `kg` trong
//! `mcpServers`, ghi nguyên tử kèm bản sao lưu.
//!
//! Đặc tả: `scripts/kg/desktop_config.py` (`default_config`, phần ghi file ở cuối). Giữ nguyên mọi khoá và mục khác
//! (gồm MCP `ohs`) theo đúng thứ tự, giữ CRLF nếu file đang dùng CRLF, thụt lề 2 và không thoát ký tự ngoài ASCII
//! như `json.dumps(indent=2, ensure_ascii=False)`.

use std::ffi::OsString;
use std::fs;
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{SystemTime, UNIX_EPOCH};

use serde_json::{Map, Value};

/// Tên MCP của `pkm` trong `mcpServers`. Giữ `kg` để Project instructions cũ vẫn đúng (PRD Q2).
pub const SERVER_NAME: &str = "kg";

/// Tên file cấu hình của Claude Desktop.
pub const CONFIG_FILE: &str = "claude_desktop_config.json";

/// Hệ điều hành quyết định chỗ đặt cấu hình. Tách khỏi `cfg` để test được cả ba trên một máy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Os {
    Windows,
    Mac,
    /// Linux và Unix khác: Claude Desktop không có bản chính thức; bản cộng đồng dùng `~/.config/Claude`.
    Other,
}

impl Os {
    pub fn current() -> Self {
        if cfg!(windows) {
            Self::Windows
        } else if cfg!(target_os = "macos") {
            Self::Mac
        } else {
            Self::Other
        }
    }
}

/// Thư mục có thể chứa cấu hình Claude Desktop, theo thứ tự thử. Bản Microsoft Store ảo hoá `%APPDATA%` vào
/// `%LOCALAPPDATA%\Packages\Claude_<id>\LocalCache\Roaming\Claude`.
pub fn candidate_dirs(
    os: Os,
    var: &dyn Fn(&str) -> Option<OsString>,
    home: Option<&Path>,
) -> Vec<PathBuf> {
    let set = |key: &str| var(key).filter(|v| !v.is_empty()).map(PathBuf::from);
    match os {
        Os::Windows => {
            let mut dirs = Vec::new();
            let appdata =
                set("APPDATA").or_else(|| home.map(|h| h.join("AppData").join("Roaming")));
            if let Some(appdata) = appdata {
                dirs.push(appdata.join("Claude"));
            }
            let local =
                set("LOCALAPPDATA").or_else(|| home.map(|h| h.join("AppData").join("Local")));
            if let Some(local) = local {
                dirs.extend(store_dirs(&local.join("Packages")));
            }
            dirs
        }
        Os::Mac => home
            .map(|h| h.join("Library").join("Application Support").join("Claude"))
            .into_iter()
            .collect(),
        Os::Other => set("XDG_CONFIG_HOME")
            .or_else(|| home.map(|h| h.join(".config")))
            .map(|c| c.join("Claude"))
            .into_iter()
            .collect(),
    }
}

/// `Packages\Claude_*\LocalCache\Roaming\Claude` (như `glob` của Python), sắp theo tên để thứ tự ổn định.
fn store_dirs(packages: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(packages) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|e| e.file_name().to_string_lossy().starts_with("Claude_"))
        .map(|e| e.path().join("LocalCache").join("Roaming").join("Claude"))
        .collect();
    dirs.sort();
    dirs
}

/// File cấu hình của máy này: `explicit` (cờ `--config`) nếu có, nếu không thì tìm trong [`candidate_dirs`].
pub fn locate(explicit: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = explicit {
        return std::path::absolute(path).map_err(|e| format!("{}: {e}", path.display()));
    }
    let home = std::env::home_dir();
    let dirs = candidate_dirs(Os::current(), &|k| std::env::var_os(k), home.as_deref());
    locate_in(&dirs)
}

/// Chọn file cấu hình trong `dirs`. Đúng một file có sẵn thì dùng file đó; nhiều file thì bắt chọn bằng `--config`
/// như Python. Khác Python: chưa có file nào nhưng đúng một thư mục Claude có sẵn (Claude Desktop đã chạy nhưng
/// chưa ai sửa cấu hình) thì tạo file mới trong thư mục đó.
pub fn locate_in(dirs: &[PathBuf]) -> Result<PathBuf, String> {
    let files: Vec<PathBuf> = dirs
        .iter()
        .map(|d| d.join(CONFIG_FILE))
        .filter(|f| f.is_file())
        .collect();
    let tried = || {
        dirs.iter()
            .map(|d| d.display().to_string())
            .collect::<Vec<_>>()
            .join(", ")
    };
    match files.as_slice() {
        [one] => return Ok(one.clone()),
        [] => {}
        many => {
            let list: Vec<String> = many.iter().map(|p| p.display().to_string()).collect();
            return Err(format!(
                "thấy {} file cấu hình Claude Desktop ({}); chỉ rõ file cần sửa bằng --config <file>",
                many.len(),
                list.join(", ")
            ));
        }
    }
    let existing: Vec<&PathBuf> = dirs.iter().filter(|d| d.is_dir()).collect();
    match existing.as_slice() {
        [one] => Ok(one.join(CONFIG_FILE)),
        [] => Err(format!(
            "không thấy Claude Desktop trên máy này (đã thử: {}). Cài và mở Claude Desktop một lần, hoặc chỉ file \
             cấu hình bằng --config <file>",
            if dirs.is_empty() {
                "không có thư mục nào".to_owned()
            } else {
                tried()
            }
        )),
        many => Err(format!(
            "thấy {} thư mục Claude Desktop nhưng chưa có file cấu hình ({}); chỉ rõ file bằng --config <file>",
            many.len(),
            many.iter()
                .map(|p| p.display().to_string())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// Mục của một MCP server trong `mcpServers`: Claude Desktop chạy `command` với `args` (và `env`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerEntry {
    pub command: String,
    pub args: Vec<String>,
    pub env: Vec<(String, String)>,
}

impl ServerEntry {
    /// Dạng JSON trong cấu hình: `{"command", "args"}`, thêm `"env"` khi có biến môi trường.
    pub fn to_value(&self) -> Value {
        let mut obj = Map::new();
        obj.insert("command".into(), Value::String(self.command.clone()));
        obj.insert(
            "args".into(),
            Value::Array(self.args.iter().cloned().map(Value::String).collect()),
        );
        if !self.env.is_empty() {
            let env = self
                .env
                .iter()
                .map(|(k, v)| (k.clone(), Value::String(v.clone())))
                .collect();
            obj.insert("env".into(), Value::Object(env));
        }
        Value::Object(obj)
    }

    /// Đọc một mục từ cấu hình; báo lỗi khi sai kiểu (Claude Desktop cũng không chạy được mục đó).
    pub fn from_value(value: &Value) -> Result<Self, String> {
        let obj = value
            .as_object()
            .ok_or("mục không phải object JSON".to_owned())?;
        let command = obj
            .get("command")
            .and_then(Value::as_str)
            .filter(|c| !c.is_empty())
            .ok_or("mục thiếu `command` (chuỗi)".to_owned())?
            .to_owned();
        let args = match obj.get("args") {
            None => Vec::new(),
            Some(Value::Array(items)) => items
                .iter()
                .map(|a| a.as_str().map(str::to_owned))
                .collect::<Option<Vec<_>>>()
                .ok_or("`args` có phần tử không phải chuỗi".to_owned())?,
            Some(_) => return Err("`args` không phải danh sách".to_owned()),
        };
        let env = match obj.get("env") {
            None => Vec::new(),
            Some(Value::Object(env)) => env
                .iter()
                .map(|(k, v)| v.as_str().map(|v| (k.clone(), v.to_owned())))
                .collect::<Option<Vec<_>>>()
                .ok_or("`env` có giá trị không phải chuỗi".to_owned())?,
            Some(_) => return Err("`env` không phải object".to_owned()),
        };
        Ok(Self { command, args, env })
    }
}

/// Nội dung file cấu hình đã đọc.
#[derive(Debug, Clone)]
pub struct ConfigDoc {
    /// Gốc JSON, giữ thứ tự khoá.
    pub root: Map<String, Value>,
    /// File có sẵn trên đĩa (không thì sẽ tạo mới, không cần sao lưu).
    pub exists: bool,
    /// File dùng CRLF: ghi lại cũng dùng CRLF.
    pub crlf: bool,
}

impl ConfigDoc {
    /// Đọc `path`. File chưa có hoặc chỉ có khoảng trắng thì coi như `{}`. Bỏ BOM UTF-8 (như `utf-8-sig`).
    pub fn load(path: &Path) -> Result<Self, String> {
        let raw = match fs::read(path) {
            Ok(raw) => raw,
            Err(e) if e.kind() == io::ErrorKind::NotFound => {
                return Ok(Self {
                    root: Map::new(),
                    exists: false,
                    crlf: false,
                });
            }
            Err(e) => return Err(format!("đọc {}: {e}", path.display())),
        };
        let text =
            String::from_utf8(raw).map_err(|_| format!("{} không phải UTF-8", path.display()))?;
        let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
        let crlf = text.contains("\r\n");
        if text.trim().is_empty() {
            return Ok(Self {
                root: Map::new(),
                exists: true,
                crlf,
            });
        }
        match serde_json::from_str::<Value>(text) {
            Ok(Value::Object(root)) => Ok(Self {
                root,
                exists: true,
                crlf,
            }),
            Ok(_) => Err(format!(
                "{}: gốc JSON không phải object; sửa tay file này trước",
                path.display()
            )),
            Err(e) => Err(format!(
                "{} không phải JSON hợp lệ ({e}); sửa tay file này trước",
                path.display()
            )),
        }
    }

    /// Mục `name` trong `mcpServers`, nếu có.
    pub fn server(&self, name: &str) -> Option<&Value> {
        self.root.get("mcpServers")?.as_object()?.get(name)
    }

    /// Tên các server trong `mcpServers`, theo thứ tự trong file.
    pub fn server_names(&self) -> Vec<String> {
        self.root
            .get("mcpServers")
            .and_then(Value::as_object)
            .map(|s| s.keys().cloned().collect())
            .unwrap_or_default()
    }

    /// Đặt (`Some`) hoặc xoá (`None`) mục `name` trong `mcpServers`, giữ nguyên mọi thứ khác. `mcpServers` không
    /// phải object thì thay bằng object mới như Python (file cũ còn trong bản sao lưu). Trả `true` khi nội dung đổi.
    pub fn set_server(&mut self, name: &str, entry: Option<Value>) -> bool {
        if entry.is_none() && self.server(name).is_none() {
            return false;
        }
        let servers = self
            .root
            .entry("mcpServers")
            .or_insert_with(|| Value::Object(Map::new()));
        if !servers.is_object() {
            *servers = Value::Object(Map::new());
        }
        let Value::Object(servers) = servers else {
            unreachable!("vừa đặt mcpServers thành object")
        };
        match entry {
            Some(entry) => {
                let changed = servers.get(name) != Some(&entry);
                servers.insert(name.to_owned(), entry);
                changed
            }
            None => servers.shift_remove(name).is_some(),
        }
    }

    /// Văn bản sẽ ghi: thụt lề 2, giữ ký tự ngoài ASCII, CRLF nếu file cũ dùng CRLF, không có dòng trống cuối file
    /// (như `json.dumps` của Python).
    pub fn render(&self) -> String {
        let text = serde_json::to_string_pretty(&Value::Object(self.root.clone()))
            .expect("Map<String, Value> luôn chuyển được thành JSON");
        if self.crlf {
            text.replace('\n', "\r\n")
        } else {
            text
        }
    }
}

/// Dòng mô tả thay đổi của mục `name`: `-` cho bản cũ, `+` cho bản mới, dạng JSON thụt lề như trong file.
pub fn entry_diff(name: &str, before: Option<&Value>, after: Option<&Value>) -> String {
    let block = |v: &Value| {
        let mut one = Map::new();
        one.insert(name.to_owned(), v.clone());
        serde_json::to_string_pretty(&Value::Object(one))
            .expect("Value luôn chuyển được thành JSON")
    };
    let mut out = String::new();
    if before == after {
        out.push_str(&format!("  (mục `{name}` không đổi)\n"));
        return out;
    }
    for (sign, value) in [('-', before), ('+', after)] {
        if let Some(v) = value {
            for line in block(v).lines() {
                out.push(sign);
                out.push(' ');
                out.push_str(line);
                out.push('\n');
            }
        }
    }
    out
}

/// Ghi `text` vào `path` nguyên tử: sao lưu file cũ thành `<file>.bak-<thời điểm UTC>`, ghi file tạm mới tạo (tên
/// ngẫu nhiên, không bao giờ mở file có sẵn) cùng thư mục với file đích, `fsync`, rồi đổi tên đè lên file đích. `path`
/// là symlink (cấu hình đồng bộ bằng dotfiles) thì ghi vào file nó trỏ tới và giữ nguyên symlink. Trên Unix, file
/// mới giữ quyền của file cũ (ví dụ `0600` khi `env` của MCP khác có token). Trả đường dẫn bản sao lưu (nếu có file
/// cũ).
pub fn write_atomic(path: &Path, text: &str) -> Result<Option<PathBuf>, String> {
    let target = write_target(path)?;
    let dir = target
        .parent()
        .filter(|d| !d.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !dir.is_dir() {
        return Err(format!("thư mục {} không tồn tại", dir.display()));
    }
    let original = fs::metadata(&target).ok();
    let backup = if original.is_some() {
        let bak = backup_path(path, SystemTime::now());
        fs::copy(&target, &bak).map_err(|e| format!("sao lưu {}: {e}", path.display()))?;
        Some(bak)
    } else {
        None
    };
    let name = target.file_name().map_or_else(
        || OsString::from(CONFIG_FILE),
        std::ffi::OsStr::to_os_string,
    );
    let (mut f, tmp) = create_temp(dir, &name, original.is_some())
        .map_err(|e| format!("tạo file tạm trong {}: {e}", dir.display()))?;
    let result = (|| -> io::Result<()> {
        f.write_all(text.as_bytes())?;
        #[cfg(unix)]
        if let Some(m) = &original {
            f.set_permissions(m.permissions())?;
        }
        f.sync_all()?;
        drop(f);
        fs::rename(&tmp, &target)
    })();
    if let Err(e) = result {
        let _ = fs::remove_file(&tmp);
        return Err(format!("ghi {}: {e}", path.display()));
    }
    Ok(backup)
}

/// File thật cần ghi: `path`, hoặc file mà `path` (symlink) trỏ tới. Symlink gãy là lỗi: ghi đè nó sẽ thay symlink
/// bằng file thường và cắt đứt đồng bộ mà không ai biết.
fn write_target(path: &Path) -> Result<PathBuf, String> {
    match fs::symlink_metadata(path) {
        Ok(m) if m.file_type().is_symlink() => fs::canonicalize(path).map_err(|e| {
            format!(
                "{} là symlink nhưng không theo được tới file đích ({e}); sửa symlink rồi chạy lại",
                path.display()
            )
        }),
        _ => Ok(path.to_path_buf()),
    }
}

/// Tạo mới (không mở file có sẵn, không theo symlink có sẵn) `<name>.tmp-<pid>-<ngẫu nhiên>` trong `dir`. `private`:
/// trên Unix tạo với quyền `0600` (quyền của file cũ được chép sang sau), để nội dung không lộ trong lúc ghi.
fn create_temp(
    dir: &Path,
    name: &std::ffi::OsStr,
    private: bool,
) -> io::Result<(fs::File, PathBuf)> {
    let mut last = None;
    for _ in 0..16 {
        let mut tmp_name = name.to_os_string();
        tmp_name.push(format!(".tmp-{}-{:016x}", std::process::id(), random_u64()));
        let tmp = dir.join(tmp_name);
        let mut opts = fs::OpenOptions::new();
        opts.write(true).create_new(true);
        #[cfg(unix)]
        if private {
            use std::os::unix::fs::OpenOptionsExt;
            opts.mode(0o600);
        }
        #[cfg(not(unix))]
        let _ = private;
        match opts.open(&tmp) {
            Ok(f) => return Ok((f, tmp)),
            Err(e) if e.kind() == io::ErrorKind::AlreadyExists => last = Some(e),
            Err(e) => return Err(e),
        }
    }
    Err(last.unwrap_or_else(|| io::Error::other("không tạo được tên file tạm")))
}

/// Số ngẫu nhiên cho tên file tạm: khoá ngẫu nhiên của `RandomState` trộn với thời điểm hiện tại.
fn random_u64() -> u64 {
    use std::hash::{BuildHasher, Hasher};
    let mut h = std::collections::hash_map::RandomState::new().build_hasher();
    h.write_u128(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos()),
    );
    h.finish()
}

/// `<file>.bak-YYYYMMDDTHHMMSSZ`, thêm `-2`, `-3`… nếu trùng.
fn backup_path(path: &Path, now: SystemTime) -> PathBuf {
    let stamp = utc_stamp(now);
    let base = {
        let mut s = path.as_os_str().to_owned();
        s.push(format!(".bak-{stamp}"));
        PathBuf::from(s)
    };
    if !base.exists() {
        return base;
    }
    (2u32..)
        .map(|n| {
            let mut s = base.as_os_str().to_owned();
            s.push(format!("-{n}"));
            PathBuf::from(s)
        })
        .find(|p| !p.exists())
        .expect("luôn có tên chưa dùng")
}

/// Thời điểm UTC dạng `YYYYMMDDTHHMMSSZ`.
pub fn utc_stamp(t: SystemTime) -> String {
    let secs = t.duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
    let (days, rem) = (secs / 86_400, secs % 86_400);
    let (y, m, d) = civil_from_days(days as i64);
    format!(
        "{y:04}{m:02}{d:02}T{:02}{:02}{:02}Z",
        rem / 3600,
        rem % 3600 / 60,
        rem % 60
    )
}

/// Ngày lịch Gregory từ số ngày kể từ 1970-01-01 (thuật toán `civil_from_days` của Howard Hinnant).
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    let y = yoe + era * 400 + i64::from(m <= 2);
    (y, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn vars(pairs: &[(&str, &str)]) -> impl Fn(&str) -> Option<OsString> + use<> {
        let pairs: Vec<(String, OsString)> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), (*v).into()))
            .collect();
        move |key| pairs.iter().find(|(k, _)| k == key).map(|(_, v)| v.clone())
    }

    #[test]
    fn windows_candidates_include_appdata_and_store_package() {
        let tmp = tempfile::tempdir().unwrap();
        let local = tmp.path().join("Local");
        let store = local.join("Packages").join("Claude_pzs8sxrjxfjjc");
        fs::create_dir_all(&store).unwrap();
        fs::create_dir_all(local.join("Packages").join("Other_1")).unwrap();
        let roaming = tmp.path().join("Roaming");
        let env = vars(&[
            ("APPDATA", roaming.to_str().unwrap()),
            ("LOCALAPPDATA", local.to_str().unwrap()),
        ]);
        let got = candidate_dirs(Os::Windows, &env, None);
        assert_eq!(
            got,
            [
                roaming.join("Claude"),
                store.join("LocalCache").join("Roaming").join("Claude")
            ]
        );
    }

    #[test]
    fn macos_and_linux_candidates() {
        let home = Path::new("/home/u");
        assert_eq!(
            candidate_dirs(Os::Mac, &vars(&[]), Some(home)),
            [home.join("Library/Application Support/Claude")]
        );
        assert_eq!(
            candidate_dirs(Os::Other, &vars(&[("XDG_CONFIG_HOME", "/x")]), Some(home)),
            [PathBuf::from("/x/Claude")]
        );
        assert_eq!(
            candidate_dirs(Os::Other, &vars(&[("XDG_CONFIG_HOME", "")]), Some(home)),
            [home.join(".config/Claude")]
        );
        assert!(candidate_dirs(Os::Mac, &vars(&[]), None).is_empty());
    }

    #[test]
    fn locate_prefers_the_single_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        fs::create_dir_all(&a).unwrap();
        fs::create_dir_all(&b).unwrap();
        fs::write(b.join(CONFIG_FILE), "{}").unwrap();
        assert_eq!(
            locate_in(&[a.clone(), b.clone()]).unwrap(),
            b.join(CONFIG_FILE)
        );
        fs::write(a.join(CONFIG_FILE), "{}").unwrap();
        let err = locate_in(&[a, b]).unwrap_err();
        assert!(
            err.contains("thấy 2 file") && err.contains("--config"),
            "{err}"
        );
    }

    #[test]
    fn locate_creates_file_in_the_single_existing_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let (a, b) = (tmp.path().join("a"), tmp.path().join("b"));
        fs::create_dir_all(&b).unwrap();
        assert_eq!(
            locate_in(&[a.clone(), b.clone()]).unwrap(),
            b.join(CONFIG_FILE)
        );
        fs::create_dir_all(&a).unwrap();
        assert!(locate_in(&[a, b]).unwrap_err().contains("thư mục"));
        let err = locate_in(&[tmp.path().join("không có")]).unwrap_err();
        assert!(err.contains("không thấy Claude Desktop"), "{err}");
    }

    #[test]
    fn entry_round_trips_and_rejects_bad_shapes() {
        let e = ServerEntry {
            command: "/bin/pkm".into(),
            args: vec!["/v".into(), "mcp".into()],
            env: vec![("A".into(), "1".into())],
        };
        assert_eq!(ServerEntry::from_value(&e.to_value()).unwrap(), e);
        let no_env = ServerEntry { env: vec![], ..e };
        assert!(no_env.to_value().get("env").is_none());
        for bad in [
            serde_json::json!([]),
            serde_json::json!({"args": []}),
            serde_json::json!({"command": "x", "args": "a"}),
            serde_json::json!({"command": "x", "args": [1]}),
            serde_json::json!({"command": "x", "env": {"A": 1}}),
        ] {
            assert!(ServerEntry::from_value(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn set_server_replaces_non_object_servers_and_reports_change() {
        let mut doc = ConfigDoc {
            root: serde_json::from_str(r#"{"mcpServers": [], "x": 1}"#).unwrap(),
            exists: true,
            crlf: false,
        };
        assert!(!doc.set_server("kg", None));
        assert!(doc.set_server("kg", Some(serde_json::json!({"command": "a"}))));
        assert!(!doc.set_server("kg", Some(serde_json::json!({"command": "a"}))));
        assert_eq!(
            doc.render(),
            "{\n  \"mcpServers\": {\n    \"kg\": {\n      \"command\": \"a\"\n    }\n  },\n  \"x\": 1\n}"
        );
    }

    #[test]
    fn load_handles_missing_empty_and_non_object() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(CONFIG_FILE);
        assert!(!ConfigDoc::load(&p).unwrap().exists);
        fs::write(&p, "  \n").unwrap();
        assert!(ConfigDoc::load(&p).unwrap().root.is_empty());
        fs::write(&p, "[1]").unwrap();
        assert!(
            ConfigDoc::load(&p)
                .unwrap_err()
                .contains("không phải object")
        );
        fs::write(&p, b"\xff\xfe").unwrap();
        assert!(ConfigDoc::load(&p).unwrap_err().contains("UTF-8"));
    }

    #[test]
    fn write_atomic_backs_up_and_leaves_no_temp_file() {
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(CONFIG_FILE);
        assert_eq!(write_atomic(&p, "one").unwrap(), None);
        let bak = write_atomic(&p, "two").unwrap().unwrap();
        assert_eq!(fs::read_to_string(&bak).unwrap(), "one");
        assert_eq!(fs::read_to_string(&p).unwrap(), "two");
        let bak2 = write_atomic(&p, "three").unwrap().unwrap();
        assert_ne!(bak, bak2, "bản sao lưu cùng giây không đè nhau");
        assert_eq!(fs::read_dir(tmp.path()).unwrap().count(), 3);
        assert!(write_atomic(&tmp.path().join("mất").join(CONFIG_FILE), "x").is_err());
    }

    #[test]
    fn temp_files_are_new_and_distinct() {
        let tmp = tempfile::tempdir().unwrap();
        let name = std::ffi::OsStr::new(CONFIG_FILE);
        let (_a, pa) = create_temp(tmp.path(), name, true).unwrap();
        let (_b, pb) = create_temp(tmp.path(), name, false).unwrap();
        assert_ne!(pa, pb);
        assert!(pa.parent() == Some(tmp.path()) && pb.parent() == Some(tmp.path()));
        assert!(
            pa.file_name()
                .unwrap()
                .to_string_lossy()
                .starts_with(&format!("{CONFIG_FILE}.tmp-{}-", std::process::id()))
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_keeps_permissions() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let p = tmp.path().join(CONFIG_FILE);
        fs::write(&p, "one").unwrap();
        fs::set_permissions(&p, fs::Permissions::from_mode(0o600)).unwrap();
        write_atomic(&p, "two").unwrap();
        assert_eq!(fs::read_to_string(&p).unwrap(), "two");
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o600
        );
        fs::set_permissions(&p, fs::Permissions::from_mode(0o640)).unwrap();
        write_atomic(&p, "three").unwrap();
        assert_eq!(
            fs::metadata(&p).unwrap().permissions().mode() & 0o777,
            0o640
        );
    }

    #[cfg(unix)]
    #[test]
    fn write_atomic_writes_through_symlink() {
        use std::os::unix::fs::PermissionsExt;
        let tmp = tempfile::tempdir().unwrap();
        let dotfiles = tmp.path().join("dotfiles");
        let claude = tmp.path().join("Claude");
        fs::create_dir_all(&dotfiles).unwrap();
        fs::create_dir_all(&claude).unwrap();
        let real = dotfiles.join(CONFIG_FILE);
        fs::write(&real, "one").unwrap();
        fs::set_permissions(&real, fs::Permissions::from_mode(0o600)).unwrap();
        let link = claude.join(CONFIG_FILE);
        std::os::unix::fs::symlink(&real, &link).unwrap();

        let bak = write_atomic(&link, "two").unwrap().unwrap();
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert_eq!(fs::read_link(&link).unwrap(), real);
        assert_eq!(fs::read_to_string(&real).unwrap(), "two");
        assert_eq!(
            fs::metadata(&real).unwrap().permissions().mode() & 0o777,
            0o600
        );
        assert_eq!(fs::read_to_string(&bak).unwrap(), "one");
        assert_eq!(
            fs::read_dir(&dotfiles).unwrap().count(),
            1,
            "không sót file tạm"
        );

        // Symlink gãy: từ chối, giữ nguyên symlink.
        fs::remove_file(&real).unwrap();
        let err = write_atomic(&link, "three").unwrap_err();
        assert!(err.contains("symlink"), "{err}");
        assert!(
            fs::symlink_metadata(&link)
                .unwrap()
                .file_type()
                .is_symlink()
        );
        assert!(!real.exists());
    }

    #[test]
    fn utc_stamp_formats_known_instants() {
        assert_eq!(utc_stamp(UNIX_EPOCH), "19700101T000000Z");
        let t = UNIX_EPOCH + Duration::from_secs(1_793_604_645);
        assert_eq!(utc_stamp(t), "20261102T073045Z");
        let leap = UNIX_EPOCH + Duration::from_secs(951_782_400);
        assert_eq!(utc_stamp(leap), "20000229T000000Z");
    }

    #[test]
    fn entry_diff_marks_old_and_new() {
        let old = serde_json::json!({"command": "a"});
        let new = serde_json::json!({"command": "b"});
        let d = entry_diff("kg", Some(&old), Some(&new));
        assert!(
            d.contains("-     \"command\": \"a\"") && d.contains("+     \"command\": \"b\""),
            "{d}"
        );
        assert!(entry_diff("kg", Some(&old), Some(&old)).contains("không đổi"));
    }
}
