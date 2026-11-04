//! Preset bản B nhúng trong binary: `presets/b/setup.json` và các file nó chỉ tới (mẫu `CLAUDE.md`, snippet theme).
//! Nhúng lúc build nên `pkm setup` không cần repo hay mạng để có dữ liệu preset.

use serde_json::{Map, Value};

/// `presets/b/setup.json`.
pub const SETUP_JSON: &str = include_str!("../../../../presets/b/setup.json");

/// File preset mà `setup.json` được phép chỉ tới, theo đường dẫn trong `presets/b/`.
const EMBEDDED: [(&str, &[u8]); 2] = [
    (
        "vault-agent.md",
        include_bytes!("../../../../presets/b/vault-agent.md"),
    ),
    (
        "theme/e11-theme.css",
        include_bytes!("../../../../presets/b/theme/e11-theme.css"),
    ),
];

/// Plugin cộng đồng ghim phiên bản: tải từ `https://github.com/<repo>/releases/download/<version>/<file>`.
#[derive(Debug, Clone)]
pub struct PinnedPlugin {
    pub id: String,
    pub repo: String,
    pub version: String,
    /// `(tên file, sha256 hex)` theo thứ tự trong `setup.json`.
    pub files: Vec<(String, String)>,
}

impl PinnedPlugin {
    pub fn url(&self, file: &str) -> String {
        format!(
            "https://github.com/{}/releases/download/{}/{file}",
            self.repo, self.version
        )
    }
}

/// Nội dung `setup.json` đã đọc.
#[derive(Debug, Clone)]
pub struct Preset {
    pub plugins: Vec<PinnedPlugin>,
    pub core_plugins: Vec<String>,
    /// Gói npm của adapter ACP và phiên bản ghim.
    pub acp_package: String,
    pub acp_version: String,
    /// Preset Claude Code của Agent Client và lệnh mặc định của nó.
    pub agent_preset: String,
    pub agent_preset_default_command: String,
    /// Mẫu `CLAUDE.md` và chỗ ghi trong vault.
    pub guide: &'static [u8],
    pub guide_target: String,
    /// Tool Claude Code được dùng không hỏi (`.claude/settings.local.json`).
    pub allow: Vec<String>,
    /// Server MCP cho Claude Code và các server cũ nó thay.
    pub mcp_name: String,
    pub mcp_replaces: Vec<String>,
    /// Cờ đọc vault của bản B (`--packs ... --adapter e11`).
    pub mcp_flags: Vec<String>,
    pub about: String,
    /// `(tên snippet, nội dung)`: ghi vào `.obsidian/snippets/<tên>.css`.
    pub snippets: Vec<(String, &'static [u8])>,
    pub css_theme: String,
    pub css_theme_version: String,
    pub base_font_size: u64,
    /// Plugin phải tắt vì ghi đè cỡ chữ.
    pub disable_plugins: Vec<String>,
    /// Plugin Style Settings mà snippet cần để chọn font.
    pub style_settings: String,
}

impl Preset {
    /// Preset nhúng; `setup.json` hỏng là lỗi lúc build (test `embedded_preset_parses` bắt).
    pub fn embedded() -> Self {
        Self::parse(SETUP_JSON).expect("presets/b/setup.json hợp lệ")
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        let root: Value =
            serde_json::from_str(text).map_err(|e| format!("setup.json không phải JSON: {e}"))?;
        let obj = |v: &Value, key: &str| -> Result<Map<String, Value>, String> {
            v.get(key)
                .and_then(Value::as_object)
                .cloned()
                .ok_or_else(|| format!("setup.json: thiếu object `{key}`"))
        };
        let agent = Value::Object(obj(&root, "agent")?);
        let mcp = Value::Object(obj(&root, "mcp")?);
        let theme = Value::Object(obj(&root, "theme")?);
        let mut plugins = Vec::new();
        for (id, p) in obj(&root, "plugins")? {
            let files = obj(&p, "sha256")?
                .into_iter()
                .map(|(name, sha)| {
                    let sha = sha
                        .as_str()
                        .filter(|s| s.len() == 64 && s.bytes().all(|b| b.is_ascii_hexdigit()))
                        .ok_or_else(|| {
                            format!("setup.json: sha256 của {id}/{name} không hợp lệ")
                        })?;
                    Ok((name, sha.to_ascii_lowercase()))
                })
                .collect::<Result<Vec<_>, String>>()?;
            plugins.push(PinnedPlugin {
                repo: string(&p, "repo")?,
                version: string(&p, "version")?,
                id,
                files,
            });
        }
        let guide = obj(&agent, "guide")?;
        let guide = Value::Object(guide);
        let snippets = obj(&root, "snippets")?
            .into_iter()
            .map(|(name, file)| {
                let file = file
                    .as_str()
                    .ok_or_else(|| format!("setup.json: snippet {name} không phải chuỗi"))?;
                Ok((name, embedded(file)?))
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(Self {
            plugins,
            core_plugins: strings(&root, "core_plugins")?,
            acp_package: string(&agent, "acp_package")?,
            acp_version: string(&agent, "acp_version")?,
            agent_preset: string(&agent, "agent_client_preset")?,
            agent_preset_default_command: string(&agent, "agent_client_preset_default_command")?,
            guide: embedded(&string(&guide, "source")?)?,
            guide_target: string(&guide, "target")?,
            allow: strings(&agent, "allow")?,
            mcp_name: string(&mcp, "name")?,
            mcp_replaces: strings(&mcp, "replaces")?,
            mcp_flags: strings(&mcp, "flags")?,
            about: string(&mcp, "about")?,
            snippets,
            css_theme: string(&theme, "css_theme")?,
            css_theme_version: string(&theme, "css_theme_version")?,
            base_font_size: theme
                .get("base_font_size")
                .and_then(Value::as_u64)
                .ok_or("setup.json: thiếu theme.base_font_size")?,
            disable_plugins: strings(&theme, "disable_plugins")?,
            style_settings: string(&theme, "style_settings_plugin")?,
        })
    }
}

/// Nội dung file preset `rel` (đường dẫn trong `presets/b/`) đã nhúng.
fn embedded(rel: &str) -> Result<&'static [u8], String> {
    EMBEDDED
        .iter()
        .find(|(name, _)| *name == rel)
        .map(|(_, bytes)| *bytes)
        .ok_or_else(|| format!("setup.json chỉ tới {rel}, file này không được nhúng vào pkm"))
}

fn string(v: &Value, key: &str) -> Result<String, String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or_else(|| format!("setup.json: thiếu chuỗi `{key}`"))
}

fn strings(v: &Value, key: &str) -> Result<Vec<String>, String> {
    v.get(key)
        .and_then(Value::as_array)
        .and_then(|a| a.iter().map(|x| x.as_str().map(str::to_owned)).collect())
        .ok_or_else(|| format!("setup.json: thiếu mảng chuỗi `{key}`"))
}
