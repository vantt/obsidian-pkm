use super::*;
use std::cell::RefCell;
use std::fs;

use pkm_core::ohs::OhsState;

use crate::install::handshake::Handshake;

/// Một lệnh đã "chạy": `(chương trình, đối số, cwd)`.
type Run = (PathBuf, Vec<String>, Option<PathBuf>);
/// Dòng báo cáo: `(bước, trạng thái, chữ)`.
type Item = (String, Status, String);

/// Hooks giả: không mạng, không tiến trình thật; ghi lại mọi lệnh "chạy".
struct Fake {
    /// Nội dung trả cho từng URL.
    served: Vec<(String, Vec<u8>)>,
    /// Chương trình "có trên PATH".
    programs: Vec<(&'static str, PathBuf)>,
    /// Lệnh đã chạy: `(chương trình, đối số, cwd)`.
    runs: RefCell<Vec<Run>>,
    /// Lệnh có đối số chứa chuỗi này thì thất bại.
    fail_on: Option<&'static str>,
    ohs_state: OhsState,
    builds: RefCell<u32>,
}

impl Fake {
    fn new() -> Self {
        Self {
            served: Vec::new(),
            programs: Vec::new(),
            runs: RefCell::default(),
            fail_on: None,
            ohs_state: OhsState::Ready,
            builds: RefCell::new(0),
        }
    }

    fn runs(&self) -> Vec<Vec<String>> {
        self.runs
            .borrow()
            .iter()
            .map(|(_, a, _)| a.clone())
            .collect()
    }
}

impl Hooks for Fake {
    fn build_index(&self, _: &Path, _: &Path, _: &Path, _: &[String]) -> Result<(), String> {
        *self.builds.borrow_mut() += 1;
        Ok(())
    }

    fn handshake(&self, _: &ServerEntry, _: Duration) -> Result<Handshake, String> {
        Ok(Handshake {
            server_name: "pkm".into(),
            server_version: "test".into(),
            tools: vec!["search".into()],
        })
    }

    fn desktop_process(&self) -> io::Result<Option<String>> {
        Ok(None)
    }

    fn ohs_state(&self, _: OhsPackage, _: &Path, _: Duration) -> OhsState {
        self.ohs_state.clone()
    }

    fn warm_ohs(&self, _: OhsPackage, _: &Path, _: Duration, _: &mut dyn Write) -> OhsState {
        OhsState::Ready
    }

    fn fetch(&self, url: &str, out: &Path) -> Result<(), String> {
        let (_, body) = self
            .served
            .iter()
            .find(|(u, _)| u == url)
            .ok_or_else(|| format!("404 {url}"))?;
        fs::write(out, body).map_err(|e| e.to_string())
    }
}

impl SetupHooks for Fake {
    fn desktop_config(&self, explicit: Option<&Path>) -> Result<Option<PathBuf>, String> {
        Ok(explicit.map(Path::to_path_buf))
    }

    fn which(&self, name: &str) -> Option<PathBuf> {
        self.programs
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, p)| p.clone())
    }

    fn run(&self, program: &Path, args: &[OsString], cwd: Option<&Path>) -> Result<String, String> {
        let args: Vec<String> = args
            .iter()
            .map(|a| a.to_string_lossy().into_owned())
            .collect();
        self.runs.borrow_mut().push((
            program.to_path_buf(),
            args.clone(),
            cwd.map(Path::to_path_buf),
        ));
        if let Some(bad) = self.fail_on
            && args.iter().any(|a| a.contains(bad))
        {
            return Err(format!("{bad} lỗi"));
        }
        // Giả lập kết quả: npm cài gói vào --prefix, pairs ghi file.
        if args.first().map(String::as_str) == Some("install") {
            let prefix = PathBuf::from(&args[2]);
            let (pkg, version) = args[3].rsplit_once('@').unwrap();
            let dir = pkg
                .split('/')
                .fold(prefix.join("node_modules"), |d, p| d.join(p));
            fs::create_dir_all(&dir).unwrap();
            fs::write(
                dir.join("package.json"),
                format!(r#"{{"version":"{version}"}}"#),
            )
            .unwrap();
        }
        if args.get(1).map(String::as_str) == Some("pairs") {
            let vault = PathBuf::from(&args[0]);
            let out = match args.iter().position(|a| a == "--out") {
                Some(i) => PathBuf::from(&args[i + 1]),
                None => vault.join(PAIRS_QUEUE),
            };
            fs::create_dir_all(out.parent().unwrap()).unwrap();
            fs::write(out, "[]").unwrap();
        }
        Ok("notes 2, picked 0".into())
    }
}

