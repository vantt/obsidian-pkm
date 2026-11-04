//! File trong vault mà `pkm setup` ghi: manifest những gì đã thêm (để `--remove` gỡ đúng phần đó), file JSON cấu hình
//! của Obsidian, và file ghi nguyên cả file (snippet, plugin, mẫu) có mã băm để không bao giờ đè file người dùng đã sửa.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

/// Manifest trong vault, cùng chỗ và cùng các khoá của `setup_vault.py` cũ (`plugins`, `community`, `core`,
/// `core_off`, `agent_settings`, `agent_guide`, `agent_allow`, `mcp`), nên vault đã dựng bằng script đó gỡ được bằng
/// `pkm setup --remove`. Khoá thêm: `files` (đường dẫn trong vault -> sha256 của nội dung đã ghi), `snippets_on`,
/// `font_size` (giá trị cũ, `null` là chưa đặt), `plugins_off`, `desktop_config`, `claude_mcp`.
pub const MANIFEST: &str = "_E11/setup-manifest.json";

/// Manifest đọc từ vault; giữ nguyên khoá lạ.
#[derive(Debug, Clone, PartialEq)]
pub struct Manifest(pub Map<String, Value>);

impl Default for Manifest {
    fn default() -> Self {
        let mut m = Map::new();
        for key in ["plugins", "community", "core", "core_off"] {
            m.insert(key.to_owned(), Value::Array(Vec::new()));
        }
        Self(m)
    }
}

impl Manifest {
    pub fn path(vault: &Path) -> PathBuf {
        vault.join(MANIFEST)
    }

    /// Manifest của `vault`; `None` khi chưa có.
    pub fn load(vault: &Path) -> Result<Option<Self>, String> {
        match read_json(&Self::path(vault))? {
            None => Ok(None),
            Some(Value::Object(m)) => Ok(Some(Self(m))),
            Some(_) => Err(format!("{MANIFEST}: gốc JSON không phải object")),
        }
    }

    pub fn save(&self, vault: &Path) -> Result<(), String> {
        write_json(&Self::path(vault), &Value::Object(self.0.clone()))
    }

    /// Mảng chuỗi ở `key` (rỗng khi không có).
    pub fn list(&self, key: &str) -> Vec<String> {
        self.0
            .get(key)
            .and_then(Value::as_array)
            .map(|a| {
                a.iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default()
    }

    /// Thêm vào mảng `key` các mục chưa có.
    pub fn add(&mut self, key: &str, items: &[String]) {
        let mut list = self.list(key);
        for i in items {
            if !list.contains(i) {
                list.push(i.clone());
            }
        }
        self.0.insert(key.to_owned(), list.into());
    }

    /// Bỏ khỏi mảng `key` các mục trong `items`.
    pub fn drop_items(&mut self, key: &str, items: &[String]) {
        let list: Vec<String> = self
            .list(key)
            .into_iter()
            .filter(|i| !items.contains(i))
            .collect();
        self.0.insert(key.to_owned(), list.into());
    }

    pub fn get(&self, key: &str) -> Option<&Value> {
        self.0.get(key)
    }

    pub fn set(&mut self, key: &str, v: Value) {
        self.0.insert(key.to_owned(), v);
    }

    /// Mã băm đã ghi của file `rel` (đường dẫn trong vault, dấu `/`).
    pub fn file_hash(&self, rel: &str) -> Option<&str> {
        self.0.get("files")?.as_object()?.get(rel)?.as_str()
    }

    /// Các file đã ghi: `(đường dẫn trong vault, sha256)`.
    pub fn files(&self) -> Vec<(String, String)> {
        self.0
            .get("files")
            .and_then(Value::as_object)
            .map(|m| {
                m.iter()
                    .filter_map(|(k, v)| Some((k.clone(), v.as_str()?.to_owned())))
                    .collect()
            })
            .unwrap_or_default()
    }

    pub fn set_file(&mut self, rel: &str, sha: Option<String>) {
        let files = self
            .0
            .entry("files")
            .or_insert_with(|| Value::Object(Map::new()));
        if !files.is_object() {
            *files = Value::Object(Map::new());
        }
        let Value::Object(files) = files else {
            unreachable!("vừa đặt object")
        };
        match sha {
            Some(s) => {
                files.insert(rel.to_owned(), Value::String(s));
            }
            None => {
                files.remove(rel);
            }
        }
    }
}

/// sha256 hex chữ thường.
pub fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Nội dung file, `None` khi chưa có.
pub fn read_opt(path: &Path) -> Result<Option<Vec<u8>>, String> {
    match fs::read(path) {
        Ok(b) => Ok(Some(b)),
        Err(e) if e.kind() == io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(format!("đọc {}: {e}", path.display())),
    }
}

/// JSON trong `path` (`None` khi chưa có file). File có mà không phải JSON là lỗi: không bao giờ ghi đè nó.
pub fn read_json(path: &Path) -> Result<Option<Value>, String> {
    let Some(raw) = read_opt(path)? else {
        return Ok(None);
    };
    let text = String::from_utf8_lossy(&raw);
    let text = text.strip_prefix('\u{feff}').unwrap_or(&text);
    if text.trim().is_empty() {
        return Ok(None);
    }
    serde_json::from_str(text).map(Some).map_err(|e| {
        format!(
            "{} không phải JSON hợp lệ ({e}); sửa tay file này rồi chạy lại",
            path.display()
        )
    })
}

/// Ghi JSON thụt 2 dấu cách (như Obsidian), tạo thư mục cha, qua file tạm rồi đổi tên.
pub fn write_json(path: &Path, v: &Value) -> Result<(), String> {
    let text = serde_json::to_string_pretty(v).map_err(|e| e.to_string())?;
    write_file(path, text.as_bytes())
}

/// Ghi `bytes` vào `path` qua file tạm cùng thư mục rồi đổi tên, để file không bao giờ dở dang.
pub fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let dir = path
        .parent()
        .ok_or_else(|| format!("{} không có thư mục cha", path.display()))?;
    fs::create_dir_all(dir).map_err(|e| format!("tạo {}: {e}", dir.display()))?;
    let name = path
        .file_name()
        .map(|n| n.to_string_lossy().into_owned())
        .unwrap_or_default();
    let tmp = dir.join(format!(".{name}.pkm-setup.tmp"));
    fs::write(&tmp, bytes).map_err(|e| format!("ghi {}: {e}", tmp.display()))?;
    fs::rename(&tmp, path).map_err(|e| {
        let _ = fs::remove_file(&tmp);
        format!("ghi {}: {e}", path.display())
    })
}

/// Kết quả của [`put_owned`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Put {
    /// Chưa có file: ghi mới.
    Wrote,
    /// File do setup ghi, người dùng chưa sửa, nội dung mới khác: ghi lại.
    Updated,
    /// File đã đúng nội dung này.
    Same,
    /// File khác nội dung và không phải bản setup ghi (người dùng đã sửa, hoặc file của người dùng): giữ nguyên.
    Kept,
    /// Chế độ "chỉ ghi khi chưa có": file đã có, giữ nguyên.
    Exists,
}

