//! `pkm setup` như người dùng gọi, trên vault tạm: plugin `pkm` từ một bản dựng giả (`--plugin-dir`), không OHS,
//! cấu hình Claude Desktop là file tạm, thư mục dữ liệu của pkm và HOME là thư mục tạm. PATH chỉ có một `claude` giả
//! luôn thành công (không `npm`): không bao giờ chạm cấu hình thật của người dùng. Bốn plugin cộng đồng và theme
//! Primary có sẵn đúng bản ghim nên không tải gì từ mạng.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::{Value, json};

const VERSION: &str = env!("CARGO_PKG_VERSION");

struct Env {
    tmp: tempfile::TempDir,
    vault: PathBuf,
    config: PathBuf,
    plugin: PathBuf,
}

fn env() -> Env {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Kho thử");
    let obs = vault.join(".obsidian");
    fs::create_dir_all(&obs).unwrap();
    fs::write(vault.join("A.md"), "---\ntype: note\n---\nXem [[B]].\n").unwrap();
    fs::write(vault.join("B.md"), "B\n").unwrap();
    for (id, v) in [
        ("homepage", "4.2.2"),
        ("templater-obsidian", "2.26.0"),
        ("agent-client", "0.13.0"),
        ("obsidian-style-settings", "1.0.9"),
    ] {
        let d = obs.join("plugins").join(id);
        fs::create_dir_all(&d).unwrap();
        fs::write(
            d.join("manifest.json"),
            json!({"id": id, "version": v}).to_string(),
        )
        .unwrap();
    }
    let theme = obs.join("themes").join("Primary");
    fs::create_dir_all(&theme).unwrap();
    fs::write(
        theme.join("manifest.json"),
        json!({"name": "Primary", "version": "2.10.0"}).to_string(),
    )
    .unwrap();
    fs::write(obs.join("community-plugins.json"), r#"["dataview"]"#).unwrap();
    let plugin = tmp.path().join("plugin-build");
    fs::create_dir_all(&plugin).unwrap();
    fs::write(plugin.join("main.js"), "module.exports = {};\n").unwrap();
    fs::write(plugin.join("styles.css"), ".folder-grid {}\n").unwrap();
    fs::write(
        plugin.join("manifest.json"),
        json!({"id": "pkm", "version": VERSION}).to_string(),
    )
    .unwrap();
    for d in ["home", "path", "claude"] {
        fs::create_dir_all(tmp.path().join(d)).unwrap();
    }
    fake_claude(&tmp.path().join("path"));
    Env {
        config: tmp.path().join("claude").join("claude_desktop_config.json"),
        vault,
        plugin,
        tmp,
    }
}

/// `claude` giả trong `dir`: mọi lệnh (`mcp add`, `mcp remove`) thành công, không làm gì.
fn fake_claude(dir: &Path) {
    if cfg!(windows) {
        fs::write(dir.join("claude.cmd"), "@exit /b 0\r\n").unwrap();
    } else {
        let p = dir.join("claude");
        fs::write(&p, "#!/bin/sh\nexit 0\n").unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&p, fs::Permissions::from_mode(0o755)).unwrap();
        }
    }
}

impl Env {
    fn setup(&self, extra: &[&str]) -> Output {
        self.setup_with(Some(&self.config), extra)
    }

    /// Như [`Env::setup`]; `config` `None` là máy không có Claude Desktop (không có `--config`, HOME tạm rỗng).
    fn setup_with(&self, config: Option<&Path>, extra: &[&str]) -> Output {
        let home = self.tmp.path().join("home");
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_pkm"));
        cmd.arg("setup")
            .arg(&self.vault)
            .arg("--plugin-dir")
            .arg(&self.plugin);
        if let Some(c) = config {
            cmd.arg("--config").arg(c);
        }
        cmd.args(["--no-ohs", "--force", "--allow-any-location"])
            .args(extra)
            .env("PATH", self.tmp.path().join("path"))
            .env("HOME", &home)
            .env("USERPROFILE", &home)
            .env("XDG_CONFIG_HOME", home.join("config"))
            .env("XDG_CACHE_HOME", home.join("cache"))
            .env("LOCALAPPDATA", home.join("local"))
            .env("APPDATA", home.join("roaming"))
            .env_remove("PKM_INSTALL_DIR")
            .env_remove("PKM_OHS_DIR")
            .env_remove("PKM_OHS_PACKAGE");
        cmd.output().unwrap()
    }