/// Plugin cộng đồng giả, nội dung và sha256 của từng file.
fn fake_plugin(id: &str, version: &str) -> (PinnedPlugin, Vec<(String, Vec<u8>)>) {
    let mut files = Vec::new();
    let mut served = Vec::new();
    for name in PLUGIN_FILES {
        let body = if name == "manifest.json" {
            format!(r#"{{"id":"{id}","version":"{version}"}}"#).into_bytes()
        } else {
            format!("{id} {name}").into_bytes()
        };
        files.push((name.to_owned(), files::sha256_hex(&body)));
        served.push((name.to_owned(), body));
    }
    let p = PinnedPlugin {
        id: id.to_owned(),
        repo: format!("owner/{id}"),
        version: version.to_owned(),
        files,
        base: format!("https://github.com/owner/{id}/releases/download/{version}"),
    };
    let served = served
        .into_iter()
        .map(|(name, body)| (p.url(&name), body))
        .collect();
    (p, served)
}

/// Theme giả (`manifest.json`, `theme.css`) như theme Primary ghim của preset.
fn fake_theme(name: &str, version: &str) -> (PinnedPlugin, Vec<(String, Vec<u8>)>) {
    let mut p = PinnedPlugin {
        id: name.to_owned(),
        repo: "owner/theme".to_owned(),
        version: version.to_owned(),
        files: Vec::new(),
        base: "https://raw.githubusercontent.com/owner/theme/v1".to_owned(),
    };
    let mut served = Vec::new();
    for (file, body) in [
        (
            "manifest.json",
            format!(r#"{{"name":"{name}","version":"{version}"}}"#),
        ),
        ("theme.css", format!("/* {name} */")),
    ] {
        p.files
            .push((file.to_owned(), files::sha256_hex(body.as_bytes())));
        served.push((p.url(file), body.into_bytes()));
    }
    (p, served)
}

/// Ba file của bản dựng plugin `pkm` (giả).
fn pkm_plugin_files(version: &str) -> Vec<(&'static str, Vec<u8>)> {
    PLUGIN_FILES
        .iter()
        .map(|n| {
            let body = if *n == "manifest.json" {
                format!(r#"{{"id":"pkm","version":"{version}"}}"#).into_bytes()
            } else {
                format!("pkm {n}").into_bytes()
            };
            (*n, body)
        })
        .collect()
}

struct T {
    tmp: tempfile::TempDir,
    inst: Install,
    preset: Preset,
    vault: PathBuf,
    config: PathBuf,
    plugin_dir: PathBuf,
}

/// Vault tạm có `.obsidian/`, thư mục pkm, bản dựng plugin `pkm`, preset có một plugin cộng đồng giả.
fn t() -> (T, Fake) {
    let tmp = tempfile::tempdir().unwrap();
    let vault = tmp.path().join("Kho");
    fs::create_dir_all(vault.join(".obsidian")).unwrap();
    fs::write(vault.join("A.md"), "a").unwrap();
    let plugin_dir = tmp.path().join("plugin-build");
    fs::create_dir_all(&plugin_dir).unwrap();
    for (n, b) in pkm_plugin_files(VERSION) {
        fs::write(plugin_dir.join(n), b).unwrap();
    }
    let bin = tmp.path().join("bin");
    let inst = Install {
        pkm_dir: tmp.path().join("pkm"),
        exe: bin.join("pkm"),
        bundle: None,
        bin_dirs: vec![bin],
        ohs_source: None,
    };
    let (p, served) = fake_plugin("homepage", "4.2.2");
    let (theme, theme_files) = fake_theme("Primary", "2.10.0");
    let mut preset = Preset::embedded();
    preset.plugins = vec![p];
    preset.css_theme = theme;
    let mut fake = Fake::new();
    fake.served = [served, theme_files].concat();
    // Bản B cần Claude Code: có sẵn, trừ test bỏ nó đi.
    fake.programs = vec![("claude", PathBuf::from("/x/claude"))];
    let config = tmp.path().join("claude_desktop_config.json");
    (
        T {
            tmp,
            inst,
            preset,
            vault,
            config,
            plugin_dir,
        },
        fake,
    )
}

impl T {
    fn args(&self) -> SetupArgs {
        SetupArgs {
            vault: self.vault.clone(),
            plugin_dir: Some(self.plugin_dir.clone()),
            config: Some(self.config.clone()),
            no_ohs: true,
            ..SetupArgs::default()
        }
    }

    fn apply(&self) -> SetupArgs {
        SetupArgs {
            apply: true,
            ..self.args()
        }
    }

    /// Chạy, trả `(kết quả, dòng báo cáo)`.
    fn run(&self, args: &SetupArgs, hooks: &Fake) -> (Result<(), String>, Vec<Item>) {
        let mut out = Vec::new();
        let mut log = Log::new(&mut out, true);
        let r = Ctx::new(&self.inst, &self.preset, args, hooks).and_then(|c| {
            if args.remove {
                c.remove(&mut log)
            } else {
                c.setup(&mut log)
            }
        });
        (r, log.items())
    }

    fn ok(&self, args: &SetupArgs, hooks: &Fake) -> Vec<(String, Status, String)> {
        let (r, items) = self.run(args, hooks);
        r.unwrap_or_else(|e| panic!("{e}\n{items:#?}"));
        items
    }

    fn read(&self, rel: &str) -> String {
        fs::read_to_string(self.vault.join(rel)).unwrap_or_else(|e| panic!("{rel}: {e}"))
    }

    fn json(&self, rel: &str) -> Value {
        serde_json::from_str(&self.read(rel)).unwrap()
    }
}

/// Mọi file dưới `dir` kèm nội dung, để so trước và sau.
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

fn has(items: &[(String, Status, String)], step: &str, status: Status, text: &str) -> bool {
    items
        .iter()
        .any(|(s, st, t)| s == step && *st == status && t.contains(text))
}

#[test]
fn embedded_preset_parses_and_matches_its_readme() {
    let p = Preset::embedded();
    let ids: Vec<&str> = p.plugins.iter().map(|p| p.id.as_str()).collect();
    assert_eq!(
        ids,
        [
            "homepage",
            "templater-obsidian",
            "agent-client",
            "obsidian-style-settings"
        ]
    );
    assert!(p.plugins.iter().all(|p| p.files.len() == 3));
    assert_eq!(
        p.plugins[3].url("main.js"),
        "https://github.com/mgmeyers/obsidian-style-settings/releases/download/1.0.9/main.js"
    );
    let theme = &p.css_theme;
    assert_eq!(
        (theme.id.as_str(), theme.version.as_str()),
        ("Primary", "2.10.0")
    );
    assert_eq!(theme.files.len(), 2);
    assert_eq!(
        theme.url("theme.css"),
        "https://raw.githubusercontent.com/primary-theme/obsidian/v.2.10.0/theme.css"
    );
    assert_eq!(p.core_plugins, ["bases"]);
    assert_eq!(p.guide_target, ".claude/CLAUDE.md");
    assert_eq!(p.mcp_name, SERVER_NAME);
    let (name, css) = &p.snippets[0];
    assert_eq!(name, "e11-theme");
    // sha256 ghi trong presets/b/README.md.
    assert_eq!(
        files::sha256_hex(css),
        "e778c30df46ac222913d722be7927ae839fe46317f859850d378443c9f4c55de"
    );
    assert!(Preset::parse(&SETUP_JSON_WITH_UNKNOWN_FILE).is_err());
}

static SETUP_JSON_WITH_UNKNOWN_FILE: std::sync::LazyLock<String> =
    std::sync::LazyLock::new(|| preset::SETUP_JSON.replace("theme/e11-theme.css", "theme/x.css"));

#[test]
fn preset_flags_become_mcp_install_args() {
    let (t, fake) = t();
    let args = t.apply();
    let ctx = Ctx::new(&t.inst, &t.preset, &args, &fake).unwrap();
    let a = ctx.install_args().unwrap();
    assert_eq!(a.packs.as_deref(), Some("capture"));
    assert!(a.no_aliases && a.body_edges && a.no_ohs && !a.dry_run);
    assert_eq!(a.edge_keys.as_deref(), Some("links"));
    assert_eq!(a.adapter.as_deref(), Some("e11"));
    assert_eq!(a.db, Some(t.inst.default_db(&t.vault)));
    assert_eq!(a.config.as_deref(), Some(t.config.as_path()));
}

#[test]
fn dry_run_writes_nothing_and_lists_every_step() {
    let (t, mut fake) = t();
    fake.programs = vec![
        ("claude", PathBuf::from("/x/claude")),
        ("npm", PathBuf::from("/x/npm")),
    ];
    let before = snapshot(t.tmp.path());
    let items = t.ok(&t.args(), &fake);
    assert_eq!(snapshot(t.tmp.path()), before, "chạy thử không ghi gì");
    assert!(fake.runs().is_empty() && *fake.builds.borrow() == 0);
    for step in [
        "plugins",
        "pkm-plugin",
        "theme",
        "agent",
        "mcp",
        "writing",
        "workshop",
    ] {
        assert!(items.iter().any(|(s, _, _)| s == step), "thiếu bước {step}");
    }
    for name in ["normalize", "migrate_v3", "colours", "graph"] {
        assert!(has(&items, "not-ported", Status::Skip, name), "{name}");
    }
    assert!(has(&items, "plugins", Status::Todo, "homepage 4.2.2"));
    assert!(has(
        &items,
        "agent",
        Status::Todo,
        "claude-agent-acp@0.99.1"
    ));
    assert!(has(&items, "mcp", Status::Info, "[dry-run]"));
}

#[test]
fn apply_sets_up_the_vault_and_a_second_run_changes_nothing() {
    let (t, mut fake) = t();
    let claude = t.tmp.path().join("tools").join("claude");
    let npm = t.tmp.path().join("tools").join("npm");
    fake.programs = vec![("claude", claude.clone()), ("npm", npm.clone())];
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "plugins", Status::Done, "homepage 4.2.2"));
    assert_eq!(
        t.read(".obsidian/plugins/homepage/main.js"),
        "homepage main.js"
    );
    assert_eq!(
        t.json(".obsidian/community-plugins.json"),
        json!(["homepage", "pkm"])
    );
    assert_eq!(
        t.json(".obsidian/core-plugins.json"),
        json!({"bases": true})
    );
    assert_eq!(t.read(".obsidian/plugins/pkm/main.js"), "pkm main.js");
    let data = t.json(".obsidian/plugins/pkm/data.json");
    assert_eq!(data["pkmPath"], json!(t.inst.exe.to_str().unwrap()));
    assert_eq!(
        data["graphDbPath"],
        json!(t.inst.default_db(&t.vault).to_str().unwrap())
    );
    assert_eq!(
        fs::read(t.vault.join(".obsidian/snippets/e11-theme.css")).unwrap(),
        t.preset.snippets[0].1
    );
    let look = t.json(".obsidian/appearance.json");
    assert_eq!(look["enabledCssSnippets"], json!(["e11-theme"]));
    assert_eq!(look["baseFontSize"], json!(19));
    assert!(has(&items, "theme", Status::Done, "Primary 2.10.0"));
    assert_eq!(
        t.read(".obsidian/themes/Primary/theme.css"),
        "/* Primary */"
    );
    assert_eq!(look["cssTheme"], json!("Primary"));
    assert_eq!(t.read(".claude/CLAUDE.md").as_bytes(), t.preset.guide);
    assert_eq!(
        t.json(".claude/settings.local.json"),
        json!({"permissions": {"allow": ["mcp__pkm"]}})
    );
    // Claude Code: bỏ kg, ohs, pkm cũ rồi thêm pkm ở scope local, trong thư mục vault.
    let runs = fake.runs.borrow().clone();
    let claude_runs: Vec<&Vec<String>> = runs
        .iter()
        .filter(|(p, _, cwd)| *p == claude && cwd.as_deref() == Some(t.vault.as_path()))
        .map(|(_, a, _)| a)
        .collect();
    assert_eq!(claude_runs.len(), 4);
    assert_eq!(claude_runs[0][..5], ["mcp", "remove", "-s", "local", "kg"]);
    let add = claude_runs[3];
    assert_eq!(add[..5], ["mcp", "add", "-s", "local", "pkm"]);
    assert_eq!(add[5..7], ["-e", "PKM_NO_OHS=1"]);
    let sep = add.iter().position(|a| a == "--").unwrap();
    assert_eq!(add[sep + 1], t.inst.exe.to_str().unwrap());
    assert_eq!(add[sep + 2], t.vault.to_str().unwrap());
    assert!(add.contains(&"--body-edges".to_owned()) && add.contains(&"e11".to_owned()));
    // Adapter ACP vào thư mục của pkm, script khởi động, preset của Agent Client.
    let acp = t.inst.pkm_dir.join(ACP_DIR);
    assert!(runs.iter().any(|(p, a, _)| *p == npm
        && a == &[
            "install",
            "--prefix",
            acp.to_str().unwrap(),
            "@agentclientprotocol/claude-agent-acp@0.99.1"
        ]));
    let wrapper = acp.join(if cfg!(windows) {
        "claude-agent-acp.cmd"
    } else {
        "claude-agent-acp.sh"
    });
    assert!(
        fs::read_to_string(&wrapper)
            .unwrap()
            .contains(claude.to_str().unwrap())
    );
    let agent = t.json(".obsidian/plugins/agent-client/data.json");
    assert_eq!(
        agent["presetAgents"]["claude-code-acp"]["command"],
        json!(wrapper.to_str().unwrap())
    );
    // Claude Desktop.
    let cfg: Value = serde_json::from_str(&fs::read_to_string(&t.config).unwrap()).unwrap();
    assert_eq!(
        cfg["mcpServers"]["pkm"]["command"],
        json!(t.inst.exe.to_str().unwrap())
    );
    assert_eq!(*fake.builds.borrow(), 1);
    let m = Manifest::load(&t.vault).unwrap().unwrap();
    assert_eq!(m.list("plugins"), ["homepage"]);
    assert_eq!(m.list("themes"), ["Primary"]);
    assert_eq!(m.list("mcp"), ["pkm"]);
    assert_eq!(
        m.get("desktop_config"),
        Some(&json!(t.config.to_str().unwrap()))
    );

    // Lần hai: không còn việc gì, không chạy claude hay npm nữa.
    let snap = snapshot(&t.vault);
    let n = fake.runs.borrow().len();
    let again = t.ok(&t.apply(), &fake);
    assert!(
        !again.iter().any(|(_, s, _)| *s == Status::Done),
        "{again:#?}"
    );
    assert_eq!(fake.runs.borrow().len(), n);
    assert_eq!(snapshot(&t.vault), snap);
}

#[test]
fn wrong_pinned_hash_writes_nothing() {
    let (t, mut fake) = t();
    fake.served[0].1 = b"tampered".to_vec();
    let (r, _) = t.run(&t.apply(), &fake);
    assert!(r.unwrap_err().contains("sha256 không khớp"));
    let plugins = fs::read_dir(t.vault.join(".obsidian/plugins"))
        .unwrap()
        .count();
    assert_eq!(plugins, 0, "không để lại thư mục plugin hay thư mục tạm");
}

#[test]
fn plugin_of_another_version_is_reported_not_replaced() {
    let (t, fake) = t();
    let dir = t.vault.join(".obsidian/plugins/homepage");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("manifest.json"), r#"{"version":"4.0.0"}"#).unwrap();
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "plugins", Status::Warn, "4.0.0"));
    assert!(!dir.join("main.js").exists());
    assert!(
        Manifest::load(&t.vault)
            .unwrap()
            .unwrap()
            .list("plugins")
            .is_empty()
    );
}