impl Put {
    /// Lần này có ghi (hoặc sẽ ghi, khi chạy thử).
    pub fn writes(self) -> bool {
        matches!(self, Put::Wrote | Put::Updated)
    }
}

/// Đặt file `rel` (trong `vault`, dấu `/`) thành `bytes` mà không đè file người dùng đã sửa: file chưa có thì ghi; file
/// đúng mã băm setup đã ghi lần trước thì ghi lại (nâng bản); khác thì giữ. `keep_once`: đã có file là giữ, kể cả
/// file setup ghi (mẫu người dùng sẽ sửa). Chỉ ghi khi `apply`; ghi thì lưu mã băm vào `m`.
pub fn put_owned(
    vault: &Path,
    rel: &str,
    bytes: &[u8],
    m: &mut Manifest,
    apply: bool,
    keep_once: bool,
) -> Result<Put, String> {
    let path = vault.join(rel);
    let want = sha256_hex(bytes);
    let outcome = match read_opt(&path)? {
        None => Put::Wrote,
        Some(cur) => {
            let cur = sha256_hex(&cur);
            if cur == want {
                Put::Same
            } else if keep_once {
                Put::Exists
            } else if m.file_hash(rel) == Some(cur.as_str()) {
                Put::Updated
            } else {
                Put::Kept
            }
        }
    };
    if apply && outcome.writes() {
        write_file(&path, bytes)?;
        m.set_file(rel, Some(want));
    }
    Ok(outcome)
}

/// Kết quả của [`remove_owned`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Removed {
    Deleted,
    /// Đã sửa từ lần ghi: giữ.
    Edited,
    /// Không còn file.
    Missing,
}

/// Xoá file `rel` khi nội dung còn đúng mã băm `sha` đã ghi (chỉ khi `apply`).
pub fn remove_owned(vault: &Path, rel: &str, sha: &str, apply: bool) -> Result<Removed, String> {
    let path = vault.join(rel);
    let Some(cur) = read_opt(&path)? else {
        return Ok(Removed::Missing);
    };
    if sha256_hex(&cur) != sha {
        return Ok(Removed::Edited);
    }
    if apply {
        fs::remove_file(&path).map_err(|e| format!("xoá {}: {e}", path.display()))?;
    }
    Ok(Removed::Deleted)
}

/// Xoá thư mục `dir` khi rỗng; trả tên các mục còn lại.
pub fn remove_dir_if_empty(dir: &Path) -> Vec<String> {
    let rest: Vec<String> = fs::read_dir(dir)
        .map(|it| {
            it.filter_map(Result::ok)
                .map(|e| e.file_name().to_string_lossy().into_owned())
                .collect()
        })
        .unwrap_or_default();
    if rest.is_empty() {
        let _ = fs::remove_dir(dir);
    }
    rest
}

/// `version` trong `manifest.json` của một thư mục plugin.
pub fn plugin_version(dir: &Path) -> Option<String> {
    read_json(&dir.join("manifest.json"))
        .ok()??
        .get("version")?
        .as_str()
        .map(str::to_owned)
}