    fn ok(&self, extra: &[&str]) -> String {
        Self::check(self.setup(extra), extra)
    }

    fn check(o: Output, extra: &[&str]) -> String {
        let out = String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(
            o.status.success(),
            "pkm setup {extra:?}: {}\n{out}\n{}",
            o.status,
            String::from_utf8_lossy(&o.stderr)
        );
        out
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(&fs::read_to_string(self.vault.join(rel)).unwrap()).unwrap()
    }
}

/// Mọi file dưới `dir` kèm nội dung.
fn snapshot(dir: &Path) -> Vec<(PathBuf, Vec<u8>)> {
    let mut out = Vec::new();
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for e in fs::read_dir(&d).unwrap() {
            let p = e.unwrap().path();
            if p.is_dir() {
                stack.push(p.clone());
                out.push((p, Vec::new()));
            } else {
                out.push((p.clone(), fs::read(&p).unwrap()));
            }
        }
    }
    out.sort();
    out
}

#[test]
fn setup_dry_run_apply_rerun_and_remove() {
    let e = env();
    let pristine = snapshot(&e.vault);

    // Chạy thử: in kế hoạch, không ghi gì.
    let out = e.ok(&[]);
    assert!(
        out.contains("[pkm-plugin]") && out.contains("--apply"),
        "{out}"
    );
    assert_eq!(snapshot(&e.vault), pristine);
    assert!(!e.config.exists());

    // Báo cáo JSON cho vỏ GUI.
    let report: Value = serde_json::from_str(&e.ok(&["--json"])).unwrap();
    assert_eq!(report["ok"], json!(true));
    assert_eq!(report["apply"], json!(false));

    // Làm thật.
    let out = e.ok(&["--apply"]);
    assert!(out.contains("MCP pkm cho Claude Code"), "{out}");
    let p = e.vault.join(".obsidian/plugins/pkm");
    assert_eq!(
        fs::read_to_string(p.join("main.js")).unwrap(),
        "module.exports = {};\n"
    );
    assert_eq!(
        e.json(".obsidian/plugins/pkm/manifest.json")["version"],
        json!(VERSION)
    );
    let data = e.json(".obsidian/plugins/pkm/data.json");
    assert_eq!(
        Path::new(data["pkmPath"].as_str().unwrap()),
        Path::new(env!("CARGO_BIN_EXE_pkm"))
    );
    let db = PathBuf::from(data["graphDbPath"].as_str().unwrap());
    assert!(
        db.starts_with(e.tmp.path().join("home")),
        "{}",
        db.display()
    );
    assert!(db.is_file(), "index đã dựng");
    assert_eq!(
        e.json(".obsidian/community-plugins.json"),
        json!([
            "dataview",
            "homepage",
            "templater-obsidian",
            "agent-client",
            "obsidian-style-settings",
            "pkm"
        ])
    );
    assert_eq!(
        e.json(".obsidian/core-plugins.json"),
        json!({"bases": true})
    );
    assert_eq!(
        e.json(".obsidian/appearance.json"),
        json!({"cssTheme": "Primary", "enabledCssSnippets": ["e11-theme"], "baseFontSize": 19})
    );
    assert!(e.vault.join(".obsidian/snippets/e11-theme.css").is_file());
    assert!(e.vault.join(".claude/CLAUDE.md").is_file());
    assert_eq!(
        e.json(".claude/settings.local.json"),
        json!({"permissions": {"allow": ["mcp__pkm"]}})
    );
    let cfg: Value = serde_json::from_str(&fs::read_to_string(&e.config).unwrap()).unwrap();
    let entry = &cfg["mcpServers"]["pkm"];
    assert_eq!(entry["args"][1], json!("mcp"));
    assert!(
        entry["args"]
            .as_array()
            .unwrap()
            .contains(&json!("--body-edges"))
    );
    assert!(e.vault.join("_E11/setup-manifest.json").is_file());

    // Chạy lại: không đổi file nào.
    let applied = snapshot(&e.vault);
    let out = e.ok(&["--apply"]);
    assert!(!out.contains("  ✓ "), "lần hai không còn việc gì:\n{out}");
    assert_eq!(snapshot(&e.vault), applied);

    // Gỡ: file đã sửa giữ nguyên, phần còn lại về như trước.
    fs::write(e.vault.join(".claude/CLAUDE.md"), "của tôi\n").unwrap();
    let edited = snapshot(&e.vault);
    e.ok(&["--remove"]);
    assert_eq!(snapshot(&e.vault), edited, "gỡ thử không đổi gì");
    e.ok(&["--remove", "--apply"]);
    assert_eq!(
        fs::read_to_string(e.vault.join(".claude/CLAUDE.md")).unwrap(),
        "của tôi\n"
    );
    for gone in [
        ".obsidian/plugins/pkm",
        ".obsidian/snippets",
        "_E11/setup-manifest.json",
    ] {
        assert!(!e.vault.join(gone).exists(), "{gone}");
    }
    assert_eq!(
        e.json(".obsidian/community-plugins.json"),
        json!(["dataview"])
    );
    assert_eq!(e.json(".obsidian/core-plugins.json"), json!({}));
    let cfg: Value = serde_json::from_str(&fs::read_to_string(&e.config).unwrap()).unwrap();
    assert!(cfg["mcpServers"].get("pkm").is_none());
    assert_eq!(
        e.json(".obsidian/appearance.json"),
        json!({"enabledCssSnippets": []})
    );
    assert!(
        e.vault
            .join(".obsidian/themes/Primary/manifest.json")
            .is_file(),
        "theme có từ trước được giữ"
    );
    for id in [
        "homepage",
        "templater-obsidian",
        "agent-client",
        "obsidian-style-settings",
    ] {
        assert!(
            e.vault
                .join(".obsidian/plugins")
                .join(id)
                .join("manifest.json")
                .is_file(),
            "plugin {id} có từ trước được giữ"
        );
    }
}