/// Zip phẳng của ba file.
fn zip_of(files: &[(&str, Vec<u8>)]) -> Vec<u8> {
    let mut buf = io::Cursor::new(Vec::new());
    {
        let mut z = zip::ZipWriter::new(&mut buf);
        let opts = zip::write::SimpleFileOptions::default();
        for (n, b) in files {
            z.start_file(*n, opts).unwrap();
            z.write_all(b).unwrap();
        }
        z.finish().unwrap();
    }
    buf.into_inner()
}

#[test]
fn release_plugin_is_checked_against_checksums_txt() {
    let (t, mut fake) = t();
    let zip = zip_of(&pkm_plugin_files(VERSION));
    let base = format!("https://github.com/{RELEASE_REPO}/releases/download/v{VERSION}");
    let sums = format!(
        "{}  pkm-x86_64-unknown-linux-gnu\n{}  {PLUGIN_ASSET}\n",
        "0".repeat(64),
        files::sha256_hex(&zip)
    );
    fake.served
        .push((format!("{base}/checksums.txt"), sums.into_bytes()));
    fake.served
        .push((format!("{base}/{PLUGIN_ASSET}"), zip.clone()));
    let mut args = t.apply();
    args.plugin_dir = None;
    let items = t.ok(&args, &fake);
    assert!(has(&items, "pkm-plugin", Status::Done, PLUGIN_ASSET));
    assert_eq!(t.read(".obsidian/plugins/pkm/styles.css"), "pkm styles.css");
    assert!(!t.vault.join(".obsidian/plugins/.pkm.pkm-setup").exists());

    // Zip bị sửa: dừng, không đổi gì.
    let (t, mut fake) = self::t();
    fake.served.push((
        format!("{base}/checksums.txt"),
        format!("{}  {PLUGIN_ASSET}\n", "a".repeat(64)).into_bytes(),
    ));
    fake.served.push((format!("{base}/{PLUGIN_ASSET}"), zip));
    let mut args = t.apply();
    args.plugin_dir = None;
    let (r, _) = t.run(&args, &fake);
    assert!(r.unwrap_err().contains("khác checksums.txt"));
    assert!(!t.vault.join(".obsidian/plugins/pkm").exists());
}

