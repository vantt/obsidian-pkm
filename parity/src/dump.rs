//! Đọc và ghi một thư mục dump (định dạng ở `parity/README.md`, mục "Định dạng dump").
//!
//! Dump là đầu ra trung lập ngôn ngữ của một lần chạy: bảng index (JSONL), câu đáp án,
//! bộ câu tool, bắt tay MCP, đầu ra CLI, số đo hiệu năng và `manifest.json` mô tả lần chạy.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Phiên bản định dạng dump. Đổi định dạng thì tăng số này và dựng lại golden.
pub const FORMAT: u32 = 1;

/// Các tầng so, theo thứ tự in trong báo cáo.
pub const LAYERS: [&str; 6] = ["index", "answers", "tools", "mcp", "cli", "perf"];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Manifest {
    pub format: u32,
    pub implementation: String,
    #[serde(default)]
    pub spec_sha: Option<String>,
    pub vault: VaultInfo,
    #[serde(default)]
    pub run: Value,
    #[serde(default)]
    pub tables: BTreeMap<String, TableInfo>,
    #[serde(default)]
    pub answers: Option<Value>,
    #[serde(default)]
    pub calls: Option<usize>,
    #[serde(default)]
    pub cli: Option<usize>,
    /// Mục bên này chưa làm được, dạng `tầng:mục` hoặc `tầng:*`. Báo "chưa có", không bao giờ tính là khớp.
    #[serde(default)]
    pub not_available: Vec<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct VaultInfo {
    pub name: String,
    #[serde(default)]
    pub fixture: Option<FixtureInfo>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct FixtureInfo {
    #[serde(default)]
    pub preset: Option<String>,
    #[serde(default)]
    pub version: Option<String>,
    #[serde(default)]
    pub seed: Option<Value>,
    #[serde(default)]
    pub tree_hash: Option<String>,
    #[serde(default)]
    pub files: Option<u64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct TableInfo {
    pub kind: String,
    pub columns: Vec<String>,
    #[serde(default)]
    pub key: Vec<String>,
    pub rows: usize,
}

/// Một dòng của file JSONL cùng số dòng (đếm từ 1) trong file.
#[derive(Debug, Clone)]
pub struct Line {
    pub no: usize,
    pub value: Value,
}

/// Một lệnh CLI đã chạy (`cli/commands.json`).
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct CliCommand {
    pub id: String,
    pub args: Vec<String>,
    pub exit: i32,
    pub stdout: String,
    pub kind: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stderr: Option<String>,
}

#[derive(Debug, Clone)]
pub struct Dump {
    pub dir: PathBuf,
    pub manifest: Manifest,
}

impl Dump {
    pub fn open(dir: &Path) -> Result<Self> {
        let path = dir.join("manifest.json");
        let text = fs::read_to_string(&path)
            .with_context(|| format!("không đọc được {} (thư mục dump?)", path.display()))?;
        let manifest: Manifest = serde_json::from_str(&text)
            .with_context(|| format!("{} không đúng định dạng manifest", path.display()))?;
        if manifest.format != FORMAT {
            bail!(
                "{}: định dạng dump {} khác định dạng bộ so khớp đang dùng ({FORMAT}); dựng lại dump",
                path.display(),
                manifest.format
            );
        }
        Ok(Self {
            dir: dir.to_path_buf(),
            manifest,
        })
    }

    /// Mục `layer:item` được bên này khai là chưa có.
    pub fn not_available(&self, layer: &str, item: &str) -> bool {
        self.manifest.not_available.iter().any(|d| {
            d.split_once(':')
                .is_some_and(|(l, i)| l == layer && (i == "*" || i == item))
        })
    }

    pub fn path(&self, rel: &str) -> PathBuf {
        self.dir.join(rel)
    }

    pub fn exists(&self, rel: &str) -> bool {
        self.path(rel).is_file()
    }

    pub fn jsonl(&self, rel: &str) -> Result<Vec<Line>> {
        read_jsonl(&self.path(rel))
    }

    pub fn json(&self, rel: &str) -> Result<(String, Value)> {
        let path = self.path(rel);
        let text = read_text(&path)?;
        let value = serde_json::from_str(&text)
            .with_context(|| format!("{}: JSON lỗi (sửa tay file dump?)", path.display()))?;
        Ok((text, value))
    }

    pub fn text(&self, rel: &str) -> Result<String> {
        read_text(&self.path(rel))
    }

    pub fn cli_commands(&self) -> Result<Vec<CliCommand>> {
        let (_, v) = self.json("cli/commands.json")?;
        serde_json::from_value(v).context("cli/commands.json không đúng định dạng")
    }
}

pub fn read_text(path: &Path) -> Result<String> {
    fs::read_to_string(path).with_context(|| format!("không đọc được {}", path.display()))
}

/// Đọc JSONL; dòng trống bỏ qua. Lỗi chỉ nêu file và số dòng, không in nội dung (an toàn cho chế độ `--private`).
pub fn read_jsonl(path: &Path) -> Result<Vec<Line>> {
    let text = read_text(path)?;
    let mut out = Vec::new();
    for (i, raw) in text.lines().enumerate() {
        if raw.trim().is_empty() {
            continue;
        }
        let value = serde_json::from_str(raw)
            .map_err(|e| anyhow::anyhow!("{}:{}: JSON lỗi ({e})", path.display(), i + 1))?;
        out.push(Line { no: i + 1, value });
    }
    Ok(out)
}

/// Ghi dump của bản Rust. Chỉ ghi file của mục đã làm được; mục còn lại khai trong `not_available`.
pub struct Writer {
    dir: PathBuf,
}

impl Writer {
    /// Tạo thư mục dump mới. Thư mục đã có thì chỉ xoá khi nó là dump (có `manifest.json`).
    pub fn create(dir: &Path) -> Result<Self> {
        if dir.exists() {
            let empty = fs::read_dir(dir)
                .with_context(|| format!("không đọc được {}", dir.display()))?
                .next()
                .is_none();
            if !empty && !dir.join("manifest.json").is_file() {
                bail!(
                    "{} có file nhưng không phải dump (thiếu manifest.json); không xoá",
                    dir.display()
                );
            }
            fs::remove_dir_all(dir).with_context(|| format!("không xoá được {}", dir.display()))?;
        }
        fs::create_dir_all(dir).with_context(|| format!("không tạo được {}", dir.display()))?;
        Ok(Self {
            dir: dir.to_path_buf(),
        })
    }

    fn file(&self, rel: &str) -> Result<PathBuf> {
        let path = self.dir.join(rel);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)
                .with_context(|| format!("không tạo được {}", parent.display()))?;
        }
        Ok(path)
    }

    pub fn jsonl(&self, rel: &str, rows: &[Value]) -> Result<()> {
        let mut text = String::new();
        for row in rows {
            text.push_str(&serde_json::to_string(row)?);
            text.push('\n');
        }
        self.text(rel, &text)
    }

    pub fn json(&self, rel: &str, value: &Value) -> Result<()> {
        let mut text = serde_json::to_string_pretty(value)?;
        text.push('\n');
        self.text(rel, &text)
    }

    pub fn text(&self, rel: &str, text: &str) -> Result<()> {
        let path = self.file(rel)?;
        fs::write(&path, text).with_context(|| format!("không ghi được {}", path.display()))
    }

    pub fn manifest(&self, manifest: &Manifest) -> Result<()> {
        self.json("manifest.json", &serde_json::to_value(manifest)?)
    }
}