#[test]
fn setup_without_claude_desktop_builds_the_index_for_claude_code() {
    let e = env();
    let out = Env::check(e.setup_with(None, &["--apply"]), &["--apply"]);
    assert!(out.contains("không thấy Claude Desktop"), "{out}");
    assert!(out.contains("server trả lời"), "{out}");
    let data = e.json(".obsidian/plugins/pkm/data.json");
    assert!(
        PathBuf::from(data["graphDbPath"].as_str().unwrap()).is_file(),
        "index đã dựng"
    );
    assert!(!e.config.exists());
    let m = e.json("_E11/setup-manifest.json");
    assert!(m.get("desktop_config").is_none(), "{m}");
    assert_eq!(m["mcp"], json!(["pkm"]));

    // doctor: thiếu Claude Desktop chỉ là cảnh báo.
    let home = e.tmp.path().join("home");
    let o = Command::new(env!("CARGO_BIN_EXE_pkm"))
        .args(["doctor", "--json"])
        .env("PATH", e.tmp.path().join("path"))
        .env("HOME", &home)
        .env("USERPROFILE", &home)
        .env("XDG_CONFIG_HOME", home.join("config"))
        .env("XDG_CACHE_HOME", home.join("cache"))
        .env("LOCALAPPDATA", home.join("local"))
        .env("APPDATA", home.join("roaming"))
        .output()
        .unwrap();
    let report: Value = serde_json::from_slice(&o.stdout).unwrap();
    let config = report["data"]["checks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["id"] == "claude_config")
        .unwrap();
    assert_eq!(config["status"], json!("warn"), "{report}");
}

#[test]
fn setup_refuses_a_folder_that_is_not_a_vault() {
    let e = env();
    fs::remove_dir_all(e.vault.join(".obsidian")).unwrap();
    let o = e.setup(&["--apply"]);
    assert!(!o.status.success());
    assert!(String::from_utf8_lossy(&o.stderr).contains(".obsidian"));
}