#[test]
fn plugin_of_another_version_is_refused() {
    let (t, fake) = t();
    fs::write(
        t.plugin_dir.join("manifest.json"),
        r#"{"id":"pkm","version":"0.0.1"}"#,
    )
    .unwrap();
    let (r, _) = t.run(&t.apply(), &fake);
    assert!(r.unwrap_err().contains("0.0.1"));
}

#[test]
fn files_the_user_edited_are_never_overwritten_or_removed() {
    let (t, fake) = t();
    t.ok(&t.apply(), &fake);
    let snippet = t.vault.join(".obsidian/snippets/e11-theme.css");
    fs::write(&snippet, "/* của tôi */").unwrap();
    fs::write(t.vault.join(".obsidian/plugins/pkm/data.json"), "{}").unwrap();
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "theme", Status::Warn, "e11-theme.css"));
    assert_eq!(fs::read_to_string(&snippet).unwrap(), "/* của tôi */");
    assert_eq!(t.read(".obsidian/plugins/pkm/data.json"), "{}");

    let mut rm = t.apply();
    rm.remove = true;
    t.ok(&rm, &fake);
    assert_eq!(fs::read_to_string(&snippet).unwrap(), "/* của tôi */");
    assert!(!t.vault.join(".obsidian/plugins/pkm/main.js").exists());
    assert!(t.vault.join(".obsidian/plugins/pkm/data.json").exists());
}

#[test]
fn newer_plugin_build_replaces_files_setup_wrote() {
    let (t, fake) = t();
    t.ok(&t.apply(), &fake);
    fs::write(t.plugin_dir.join("main.js"), "pkm main.js v2").unwrap();
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "pkm-plugin", Status::Done, "cài từ"));
    assert_eq!(t.read(".obsidian/plugins/pkm/main.js"), "pkm main.js v2");
    // Thư mục plugin người dùng tự chép: không thay file nào.
    let (t, fake) = self::t();
    let dir = t.vault.join(".obsidian/plugins/pkm");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("main.js"), "tự chép").unwrap();
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "pkm-plugin", Status::Warn, "giữ nguyên"));
    assert_eq!(t.read(".obsidian/plugins/pkm/main.js"), "tự chép");
    assert!(!dir.join("manifest.json").exists());
}

#[test]
fn without_claude_setup_stops_at_the_agent_step_and_without_npm_the_adapter_is_skipped() {
    let (t, mut fake) = t();
    fake.programs.clear();
    let items = t.ok(&t.args(), &fake);
    assert!(has(&items, "agent", Status::Warn, "bản B cần Claude Code"));
    let (r, items) = t.run(&t.apply(), &fake);
    assert!(r.unwrap_err().contains("bản B cần Claude Code"));
    assert!(
        !items.iter().any(|(s, _, _)| s == "mcp"),
        "dừng trước bước mcp"
    );
    assert!(t.vault.join(".claude/CLAUDE.md").exists());
    assert!(fake.runs().is_empty());

    fake.programs = vec![("claude", PathBuf::from("/x/claude"))];
    let items = t.ok(&t.apply(), &fake);
    let acp = t.inst.pkm_dir.join(ACP_DIR);
    let cmd = install::shell::command_line(
        "npm",
        &[
            "install",
            "--prefix",
            acp.to_str().unwrap(),
            "@agentclientprotocol/claude-agent-acp@0.99.1",
        ],
    );
    assert!(has(&items, "agent", Status::Skip, &cmd), "{items:#?}");
    assert!(
        !t.vault
            .join(".obsidian/plugins/agent-client/data.json")
            .exists()
    );
}

#[test]
fn agent_client_command_set_by_the_user_is_kept() {
    let (t, mut fake) = t();
    fake.programs = vec![
        ("claude", PathBuf::from("/x/claude")),
        ("npm", PathBuf::from("/x/npm")),
    ];
    let path = t.vault.join(".obsidian/plugins/agent-client/data.json");
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let mine = json!({"presetAgents": {"claude-code-acp": {"command": "/my/acp"}}, "other": 1});
    fs::write(&path, mine.to_string()).unwrap();
    let items = t.ok(&t.apply(), &fake);
    assert!(has(&items, "agent", Status::Ok, "/my/acp"));
    assert_eq!(t.json(".obsidian/plugins/agent-client/data.json"), mine);
}

/// Gói OHS giả ở `dir` (đủ để `OhsPackage::from_dir` nhận).
fn fake_ohs(dir: &Path) {
    let node = if cfg!(windows) {
        "node/node.exe"
    } else {
        "node/bin/node"
    };
    fs::create_dir_all(dir.join(node).parent().unwrap()).unwrap();
    fs::write(dir.join(node), "").unwrap();
    fs::create_dir_all(dir.join("app/dist/src")).unwrap();
    fs::write(dir.join("app/dist/src/cli.js"), "").unwrap();
    fs::write(dir.join("MANIFEST.json"), r#"{"version":"0.17.2-vantt.4"}"#).unwrap();
}

#[test]
fn writing_helpers_run_only_when_ohs_is_ready() {
    let (t, mut fake) = t();
    fake_ohs(&t.inst.ohs_dir());
    let mut args = t.apply();
    args.no_ohs = false;
    fake.ohs_state = OhsState::Indexing {
        indexed: 1,
        pending: 9,
        eta_s: Some(30),
        progress: 0.1,
    };
    let items = t.ok(&args, &fake);
    assert!(has(&items, "writing", Status::Skip, "OHS chưa sẵn sàng"));
    let pkm_runs = || -> Vec<Vec<String>> {
        fake.runs
            .borrow()
            .iter()
            .filter(|(p, _, _)| *p == t.inst.exe)
            .map(|(_, a, _)| a.clone())
            .collect()
    };
    assert!(pkm_runs().is_empty());

    fake.ohs_state = OhsState::Ready;
    let items = t.ok(&args, &fake);
    let runs = pkm_runs();
    let ops: Vec<&str> = runs.iter().map(|a| a[1].as_str()).collect();
    assert_eq!(ops, ["pairs", "pairs", "propose", "zones"]);
    assert_eq!(runs[0][2], "--write");
    assert!(runs[1].contains(&"--per-note".to_owned()));
    assert_eq!(runs[2][2..4], ["--create", "--similar"]);
    let db = t.inst.default_db(&t.vault);
    assert!(
        runs[3]
            .windows(2)
            .any(|w| w[0] == "--db" && w[1] == db.to_str().unwrap())
    );
    assert!(has(&items, "writing", Status::Done, "notes 2"));
    let m = Manifest::load(&t.vault).unwrap().unwrap();
    assert!(m.file_hash(PAIRS_QUEUE).is_some() && m.file_hash(PAIRS_WIDE).is_some());

    // Lần sau: cặp đã có, trang vùng đã có: chỉ chạy lại propose.
    fs::create_dir_all(t.vault.join("_Xưởng")).unwrap();
    fs::write(t.vault.join(pkm_core::zones::PAGE), "Chấm").unwrap();
    fake.runs.borrow_mut().clear();
    t.ok(&args, &fake);
    let ops: Vec<String> = pkm_runs().iter().map(|a| a[1].clone()).collect();
    assert_eq!(ops, ["propose"]);
}

#[test]
fn remove_undoes_setup_and_restores_settings() {
    let (t, mut fake) = t();
    let claude = PathBuf::from("/x/claude");
    fake.programs = vec![("claude", claude.clone()), ("npm", PathBuf::from("/x/npm"))];
    let obs = t.vault.join(".obsidian");
    fs::write(
        obs.join("community-plugins.json"),
        r#"["dataview","obsidian-minimal-settings"]"#,
    )
    .unwrap();
    fs::write(
        obs.join("appearance.json"),
        r#"{"cssTheme":"Primary","baseFontSize":16}"#,
    )
    .unwrap();
    fs::write(
        obs.join("core-plugins.json"),
        r#"{"bases":false,"graph":true}"#,
    )
    .unwrap();
    t.ok(&t.apply(), &fake);
    assert_eq!(
        t.json(".obsidian/community-plugins.json"),
        json!(["dataview", "homepage", "pkm"])
    );

    let mut rm = t.apply();
    rm.remove = true;
    let before_dry = snapshot(&t.vault);
    let mut dry = rm.clone();
    dry.apply = false;
    t.ok(&dry, &fake);
    assert_eq!(snapshot(&t.vault), before_dry, "gỡ thử không đổi gì");

    fake.runs.borrow_mut().clear();
    t.ok(&rm, &fake);
    assert_eq!(
        t.json(".obsidian/community-plugins.json"),
        json!(["dataview", "obsidian-minimal-settings"])
    );
    assert_eq!(
        t.json(".obsidian/appearance.json"),
        json!({"cssTheme": "Primary", "baseFontSize": 16, "enabledCssSnippets": []})
    );
    assert_eq!(
        t.json(".obsidian/core-plugins.json"),
        json!({"bases": false, "graph": true})
    );
    for gone in [
        ".obsidian/plugins/homepage",
        ".obsidian/plugins/pkm",
        ".obsidian/themes/Primary",
        ".obsidian/plugins/agent-client/data.json",
        ".obsidian/snippets",
        ".claude/CLAUDE.md",
        files::MANIFEST,
    ] {
        assert!(!t.vault.join(gone).exists(), "{gone}");
    }
    assert_eq!(fake.runs(), [["mcp", "remove", "-s", "local", "pkm"]]);
    let cfg: Value = serde_json::from_str(&fs::read_to_string(&t.config).unwrap()).unwrap();
    assert!(cfg["mcpServers"].get("pkm").is_none());
    assert!(t.vault.join("A.md").exists());
}

#[test]
fn remove_reads_the_manifest_of_setup_vault_py() {
    let (t, mut fake) = t();
    fake.programs = vec![("claude", PathBuf::from("/x/claude"))];
    let obs = t.vault.join(".obsidian");
    let dir = obs.join("plugins/homepage");
    fs::create_dir_all(&dir).unwrap();
    for n in PLUGIN_FILES {
        fs::write(dir.join(n), "x").unwrap();
    }
    fs::write(
        obs.join("community-plugins.json"),
        r#"["homepage","dataview"]"#,
    )
    .unwrap();
    fs::write(obs.join("core-plugins.json"), r#"{"bases":true}"#).unwrap();
    fs::create_dir_all(t.vault.join(".claude")).unwrap();
    fs::write(t.vault.join(".claude/CLAUDE.md"), t.preset.guide).unwrap();
    fs::write(
        t.vault.join(".claude/settings.local.json"),
        r#"{"permissions":{"allow":["Read","mcp__pkm"]}}"#,
    )
    .unwrap();
    let py = json!({
        "plugins": ["homepage"], "community": ["homepage"], "core": ["bases"], "core_off": [],
        "agent_guide": files::sha256_hex(t.preset.guide), "agent_allow": ["mcp__pkm"], "mcp": ["pkm"],
        "agent_settings": {"old": "", "created": true, "command": "/x/w.sh"}
    });
    fs::create_dir_all(t.vault.join("_E11")).unwrap();
    fs::write(t.vault.join(files::MANIFEST), py.to_string()).unwrap();
    let agent = obs.join("plugins/agent-client");
    fs::create_dir_all(&agent).unwrap();
    fs::write(agent.join("data.json"), "{}").unwrap();
    let mut rm = t.apply();
    rm.remove = true;
    t.ok(&rm, &fake);
    assert!(!dir.exists() && !agent.join("data.json").exists());
    assert!(!t.vault.join(".claude/CLAUDE.md").exists());
    assert_eq!(
        t.json(".obsidian/community-plugins.json"),
        json!(["dataview"])
    );
    assert_eq!(t.json(".obsidian/core-plugins.json"), json!({}));
    assert_eq!(
        t.json(".claude/settings.local.json"),
        json!({"permissions": {"allow": ["Read"]}})
    );
    assert_eq!(fake.runs(), [["mcp", "remove", "-s", "local", "pkm"]]);
}

#[test]
fn remove_without_manifest_and_folder_without_obsidian_are_errors() {
    let (t, fake) = t();
    let mut rm = t.apply();
    rm.remove = true;
    assert!(
        t.run(&rm, &fake)
            .0
            .unwrap_err()
            .contains("không có gì để gỡ")
    );
    fs::remove_dir(t.vault.join(".obsidian")).unwrap();
    assert!(
        t.run(&t.apply(), &fake)
            .0
            .unwrap_err()
            .contains(".obsidian")
    );
}

#[test]
fn json_report_carries_steps_and_errors() {
    let (t, fake) = t();
    let mut args = t.args();
    args.json = true;
    let mut out = Vec::new();
    run_with(&t.inst, &t.preset, &args, &fake, &mut out).unwrap();
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], json!(true));
    assert_eq!(v["apply"], json!(false));
    let steps: Vec<&str> = v["steps"]
        .as_array()
        .unwrap()
        .iter()
        .map(|s| s["step"].as_str().unwrap())
        .collect();
    assert!(steps.contains(&"pkm-plugin") && steps.contains(&"not-ported"));
    assert!(v["steps"][1]["items"][0]["status"].is_string());

    fs::remove_dir(t.vault.join(".obsidian")).unwrap();
    let mut out = Vec::new();
    assert!(run_with(&t.inst, &t.preset, &args, &fake, &mut out).is_err());
    let v: Value = serde_json::from_slice(&out).unwrap();
    assert_eq!(v["ok"], json!(false));
    assert!(v["error"].as_str().unwrap().contains(".obsidian"));
}

#[test]
fn checksums_line_must_be_unique_and_hex() {
    let sha = "AB".repeat(32);
    assert_eq!(
        checksum_for(&format!("{sha} *{PLUGIN_ASSET}\n"), PLUGIN_ASSET).unwrap(),
        sha.to_lowercase()
    );
    assert!(checksum_for("", PLUGIN_ASSET).is_err());
    assert!(checksum_for(&format!("xyz  {PLUGIN_ASSET}"), PLUGIN_ASSET).is_err());
    let twice = format!("{sha}  {PLUGIN_ASSET}\n{sha}  {PLUGIN_ASSET}\n");
    assert!(checksum_for(&twice, PLUGIN_ASSET).is_err());
    assert!(unzip_plugin(b"not a zip").is_err());
    let partial = zip_of(&[("main.js", b"x".to_vec())]);
    assert!(
        unzip_plugin(&partial)
            .unwrap_err()
            .contains("manifest.json")
    );
}

#[test]
fn log_turns_written_lines_into_items() {
    let mut out = Vec::new();
    let mut log = Log::new(&mut out, false);
    log.step("mcp", "x").unwrap();
    write!(log, "một\r\nhai\nba").unwrap();
    log.flush_partial().unwrap();
    let texts: Vec<String> = log.items().into_iter().map(|(_, _, t)| t).collect();
    assert_eq!(texts, ["một", "hai", "ba"]);
    drop(log);
    assert!(
        String::from_utf8(out)
            .unwrap()
            .contains("[mcp] x\n    một\n")
    );
}
